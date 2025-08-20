use std::sync::Arc;
use std::time::Duration;
use tauri::Manager;
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
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_cursor_visible(false);
            }

            let app_handle = Arc::new(app.handle().clone());
            
            // Get CLI arguments
            let cli_matches = app.cli().matches();
            let token = match cli_matches {
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
            
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    loop {
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
                                    }
                                } else {
                                    // If no apps, wait a bit before trying again
                                    tokio::time::sleep(Duration::from_secs(60)).await;
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
