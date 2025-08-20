use std::sync::Arc;
use std::time::{Duration, Instant};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::{BufRead, BufReader};
use tauri::{Manager, Emitter};
use tauri_plugin_cli::CliExt;
use thirtyfour::prelude::*;
use url::Url;
use serde::Deserialize;


#[derive(Deserialize)]
struct App {
    url: String,
    on_screen_duration_seconds: u64,
}

#[derive(Deserialize)]
struct ApiResponse {
    apps: Vec<App>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum AutomationState {
    KioskMode,          // Running normal kiosk URL cycling
    ZoomRunning,        // Zoom automation in progress
    ZoomComplete,       // Zoom automation complete, waiting for stop signal
    Stopping,           // Currently stopping Chrome/ChromeDriver
}

async fn fetch_apps(token: &str) -> Result<Vec<App>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://rctv.recurse.com/get_all_apps_for_tauri?tv_login_token={}", token);
    let response = reqwest::get(&url).await?;
    let api_response: ApiResponse = response.json().await?;
    Ok(api_response.apps)
}

async fn check_if_in_meeting(driver: &WebDriver) -> bool {
    // Check for Leave button to confirm we're actually in the meeting
    match driver.find(By::XPath("//*[contains(text(), 'Leave')]")).await {
        Ok(_) => {
            println!("Leave button found - confirmed in meeting!");
            true
        }
        Err(_) => {
            println!("Leave button not found - not in meeting yet");
            false
        }
    }
}

