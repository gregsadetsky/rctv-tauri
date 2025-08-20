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
                let _ = window.set_cursor_visible(true);
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
            
            // Navigate to specific Zoom URL
            if let Some(window) = app.get_webview_window("main") {
                // Enable camera and microphone permissions
                let _ = window.eval(r#"
                    navigator.mediaDevices.getUserMedia({video: true, audio: true})
                        .then(function(stream) {
                            console.log('Media permissions granted');
                        })
                        .catch(function(err) {
                            console.log('Media permissions denied:', err);
                        });
                "#);
                
                let zoom_url = "https://app.zoom.us/wc/2125949362/join?fromPWA=1&pwd=OEJ3Nkw4djlmSlBBVWl2aVdXTk93Zz09";
                if let Ok(parsed_url) = Url::parse(zoom_url) {
                    let _ = window.navigate(parsed_url);
                }
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
