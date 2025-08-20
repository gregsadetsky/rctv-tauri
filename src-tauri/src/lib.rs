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
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("chrome")
        .output();
    
    // Wait a moment for processes to die
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Start ChromeDriver process
    println!("Starting ChromeDriver...");
    let _chromedriver = Command::new("chromedriver")
        .arg("--port=9515")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start ChromeDriver - make sure it's installed");

    // Wait a moment for ChromeDriver to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Create Chrome capabilities - ChromeDriver will start Chrome automatically
    let mut caps = DesiredCapabilities::chrome();
    // Tell ChromeDriver where to find Chromium on Raspberry Pi
    caps.set_binary("/usr/bin/chromium-browser")?;
    // Enable real camera and microphone access
    caps.add_arg("--use-fake-ui-for-media-stream")?; // Auto-grant media permissions without user prompt
    caps.add_arg("--autoplay-policy=no-user-gesture-required")?; // Allow autoplay for media
    caps.add_arg("--user-data-dir=/home/rctv/.rctv-chrome-profile")?; // Persistent profile directory
    
    // Connect to ChromeDriver (which will automatically start Chrome)
    println!("Connecting to ChromeDriver and starting Chrome...");
    let driver = WebDriver::new("http://localhost:9515", caps).await?;
    
    // Navigate to example.com
    driver.goto("https://example.com").await?;
    
    println!("Successfully opened example.com in Chromium");
    
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
                                    // Try reading from /root/.rctvtoken file as fallback
                                    match std::fs::read_to_string("/root/.rctvtoken") {
                                        Ok(token_from_file) => token_from_file.trim().to_string(),
                                        Err(_) => {
                                            eprintln!("Error: --token argument is required or /root/.rctvtoken file must exist");
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
                            // Try reading from /root/.rctvtoken file as fallback
                            match std::fs::read_to_string("/root/.rctvtoken") {
                                Ok(token_from_file) => token_from_file.trim().to_string(),
                                Err(_) => {
                                    eprintln!("Error: --token argument is required or /root/.rctvtoken file must exist");
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
