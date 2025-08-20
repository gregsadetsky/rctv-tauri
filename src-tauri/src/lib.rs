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
        .arg("--enable-logging")
        .arg("--v=1")
        .stdout(Stdio::inherit()) // Show Chromium output
        .stderr(Stdio::inherit())
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
        .arg("--verbose")
        .arg("--whitelisted-ips=")
        .stdout(Stdio::inherit()) // Show ChromeDriver output
        .stderr(Stdio::inherit())
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
    
    // Wait for iframe to load and switch to it
    tokio::time::sleep(Duration::from_secs(3)).await;
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
    
    // Step 4: Wait for and click "Join" button with retries
    println!("Looking for Join button...");
    let join_button = loop {
        match driver.find(By::XPath("//button[contains(text(), 'Join')]")).await {
            Ok(element) => break element,
            Err(_) => {
                match driver.find(By::XPath("//input[@value='Join']")).await {
                    Ok(element) => break element,
                    Err(_) => {
                        match driver.find(By::XPath("//*[contains(text(), 'Join')]")).await {
                            Ok(element) => break element,
                            Err(_) => {
                                println!("Join button not found, retrying in 2 seconds...");
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        }
                    }
                }
            }
        }
    };
    join_button.click().await?;
    println!("Clicked Join button - should now be in the meeting!");
    
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
