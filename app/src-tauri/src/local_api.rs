//! Desktop → local headless REST bridge (Bot / asset-index mode).
//! No Telegram GramJS client required when `asset_index_authoritative`.

#[cfg(not(feature = "headless-server"))]
use tauri::Manager;

pub struct LocalApiBridge {
    pub port: u16,
    pub access_pwd: String,
}

impl LocalApiBridge {
    #[cfg(not(feature = "headless-server"))]
    pub fn from_data_dir(data_dir: &std::path::Path) -> Self {
        let settings = crate::commands::api_settings::load_settings_at(data_dir);
        Self {
            port: settings.port,
            access_pwd: crate::commands::api_settings::load_local_access_pwd(data_dir),
        }
    }

    #[cfg(feature = "headless-server")]
    pub fn from_data_dir(_data_dir: &std::path::Path) -> Self {
        Self {
            port: 0,
            access_pwd: String::new(),
        }
    }

    pub fn is_usable(&self) -> bool {
        !self.access_pwd.is_empty()
    }
}

pub fn build_download_url(port: u16, message_id: i32, folder_id: Option<i64>) -> String {
    let mut url = format!("http://127.0.0.1:{port}/api/v1/files/{message_id}/download");
    if let Some(fid) = folder_id {
        url.push_str(&format!("?folder_id={fid}"));
    }
    url
}

pub fn extension_from_filename(name: &str) -> String {
    let ext = std::path::Path::new(name)
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    if ext.is_empty() {
        "bin".to_string()
    } else {
        ext
    }
}

#[cfg(not(feature = "headless-server"))]
pub async fn desktop_uses_asset_index(
    app: &tauri::AppHandle,
    db_pool: &crate::db::DbConnection,
) -> Result<bool, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let settings = crate::commands::api_settings::load_settings_at(&data_dir);
    let config = crate::server_config::for_desktop_api(
        data_dir.clone(),
        settings.port,
        settings.key_hash.clone(),
        crate::STREAM_PORT,
        None,
    );
    let transport = crate::telegram_transport::TransportHandle::new(
        &config.data_dir,
        config.default_transport_mode,
    );
    let mode = transport.effective_mode(&config).await;
    Ok(crate::file_access::asset_index_authoritative(
        mode, &config, db_pool,
    ))
}

#[cfg(not(feature = "headless-server"))]
pub async fn fetch_file_to_path(
    bridge: &LocalApiBridge,
    message_id: i32,
    folder_id: Option<i64>,
    save_path: &str,
    bw_state: &crate::bandwidth::BandwidthManager,
) -> Result<u64, String> {
    use futures_util::StreamExt;

    if !bridge.is_usable() {
        return Err("Local API access password is not configured".to_string());
    }

    let url = build_download_url(bridge.port, message_id, folder_id);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(&url)
        .header("X-Access-Pwd", &bridge.access_pwd)
        .send()
        .await
        .map_err(|e| format!("API download request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API download failed ({status}): {body}"));
    }

    let total_size = response.content_length().unwrap_or(0);
    if total_size > 0 {
        bw_state.can_transfer(total_size).await?;
    }

    let mut file = tokio::fs::File::create(save_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        let bytes = chunk_result.map_err(|e| format!("API download stream error: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &bytes)
            .await
            .map_err(|e| e.to_string())?;
        downloaded += bytes.len() as u64;
    }

    let counted = if total_size > 0 {
        total_size
    } else {
        downloaded
    };
    bw_state.add_down(counted).await;
    Ok(counted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_download_url_with_folder() {
        assert_eq!(
            build_download_url(8550, 42, None),
            "http://127.0.0.1:8550/api/v1/files/42/download",
        );
        assert_eq!(
            build_download_url(9090, 7, Some(100)),
            "http://127.0.0.1:9090/api/v1/files/7/download?folder_id=100",
        );
    }

    #[test]
    fn extension_from_filename_handles_missing_ext() {
        assert_eq!(extension_from_filename("photo.jpg"), "jpg");
        assert_eq!(extension_from_filename("noext"), "bin");
    }
}