async fn start_kiosk_mode(token: String, app_handle: Arc<tauri::AppHandle>, state: Arc<std::sync::Mutex<AutomationState>>) -> std::io::Result<()> {
    println!("Starting kiosk mode...");
    
    // Show the window
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_cursor_visible(false);
    }
    
    loop {
        // Check if we should still be in kiosk mode
        {
            let current_state = state.lock().unwrap();
            if *current_state != AutomationState::KioskMode {
                println!("Exiting kiosk mode, current state: {:?}", *current_state);
                break;
            }
        }
        
        println!("Fetching apps from API...");
        match fetch_apps(&token).await {
            Ok(apps) => {
                if !apps.is_empty() {
                    println!("Found {} apps, cycling through them", apps.len());
                    
                    for app in &apps {
                        // Check again if we should still be in kiosk mode
                        {
                            let current_state = state.lock().unwrap();
                            if *current_state != AutomationState::KioskMode {
                                println!("Exiting kiosk mode during URL cycling, current state: {:?}", *current_state);
                                return Ok(());
                            }
                        }
                        
                        println!("Loading URL: {} for {} seconds", app.url, app.on_screen_duration_seconds);
                        
                        // Parse and navigate to URL
                        match Url::parse(&app.url) {
                            Ok(parsed_url) => {
                                if let Some(window) = app_handle.get_webview_window("main") {
                                    let navigate_result = window.navigate(parsed_url);
                                    if let Err(e) = navigate_result {
                                        println!("Failed to navigate: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Failed to parse URL {}: {}", app.url, e);
                            }
                        }
                        
                        // Wait for the specified duration, checking periodically if we should exit
                        let wait_time = app.on_screen_duration_seconds;
                        let check_interval = std::cmp::min(wait_time, 5); // Check every 5 seconds or less
                        let mut elapsed = 0;
                        
                        while elapsed < wait_time {
                            {
                                let current_state = state.lock().unwrap();
                                if *current_state != AutomationState::KioskMode {
                                    println!("Exiting kiosk mode during wait, current state: {:?}", *current_state);
                                    return Ok(());
                                }
                            }
                            
                            let sleep_time = std::cmp::min(check_interval, wait_time - elapsed);
                            tokio::time::sleep(Duration::from_secs(sleep_time)).await;
                            elapsed += sleep_time;
                        }
                    }
                } else {
                    println!("No apps found, waiting 10 seconds before retry...");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
            Err(e) => {
                println!("Failed to fetch apps: {}, waiting 10 seconds before retry...", e);
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        }
    }
    
    Ok(())
}

async fn kill_chrome_processes() {
    println!("Killing Chrome/Chromium and ChromeDriver processes...");
    
    // Kill ChromeDriver
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("chromedriver")
        .output();
    
    // Kill Chrome/Chromium
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("chromium")
        .output();
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("chrome")
        .output();
    
    // Wait for processes to die
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("Chrome processes killed");
}

async fn start_chromium_controller() -> WebDriverResult<()> {
    // Kill any existing Chrome/Chromium processes
    println!("Cleaning up existing Chrome processes...");
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("chromium")
        .output();
    
    // Wait a moment for processes to die
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Start Chromium directly with remote debugging
    println!("Starting Chromium with remote debugging...");
    let _chromium = Command::new("/usr/bin/chromium-browser")
        .arg("--remote-debugging-port=9222")
        .arg("--user-data-dir=/home/rctv/.rctv-chrome-profile")
        .arg("--autoplay-policy=no-user-gesture-required")
        .arg("--disable-logging")
        .arg("--log-level=3")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-extensions")
        .arg("--disable-gpu")
        .stdout(Stdio::null()) // Hide Chromium output
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start Chromium");

    // Wait for Chromium to start and check if debugging port is available
    println!("Waiting for Chromium debugging port...");
    for i in 0..10 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        // Test if port 9222 is accepting connections
        match reqwest::get("http://localhost:9222/json").await {
            Ok(response) => {
                println!("Chromium debugging port is ready! Status: {}", response.status());
                break;
            }
            Err(e) => {
                println!("Attempt {}: Chromium debugging port not ready yet: {}", i+1, e);
                if i == 9 {
                    eprintln!("Failed to connect to Chromium debugging port after 10 seconds");
                    return Err(WebDriverError::FatalError("Chromium debugging port not available".to_string()));
                }
            }
        }
    }

    // Start ChromeDriver to bridge to existing Chromium
    println!("Starting ChromeDriver...");
    let _chromedriver = Command::new("chromedriver")
        .arg("--port=9515")
        .arg("--whitelisted-ips=")
        .arg("--silent")
        .stdout(Stdio::null()) // Hide ChromeDriver output
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start ChromeDriver");

    // Wait for ChromeDriver to start
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Connect to ChromeDriver and tell it to use existing Chromium
    let mut caps = DesiredCapabilities::chrome();
    caps.add_experimental_option("debuggerAddress", "localhost:9222")?;
    
    println!("Connecting ChromeDriver to existing Chromium...");
    let driver = WebDriver::new("http://localhost:9515", caps).await?;
    
    // Navigate to Zoom meeting
    driver.goto("https://app.zoom.us/wc/2125949362/join?fromPWA=1&pwd=OEJ3Nkw4djlmSlBBVWl2aVdXTk93Zz09").await?;
    
    println!("Successfully opened Zoom meeting in Chromium");
    
    // Debug: Check what page we're on
    tokio::time::sleep(Duration::from_secs(3)).await; // Wait for page to load
    let current_url = driver.current_url().await?;
    let page_title = driver.title().await?;
    let page_source = driver.source().await?;
    
    println!("Current URL: {}", current_url);
    println!("Page title: {}", page_title);
    println!("Page source length: {} characters", page_source.len());
    println!("First 500 chars of page: {}", &page_source[..std::cmp::min(500, page_source.len())]);
    
    // Automate sign-in process
    println!("Starting automated sign-in process...");
    
    // Step 1: Switch to iframe and click "sign in" link
    println!("Looking for iframe and sign in link...");
    
    // Find iframes immediately
    let iframes = driver.find_all(By::Tag("iframe")).await?;
    println!("Found {} iframes", iframes.len());
    
    let mut sign_in_link = None;
    'outer: loop {
        // Try to find the link in the main page first
        match driver.find(By::XPath("//a[contains(text(), 'sign in')]")).await {
            Ok(element) => {
                sign_in_link = Some(element);
                break 'outer;
            }
            Err(_) => {
                // Try each iframe
                for (i, _iframe) in iframes.iter().enumerate() {
                    println!("Switching to iframe {}", i);
                    match driver.enter_frame(i as u16).await {
                        Ok(_) => {
                            match driver.find(By::XPath("//a[contains(text(), 'sign in')]")).await {
                                Ok(element) => {
                                    println!("Found sign in link in iframe {}", i);
                                    sign_in_link = Some(element);
                                    break 'outer;
                                }
                                Err(_) => {
                                    // Switch back to main content and try next iframe
                                    let _ = driver.enter_default_frame().await;
                                }
                            }
                        }
                        Err(_) => {
                            println!("Failed to switch to iframe {}", i);
                        }
                    }
                }
                println!("Sign in link not found in any iframe, retrying in 2 seconds...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    let sign_in_link = sign_in_link.unwrap();
    sign_in_link.click().await?;
    println!("Clicked sign in link");
    
    // Switch back to default frame after clicking sign in
    let _ = driver.enter_default_frame().await;
    
    // Step 2: Wait for and click Google sign-in link with retries
    println!("Looking for Google sign-in link...");
    let google_button = loop {
        match driver.find(By::XPath("//a[@aria-label='Sign in with Google']")).await {
            Ok(element) => break element,
            Err(_) => {
                println!("Google sign-in link not found, retrying in 2 seconds...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };
    google_button.click().await?;
    println!("Clicked Google sign-in button");
    
    // Step 3: Wait for and click "Recurse RCTV" account with retries
    println!("Looking for Recurse RCTV account...");
    let recurse_account = loop {
        match driver.find(By::XPath("//div[contains(text(), 'Recurse RCTV')]")).await {
            Ok(element) => break element,
            Err(_) => {
                println!("Recurse RCTV account not found, retrying in 2 seconds...");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };
    recurse_account.click().await?;
    println!("Selected Recurse RCTV account");
    
    // Step 4: Look for optional "Use microphone and camera" button (don't block if not found)
    println!("Looking for optional Use microphone and camera button...");
    let mut found_mic_camera = false;
    for attempt in 1..=3 {
        match driver.find(By::XPath("//button[contains(text(), 'Use microphone and camera')]")).await {
            Ok(element) => {
                match element.click().await {
                    Ok(_) => {
                        println!("Clicked Use microphone and camera button");
                        found_mic_camera = true;
                        break;
                    }
                    Err(e) => {
                        println!("Could not click microphone button: {}", e);
                    }
                }
            }
            Err(_) => {
                match driver.find(By::XPath("//*[contains(text(), 'Use microphone and camera')]")).await {
                    Ok(element) => {
                        match element.click().await {
                            Ok(_) => {
                                println!("Clicked Use microphone and camera button");
                                found_mic_camera = true;
                                break;
                            }
                            Err(e) => {
                                println!("Could not click microphone button: {}", e);
                            }
                        }
                    }
                    Err(_) => {
                        println!("Use microphone and camera button not found (attempt {})", attempt);
                        if attempt < 3 {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                    }
                }
            }
        }
    }
    if !found_mic_camera {
        println!("Use microphone and camera button not found - continuing without it");
    }
    
    // Step 5: Wait for and click "Join" button with retries (check iframes too)
    println!("Looking for Join button...");
    let mut join_attempts = 0;
    'outer: loop {
        join_attempts += 1;
        println!("Join button attempt #{}", join_attempts);
        // Find ALL Join buttons and check which ones are actually visible
        println!("Searching for ALL Join buttons to find visible one...");
        let join_button_result = match driver.find_all(By::XPath("//*[contains(text(), 'Join')]")).await {
            Ok(buttons) => {
                println!("Found {} total Join buttons, checking visibility...", buttons.len());
                let mut visible_button = None;
                
                for (i, button) in buttons.iter().enumerate() {
                    let display = button.css_value("display").await.unwrap_or_default();
                    let visibility = button.css_value("visibility").await.unwrap_or_default();
                    let is_displayed = button.is_displayed().await.unwrap_or(false);
                    println!("Button {} - display: {}, visibility: {}, is_displayed: {}", i, display, visibility, is_displayed);
                    
                    if display != "none" && visibility != "hidden" && is_displayed {
                        println!("Found visible Join button #{}", i);
                        visible_button = Some(button.clone());
                        break;
                    }
                }
                
                match visible_button {
                    Some(button) => Ok(button),
                    None => {
                        println!("No visible Join button found in main page");
                        Err(WebDriverError::FatalError("No visible Join button found".to_string()))
                    }
                }
            }
            Err(e) => Err(e)
        };
        
        let join_button = match join_button_result {
            Ok(element) => element,
            Err(_) => {
                // Not found in main page, try iframes
                let mut found_in_iframe = None;
                let current_iframes = driver.find_all(By::Tag("iframe")).await.unwrap_or_default();
                
                for (i, _iframe) in current_iframes.iter().enumerate() {
                    match driver.enter_frame(i as u16).await {
                        Ok(_) => {
                            let iframe_result = match driver.find(By::XPath("//button[contains(text(), 'Join')]")).await {
                                Ok(element) => Ok(element),
                                Err(_) => match driver.find(By::XPath("//input[@value='Join']")).await {
                                    Ok(element) => Ok(element),
                                    Err(_) => driver.find(By::XPath("//*[contains(text(), 'Join')]")).await,
                                },
                            };
                            
                            match iframe_result {
                                Ok(element) => {
                                    println!("Found Join button in iframe {}", i);
                                    
                                    // Check button visibility and state
                                    let is_enabled = element.is_enabled().await.unwrap_or(false);
                                    let is_displayed = element.is_displayed().await.unwrap_or(false);
                                    let is_selected = element.is_selected().await.unwrap_or(false);
                                    println!("Button state in iframe {} - enabled: {}, displayed: {}, selected: {}", 
                                            i, is_enabled, is_displayed, is_selected);
                                    
                                    // Try to click immediately while we're in the right iframe
                                    println!("Attempting to interact with Join button in iframe {}", i);
                                    for click_attempt in 1..=3 {
                                        println!("Trying standard click in iframe (attempt {} of 3)...", click_attempt);
                                        match element.click().await {
                                            Ok(_) => {
                                                println!("Standard click succeeded in iframe!");
                                                
                                                // Wait and check if we're in the meeting
                                                tokio::time::sleep(Duration::from_secs(3)).await;
                                                
                                                if check_if_in_meeting(&driver).await {
                                                    println!("Successfully joined meeting from iframe!");
                                                    return Ok(());
                                                } else {
                                                    println!("Click in iframe didn't work, trying next attempt...");
                                                }
                                            }
                                            Err(e) => {
                                                println!("Standard click in iframe attempt {} failed: {}", click_attempt, e);
                                                
                                                // Try pressing Enter as alternative
                                                println!("Trying Enter key press instead...");
                                                match element.send_keys("\n").await {
                                                    Ok(_) => {
                                                        println!("Enter key press succeeded!");
                                                        
                                                        // Wait and check if we're in the meeting
                                                        tokio::time::sleep(Duration::from_secs(3)).await;
                                                        
                                                        if check_if_in_meeting(&driver).await {
                                                            println!("Successfully joined meeting with Enter key!");
                                                            return Ok(());
                                                        } else {
                                                            println!("Enter key didn't work either, continuing...");
                                                        }
                                                    }
                                                    Err(enter_e) => {
                                                        println!("Enter key press also failed: {}", enter_e);
                                                    }
                                                }
                                                
                                                if click_attempt < 3 {
                                                    tokio::time::sleep(Duration::from_secs(1)).await;
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Try JavaScript click in iframe as fallback
                                    println!("Trying JavaScript click in iframe {}...", i);
                                    match driver.execute("arguments[0].click();", vec![element.to_json()?]).await {
                                        Ok(_) => {
                                            println!("JavaScript click executed in iframe");
                                            tokio::time::sleep(Duration::from_secs(3)).await;
                                            
                                            if check_if_in_meeting(&driver).await {
                                                println!("Successfully joined meeting with JavaScript click in iframe!");
                                                return Ok(());
                                            }
                                        }
                                        Err(e) => {
                                            println!("JavaScript click in iframe failed: {}", e);
                                        }
                                    }
                                    
                                    found_in_iframe = Some(element);
                                    break;
                                }
                                Err(_) => {
                                    let _ = driver.enter_default_frame().await;
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
                
                match found_in_iframe {
                    Some(element) => element,
                    None => {
                        println!("Join button not found anywhere, retrying in 2 seconds...");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                }
            }
        };
        
        // Debug the button state before clicking  
        let is_enabled = join_button.is_enabled().await.unwrap_or(false);
        let is_displayed = join_button.is_displayed().await.unwrap_or(false);
        let is_selected = join_button.is_selected().await.unwrap_or(false);
        let tag_name = join_button.tag_name().await.unwrap_or_default();
        
        println!("Join button state (attempt {}) - enabled: {}, displayed: {}, selected: {}, tag: {}", 
                join_attempts, is_enabled, is_displayed, is_selected, tag_name);
        
        // Get more detailed element info
        match join_button.rect().await {
            Ok(rect) => {
                println!("Button dimensions - x: {}, y: {}, width: {}, height: {}", 
                        rect.x, rect.y, rect.width, rect.height);
            }
            Err(e) => {
                println!("Could not get button dimensions: {}", e);
            }
        }
        
        // Check CSS properties that might block interaction
        let pointer_events = join_button.css_value("pointer-events").await.unwrap_or_default();
        let visibility = join_button.css_value("visibility").await.unwrap_or_default();
        let display = join_button.css_value("display").await.unwrap_or_default();
        let opacity = join_button.css_value("opacity").await.unwrap_or_default();
        let z_index = join_button.css_value("z-index").await.unwrap_or_default();
        
        println!("Button CSS - pointer-events: {}, visibility: {}, display: {}, opacity: {}, z-index: {}", 
                pointer_events, visibility, display, opacity, z_index);
        
        // Try scrolling the element into view first
        println!("Scrolling button into view...");
        let _ = driver.execute("arguments[0].scrollIntoView({behavior: 'instant', block: 'center'});", 
                              vec![join_button.to_json()?]).await;
        
        // Try to click the button multiple times before falling back to JavaScript
        let mut click_succeeded = false;
        for click_attempt in 1..=3 {
            println!("Trying standard click (attempt {} of 3)...", click_attempt);
            match join_button.click().await {
                Ok(_) => {
                    println!("Standard click succeeded!");
                    click_succeeded = true;
                    break;
                }
                Err(e) => {
                    println!("Standard click attempt {} failed: {}", click_attempt, e);
                    
                    // Try pressing Enter as alternative
                    println!("Trying Enter key press instead (main loop)...");
                    match join_button.send_keys("\n").await {
                        Ok(_) => {
                            println!("Enter key press succeeded in main loop!");
                            
                            // Wait and check if we're in the meeting
                            tokio::time::sleep(Duration::from_secs(3)).await;
                            
                            if check_if_in_meeting(&driver).await {
                                println!("Successfully joined meeting with Enter key in main loop!");
                                break 'outer;
                            } else {
                                println!("Enter key in main loop didn't work, continuing...");
                            }
                        }
                        Err(enter_e) => {
                            println!("Enter key press in main loop also failed: {}", enter_e);
                        }
                    }
                    
                    if click_attempt < 3 {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }
        
        if click_succeeded {
            // Wait and check if we're in the meeting
            tokio::time::sleep(Duration::from_secs(3)).await;
            
            if check_if_in_meeting(&driver).await {
                println!("Successfully joined meeting with standard click!");
                break;
            } else {
                println!("Standard click didn't work, continuing to retry...");
            }
        } else {
            println!("All standard clicks failed, trying JavaScript click...");
            
            // Try JavaScript click as fallback
            match driver.execute("arguments[0].click();", vec![join_button.to_json()?]).await {
                Ok(_) => {
                    println!("JavaScript click executed, waiting to see if it worked...");
                    
                    // Wait a moment and check if we're now in a meeting
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    
                    if check_if_in_meeting(&driver).await {
                        println!("Successfully joined meeting!");
                        break;
                    } else {
                        println!("JavaScript click didn't seem to work");
                        if join_attempts >= 10 {
                            println!("Too many failed attempts, giving up on Join button");
                            break;
                        }
                        println!("Retrying... (attempt {} of 10)", join_attempts + 1);
                        let _ = driver.enter_default_frame().await;
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
                Err(js_e) => {
                    println!("JavaScript click also failed ({})", js_e);
                    if join_attempts >= 10 {
                        println!("Too many failed attempts, giving up on Join button");
                        break;
                    }
                    println!("Retrying in 2 seconds... (attempt {} of 10)", join_attempts + 1);
                    let _ = driver.enter_default_frame().await; // Reset frame context
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    };
    
    println!("Automation complete!");
    Ok(())
}

async fn start_hid_controller(token: String, app_handle: Arc<tauri::AppHandle>, state: Arc<std::sync::Mutex<AutomationState>>) -> std::io::Result<()> {
    
    println!("Starting hid-recorder to discover devices...");
    
    // First run hid-recorder to discover devices
    let mut discovery_process = Command::new("hid-recorder")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    let stdout = discovery_process.stdout.take().expect("Failed to get stdout");
    let stderr = discovery_process.stderr.take().expect("Failed to get stderr");
    
    // Read from both stdout and stderr concurrently
    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = Vec::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("STDOUT: {}", line);
                lines.push(("stdout".to_string(), line));
            }
        }
        lines
    });
    
    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = Vec::new();
        for line in reader.lines() {
            if let Ok(line) = line {
                println!("STDERR: {}", line);
                lines.push(("stderr".to_string(), line));
            }
        }
        lines
    });
    
    // Wait a few seconds for output
    tokio::time::sleep(Duration::from_secs(3)).await;
    
    // Kill the process
    let _ = discovery_process.kill();
    
    // Collect all output
    let stdout_lines = stdout_task.await.unwrap_or_default();
    let stderr_lines = stderr_task.await.unwrap_or_default();
    
    let mut all_lines = Vec::new();
    all_lines.extend(stdout_lines);
    all_lines.extend(stderr_lines);
    
    let mut jabra_device_path = None;
    
    // Print all output for debugging
    println!("=== COMPLETE HID-RECORDER OUTPUT ===");
    for (source, line) in &all_lines {
        println!("{}: {}", source, line);
        
        // Look for Jabra device - any line containing Jabra
        if line.contains("Jabra") {
            println!("Found Jabra line: {}", line);
            // Extract device path - look for /dev/hidraw pattern
            if let Some(dev_start) = line.find("/dev/hidraw") {
                if let Some(colon_pos) = line[dev_start..].find(':') {
                    let device_part = &line[dev_start..dev_start + colon_pos];
                    jabra_device_path = Some(device_part.to_string());
                    println!("Found Jabra device: {}", device_part);
                } else {
                    // Maybe no colon, try to extract just the device path
                    let words: Vec<&str> = line[dev_start..].split_whitespace().collect();
                    if !words.is_empty() {
                        jabra_device_path = Some(words[0].to_string());
                        println!("Found Jabra device (no colon): {}", words[0]);
                    }
                }
            }
        }
    }
    println!("=== END HID-RECORDER OUTPUT ===");
    
    let jabra_path = jabra_device_path.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Jabra device not found")
    })?;
    
    println!("Starting hid-recorder with Jabra device: {}", jabra_path);
    
    // Now start hid-recorder with the specific device
    let mut hid_process = Command::new("hid-recorder")
        .arg(&jabra_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    
    let stdout = hid_process.stdout.take().expect("Failed to get stdout");
    let reader = BufReader::new(stdout);
    
    println!("HID recorder started, monitoring for signal: 3 03 01 00");
    
    for line in reader.lines() {
        let line = line?;
        
        // Check for the specific signal (ignore timestamp, just look for the data part)
        if line.contains("3 03 01 00") {
            let current_state = {
                let state_guard = state.lock().unwrap();
                *state_guard
            };
            
            println!("Signal detected! Current state: {:?}", current_state);
            
            match current_state {
                AutomationState::KioskMode => {
                    println!("Switching from Kiosk to Zoom mode...");
                    {
                        let mut state_guard = state.lock().unwrap();
                        *state_guard = AutomationState::ZoomRunning;
                    }
                    
                    // Hide the Tauri window
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    
                    // Start Zoom automation in background task
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        match start_chromium_controller().await {
                            Ok(_) => {
                                println!("Zoom automation completed successfully");
                                let mut state_guard = state_clone.lock().unwrap();
                                *state_guard = AutomationState::ZoomComplete;
                            }
                            Err(e) => {
                                println!("Zoom automation failed: {}", e);
                                let mut state_guard = state_clone.lock().unwrap();
                                *state_guard = AutomationState::KioskMode;
                            }
                        }
                    });
                }
                AutomationState::ZoomRunning => {
                    println!("Zoom automation already running, ignoring signal");
                }
                AutomationState::ZoomComplete => {
                    println!("Stopping Zoom and returning to Kiosk mode...");
                    {
                        let mut state_guard = state.lock().unwrap();
                        *state_guard = AutomationState::Stopping;
                    }
                    
                    // Kill Chrome processes and restart kiosk in background task
                    let state_clone = Arc::clone(&state);
                    let app_handle_clone = Arc::clone(&app_handle);
                    let token_clone = token.clone();
                    tokio::spawn(async move {
                        kill_chrome_processes().await;
                        println!("Chrome processes stopped, returning to kiosk mode");
                        let mut state_guard = state_clone.lock().unwrap();
                        *state_guard = AutomationState::KioskMode;
                        
                        // Show the Tauri window and restart kiosk mode
                        if let Some(window) = app_handle_clone.get_webview_window("main") {
                            let _ = window.show();
                        }
                        
                        // Start kiosk mode in background (this will loop indefinitely)
                        let state_for_kiosk = Arc::clone(&state_clone);
                        tokio::spawn(async move {
                            let _ = start_kiosk_mode(token_clone, app_handle_clone, state_for_kiosk).await;
                        });
                    });
                }
                AutomationState::Stopping => {
                    println!("Currently stopping processes, ignoring signal");
                }
            }
        }
    }
    
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_cursor_visible(false);
                let _ = window.show(); // Make sure window is visible for kiosk mode
            }

            let _app_handle = Arc::new(app.handle().clone());
            
            // Get CLI arguments
            let cli_matches = app.cli().matches();
            let _token = match cli_matches {
                Ok(matches) => {
                    match matches.args.get("token") {
                        Some(token_arg) => {
                            match &token_arg.value {
                                serde_json::Value::String(s) if !s.is_empty() => s.clone(),
                                serde_json::Value::Null => {
                                    // Try reading from /home/rctv/.rctvtoken file as fallback
                                    match std::fs::read_to_string("/home/rctv/.rctvtoken") {
                                        Ok(token_from_file) => token_from_file.trim().to_string(),
                                        Err(_) => {
                                            eprintln!("Error: --token argument is required or /home/rctv/.rctvtoken file must exist");
                                            std::process::exit(1);
                                        }
                                    }
                                }
                                _ => {
                                    eprintln!("Error: --token argument must be a string");
                                    std::process::exit(1);
                                }
                            }
                        }
                        None => {
                            // Try reading from /home/rctv/.rctvtoken file as fallback
                            match std::fs::read_to_string("/home/rctv/.rctvtoken") {
                                Ok(token_from_file) => token_from_file.trim().to_string(),
                                Err(_) => {
                                    eprintln!("Error: --token argument is required or /home/rctv/.rctvtoken file must exist");
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error parsing CLI arguments: {}", e);
                    std::process::exit(1);
                }
            };
            
            // Create shared state for both kiosk and HID controller
            let shared_state = Arc::new(std::sync::Mutex::new(AutomationState::KioskMode));
            
            // Start kiosk mode initially
            let kiosk_app_handle = Arc::clone(&_app_handle);
            let kiosk_token = _token.clone();
            let kiosk_state = Arc::clone(&shared_state);
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match start_kiosk_mode(kiosk_token, kiosk_app_handle, kiosk_state).await {
                        Ok(_) => println!("Kiosk mode started successfully"),
                        Err(e) => eprintln!("Failed to start kiosk mode: {}", e),
                    }
                });
            });
            
            // Start HID controller in background thread
            let hid_app_handle = Arc::clone(&_app_handle);
            let hid_token = _token.clone();
            let hid_state = Arc::clone(&shared_state);
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match start_hid_controller(hid_token, hid_app_handle, hid_state).await {
                        Ok(_) => println!("HID controller started successfully"),
                        Err(e) => eprintln!("Failed to start HID controller: {}", e),
                    }
                });
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
