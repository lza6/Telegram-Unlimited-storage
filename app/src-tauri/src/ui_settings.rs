//! Web/admin UI settings persisted beside server data (share domain override, etc.).

use actix_web::HttpRequest;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::server_config::ServerConfig;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiSettings {
    /// Host override for share/upload links, e.g. `100.x.x.x:1334` or `https://drive.example.com`
    #[serde(default)]
    pub share_domain: String,
}

pub fn ui_settings_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("ui_settings.json")
}

pub fn load_ui_settings(data_dir: &Path) -> UiSettings {
    let path = ui_settings_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => UiSettings::default(),
    }
}

pub fn save_ui_settings(data_dir: &Path, settings: &UiSettings) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = ui_settings_path(data_dir);
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// Resolve public base URL: ui_settings.share_domain > BASE_URL > request Host.
pub fn effective_base_url(req: &HttpRequest, config: &ServerConfig) -> String {    let ui = load_ui_settings(&config.data_dir);
    let override_domain = ui.share_domain.trim();
    if !override_domain.is_empty() {
        if override_domain.starts_with("http://") || override_domain.starts_with("https://") {
            return override_domain.trim_end_matches('/').to_string();
        }
        let conn = req.connection_info();
        let scheme = req
            .headers()
            .get("X-Forwarded-Proto")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(conn.scheme());
        return format!("{}://{}", scheme, override_domain.trim_end_matches('/'));
    }

    let conn = req.connection_info();
    let scheme = req
        .headers()
        .get("X-Forwarded-Proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(conn.scheme());
    if config.base_url.is_empty() {
        format!("{}://{}", scheme, conn.host())
    } else {
        config.base_url.trim_end_matches('/').to_string()
    }
}

/// Base URL for share links on desktop (no HTTP request). Uses ui_settings.share_domain or stream port.
pub fn share_base_url_from_data_dir(data_dir: &Path, stream_port: u16) -> String {
    let ui = load_ui_settings(data_dir);
    let domain = ui.share_domain.trim();
    if domain.is_empty() {
        return format!("http://127.0.0.1:{stream_port}");
    }
    if domain.starts_with("http://") || domain.starts_with("https://") {
        return domain.trim_end_matches('/').to_string();
    }
    format!("http://{}", domain.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_ui_settings() {
        let dir = std::env::temp_dir().join(format!("td-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = UiSettings {
            share_domain: "100.1.2.3:1334".into(),
        };
        save_ui_settings(&dir, &s).unwrap();
        let loaded = load_ui_settings(&dir);
        assert_eq!(loaded.share_domain, "100.1.2.3:1334");
    }

    #[test]
    fn share_base_url_uses_domain_or_stream_port() {
        let dir = std::env::temp_dir().join(format!("td-ui-base-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            share_base_url_from_data_dir(&dir, 14201),
            "http://127.0.0.1:14201"
        );
        save_ui_settings(
            &dir,
            &UiSettings {
                share_domain: "100.0.0.5:14201".into(),
            },
        )
        .unwrap();
        assert_eq!(
            share_base_url_from_data_dir(&dir, 14201),
            "http://100.0.0.5:14201"
        );
    }
}
