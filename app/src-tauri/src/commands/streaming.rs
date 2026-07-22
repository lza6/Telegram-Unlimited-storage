use tauri::{AppHandle, Manager, State};

/// Holds the per-session streaming config (token + port)
pub struct StreamConfig {
    pub token: String,
    pub port: u16,
}

/// Returned to the frontend so it can construct stream URLs dynamically
#[derive(serde::Serialize)]
pub struct StreamInfo {
    pub token: String,
    pub base_url: String,
}

/// Returns the streaming server's session token and base URL to the frontend.
/// Uses ui_settings.share_domain when set (same strategy as share links).
#[tauri::command]
pub fn cmd_get_stream_info(
    app: AppHandle,
    config: State<'_, StreamConfig>,
) -> Result<StreamInfo, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let base_url = crate::ui_settings::share_base_url_from_data_dir(&dir, config.port);
    Ok(StreamInfo {
        token: config.token.clone(),
        base_url,
    })
}
