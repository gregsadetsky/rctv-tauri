use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{Manager};
use tauri_plugin_cli::CliExt;
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

async fn fetch_apps(token: &str) -> Result<Vec<App>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://rctv.recurse.com/get_all_apps_for_tauri?tv_login_token={}", token);
    let response = reqwest::get(&url).await?;
    let api_response: ApiResponse = response.json().await?;
    Ok(api_response.apps)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            let app_handle = Arc::new(app.handle().clone());
            
            // Get CLI arguments
            let cli_matches = app.cli().matches();
            let token = match cli_matches {
                Ok(matches) => {
                    match matches.args.get("token") {
                        Some(token_arg) => {
                            match &token_arg.value {
                                serde_json::Value::String(s) => s.clone(),
                                _ => {
                                    eprintln!("Error: --token argument must be a string");
                                    std::process::exit(1);
                                }
                            }
                        }
                        None => {
                            eprintln!("Error: --token argument is required");
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error parsing CLI arguments: {}", e);
                    std::process::exit(1);
                }
            };
            
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let mut last_update_check = Instant::now();
                    let update_check_interval = Duration::from_secs(12 * 60 * 60); // 12 hours
                    
                    loop {
                        // Check for updates every 12 hours
                        if last_update_check.elapsed() >= update_check_interval {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                // Emit an event to trigger update check in frontend
                                let _ = window.emit("check-for-updates", ());
                            }
                            last_update_check = Instant::now();
                        }
                        
                        match fetch_apps(&token).await {
                            Ok(apps) => {
                                if !apps.is_empty() {
                                    for app in &apps {
                                        if let Some(window) = app_handle.get_webview_window("main") {
                                            if let Ok(parsed_url) = Url::parse(&app.url) {
                                                let _ = window.navigate(parsed_url);
                                            }
                                        }
                                        tokio::time::sleep(Duration::from_secs(app.on_screen_duration_seconds)).await;
                                        
                                        // Check if it's time for an update check during URL cycling
                                        if last_update_check.elapsed() >= update_check_interval {
                                            break; // Break out of app loop to check for updates
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to fetch apps: {}", e);
                                tokio::time::sleep(Duration::from_secs(30)).await;
                            }
                        }
                    }
                });
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
