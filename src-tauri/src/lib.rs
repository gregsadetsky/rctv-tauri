use std::sync::Arc;
use std::time::Duration;
use std::process::{Command, Stdio};
use tauri::Manager;
use tauri_plugin_cli::CliExt;
use thirtyfour::prelude::*;


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
        // Try to find join button in main page first
        let join_button_result = match driver.find(By::XPath("//button[contains(text(), 'Join')]")).await {
            Ok(element) => Ok(element),
            Err(_) => match driver.find(By::XPath("//input[@value='Join']")).await {
                Ok(element) => Ok(element),
                Err(_) => driver.find(By::XPath("//*[contains(text(), 'Join')]")).await,
            },
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
                                                let current_url = driver.current_url().await.map(|u| u.to_string()).unwrap_or_default();
                                                println!("Current URL after iframe click: {}", current_url);
                                                
                                                let in_meeting = current_url.contains("zoomgov.com") || 
                                                                current_url.contains("meeting") ||
                                                                current_url.contains("launch") ||
                                                                current_url.contains("/j/") ||
                                                                !current_url.contains("/wc/");
                                                
                                                if in_meeting {
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
                                                        let current_url = driver.current_url().await.map(|u| u.to_string()).unwrap_or_default();
                                                        println!("Current URL after Enter press: {}", current_url);
                                                        
                                                        let in_meeting = current_url.contains("zoomgov.com") || 
                                                                        current_url.contains("meeting") ||
                                                                        current_url.contains("launch") ||
                                                                        current_url.contains("/j/") ||
                                                                        !current_url.contains("/wc/");
                                                        
                                                        if in_meeting {
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
                                            let current_url = driver.current_url().await.map(|u| u.to_string()).unwrap_or_default();
                                            
                                            let in_meeting = current_url.contains("zoomgov.com") || 
                                                            current_url.contains("meeting") ||
                                                            current_url.contains("launch") ||
                                                            current_url.contains("/j/") ||
                                                            !current_url.contains("/wc/");
                                            
                                            if in_meeting {
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
                            let current_url = driver.current_url().await.map(|u| u.to_string()).unwrap_or_default();
                            println!("Current URL after Enter press (main): {}", current_url);
                            
                            let in_meeting = current_url.contains("zoomgov.com") || 
                                            current_url.contains("meeting") ||
                                            current_url.contains("launch") ||
                                            current_url.contains("/j/") ||
                                            !current_url.contains("/wc/");
                            
                            if in_meeting {
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
            let current_url = driver.current_url().await.map(|u| u.to_string()).unwrap_or_default();
            println!("Current URL after standard click: {}", current_url);
            
            let in_meeting = current_url.contains("zoomgov.com") || 
                            current_url.contains("meeting") ||
                            current_url.contains("launch") ||
                            current_url.contains("/j/") ||
                            !current_url.contains("/wc/");
            
            if in_meeting {
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
                    
                    // Wait a moment and check if we're now in a meeting (URL change, new elements, etc.)
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    let current_url = driver.current_url().await.map(|u| u.to_string()).unwrap_or_default();
                    println!("Current URL after click attempt: {}", current_url);
                    
                    // Check if we're now in the meeting (look for actual meeting indicators)
                    println!("Checking if we're actually in a meeting...");
                    
                    // Better indicators that we're in a meeting
                    let in_meeting = current_url.contains("zoomgov.com") || 
                                    current_url.contains("meeting") ||
                                    current_url.contains("launch") ||
                                    current_url.contains("/j/") ||
                                    !current_url.contains("/wc/"); // We've left the web client page
                    
                    println!("Meeting indicators - zoomgov: {}, meeting: {}, launch: {}, /j/: {}, left wc: {}", 
                            current_url.contains("zoomgov.com"),
                            current_url.contains("meeting"),
                            current_url.contains("launch"), 
                            current_url.contains("/j/"),
                            !current_url.contains("/wc/"));
                    
                    if in_meeting {
                        println!("Successfully joined meeting!");
                        break;
                    } else {
                        println!("JavaScript click didn't seem to work (URL unchanged)");
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
    
    // Keep the browser open
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_cursor_visible(true);
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
            
            // Start Chromium controller in background thread
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    match start_chromium_controller().await {
                        Ok(_) => println!("Chromium controller started successfully"),
                        Err(e) => eprintln!("Failed to start Chromium controller: {}", e),
                    }
                });
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
