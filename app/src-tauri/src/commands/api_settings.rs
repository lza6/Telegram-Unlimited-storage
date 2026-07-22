use serde::{Deserialize, Serialize};
#[cfg(feature = "desktop")]
use std::path::PathBuf;
#[cfg(feature = "desktop")]
use tauri::{AppHandle, Manager};

/// Persisted API settings (written to api_settings.json in the app data dir)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiSettingsFile {
    pub enabled: bool,
    pub port: u16,
    pub key_hash: Option<String>,
    /// Local-only admin password for `X-Access-Pwd` on 127.0.0.1 REST (auto-generated on first enable).
    #[serde(default)]
    pub local_access_pwd: Option<String>,
}

impl Default for ApiSettingsFile {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8550,
            key_hash: None,
            local_access_pwd: None,
        }
    }
}

/// What the frontend sees (never exposes the hash)
#[cfg(feature = "desktop")]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiSettingsResponse {
    pub enabled: bool,
    pub port: u16,
    pub key_set: bool,
    pub running: bool,
    /// Plaintext local admin pwd for curl/scripts (127.0.0.1 only).
    pub local_access_pwd: Option<String>,
}

#[cfg(feature = "desktop")]
fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("api_settings.json"))
}

pub fn load_settings_at(data_dir: &std::path::Path) -> ApiSettingsFile {
    let path = data_dir.join("api_settings.json");
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => ApiSettingsFile::default(),
    }
}

pub fn save_settings_at(
    data_dir: &std::path::Path,
    settings: &ApiSettingsFile,
) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = data_dir.join("api_settings.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[cfg(feature = "desktop")]
pub fn load_settings(app: &AppHandle) -> ApiSettingsFile {
    let path = match settings_path(app) {
        Ok(p) => p,
        Err(_) => return ApiSettingsFile::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => ApiSettingsFile::default(),
    }
}

#[cfg(feature = "desktop")]
fn save_settings(app: &AppHandle, settings: &ApiSettingsFile) -> Result<(), String> {
    let path = settings_path(app)?;
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn hash_key_public(key: &str) -> String {
    crate::password_kdf::hash_api_key(key)
}

/// Verify a plaintext key against a stored hash
pub fn verify_key(plaintext: &str, stored_hash: &str) -> bool {
    crate::password_kdf::verify_api_key_legacy(plaintext, stored_hash)
}

/// Verify and report whether the stored hash should be migrated to Argon2id.
pub fn verify_key_with_upgrade(plaintext: &str, stored_hash: &str) -> (bool, bool) {
    crate::password_kdf::verify_api_key(plaintext, stored_hash)
}

/// Verify an API key and, if it uses a legacy SHA-256 hash, upgrade it in
/// `api_settings.json` to Argon2id. Returns whether the key is valid.
pub fn verify_and_upgrade_key(
    plaintext: &str,
    stored_hash: &str,
    data_dir: &std::path::Path,
) -> bool {
    let (valid, should_upgrade) = verify_key_with_upgrade(plaintext, stored_hash);
    if valid && should_upgrade {
        let new_hash = hash_key_public(plaintext);
        if let Err(e) = upgrade_key_hash_in_settings(data_dir, &new_hash) {
            log::warn!("Failed to upgrade API key hash in settings: {e}");
        } else {
            log::info!("Upgraded API key hash to Argon2id in api_settings.json");
        }
    }
    valid
}

/// Write an upgraded API key hash back to `api_settings.json`.
pub fn upgrade_key_hash_in_settings(
    data_dir: &std::path::Path,
    new_hash: &str,
) -> Result<(), String> {
    let mut settings = load_settings_at(data_dir);
    if settings
        .key_hash
        .as_ref()
        .is_some_and(|h| h.starts_with(crate::password_kdf::ARGON2_MARKER))
    {
        return Ok(());
    }
    settings.key_hash = Some(new_hash.to_string());
    save_settings_at(data_dir, &settings)
}

pub fn ensure_local_access_pwd(settings: &mut ApiSettingsFile) {
    if settings
        .local_access_pwd
        .as_ref()
        .is_some_and(|s| !s.is_empty())
    {
        return;
    }
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..16).map(|_| rand::Rng::gen(&mut rng)).collect();
    settings.local_access_pwd = Some(bytes.iter().map(|b| format!("{:02x}", b)).collect());
}

/// Resolve desktop REST `X-Access-Pwd` from api_settings or ACCESS_PWD env.
pub fn load_local_access_pwd(data_dir: &std::path::Path) -> String {
    load_settings_at(data_dir)
        .local_access_pwd
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("ACCESS_PWD").ok())
        .unwrap_or_default()
}

#[cfg(feature = "desktop")]
fn wait_for_api_server(app: &AppHandle, want_running: bool) {
    if !want_running {
        return;
    }
    if let Some(state) = app.try_state::<crate::ApiServerRunning>() {
        for _ in 0..30 {
            if state.0.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(feature = "desktop")]
pub fn prepare_settings_for_runtime(app: &AppHandle) -> ApiSettingsFile {
    let mut settings = load_settings(app);
    if settings.enabled {
        let missing = settings
            .local_access_pwd
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if missing {
            ensure_local_access_pwd(&mut settings);
            let _ = save_settings(app, &settings);
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::Digest;

    #[test]
    fn hash_and_verify_roundtrip() {
        let key = "test-api-key-hex";
        let h = hash_key_public(key);
        assert!(h.starts_with("$argon2"));
        assert!(verify_key(key, &h));
        assert!(!verify_key("wrong", &h));
    }

    #[test]
    fn default_settings_file() {
        let s = ApiSettingsFile::default();
        assert!(!s.enabled);
        assert_eq!(s.port, 8550);
        assert!(s.local_access_pwd.is_none());
    }

    #[test]
    fn verify_key_with_upgrade_detects_legacy_hash() {
        let key = "legacy-key";
        // Simulate a legacy SHA-256 hex hash (no argon2 marker)
        let mut hasher = sha2::Sha256::new();
        hasher.update(key.as_bytes());
        let legacy_hash = format!("{:x}", hasher.finalize());

        let (valid, should_upgrade) = verify_key_with_upgrade(key, &legacy_hash);
        assert!(valid);
        assert!(should_upgrade);

        let (wrong_valid, _) = verify_key_with_upgrade("wrong", &legacy_hash);
        assert!(!wrong_valid);
    }

    #[test]
    fn verify_key_with_upgrade_argon2_does_not_upgrade() {
        let key = "argon2-key";
        let h = hash_key_public(key);
        let (valid, should_upgrade) = verify_key_with_upgrade(key, &h);
        assert!(valid);
        assert!(!should_upgrade);
    }

    #[test]
    fn upgrade_key_hash_in_settings_migrates_legacy_hash() {
        let dir = tempfile::tempdir().unwrap();
        let key = "legacy-key";
        let mut hasher = sha2::Sha256::new();
        hasher.update(key.as_bytes());
        let legacy_hash = format!("{:x}", hasher.finalize());

        let mut settings = ApiSettingsFile::default();
        settings.key_hash = Some(legacy_hash);
        save_settings_at(dir.path(), &settings).unwrap();

        let new_hash = hash_key_public(key);
        upgrade_key_hash_in_settings(dir.path(), &new_hash).unwrap();

        let reloaded = load_settings_at(dir.path());
        assert_eq!(reloaded.key_hash.as_deref(), Some(new_hash.as_str()));
        assert!(reloaded.key_hash.as_deref().unwrap().starts_with("$argon2"));
    }

    #[test]
    fn upgrade_key_hash_in_settings_skips_already_argon2() {
        let dir = tempfile::tempdir().unwrap();
        let key = "argon2-key";
        let argon2_hash = hash_key_public(key);

        let mut settings = ApiSettingsFile::default();
        settings.key_hash = Some(argon2_hash.clone());
        save_settings_at(dir.path(), &settings).unwrap();

        let another_argon2 = hash_key_public("different-key");
        upgrade_key_hash_in_settings(dir.path(), &another_argon2).unwrap();

        let reloaded = load_settings_at(dir.path());
        // Should keep the original Argon2 hash, not overwrite with different key's hash
        assert_eq!(reloaded.key_hash.as_deref(), Some(argon2_hash.as_str()));
    }

    #[test]
    fn verify_and_upgrade_key_migrates_legacy_hash() {
        let dir = tempfile::tempdir().unwrap();
        let key = "legacy-key";
        let mut hasher = sha2::Sha256::new();
        hasher.update(key.as_bytes());
        let legacy_hash = format!("{:x}", hasher.finalize());

        let mut settings = ApiSettingsFile::default();
        settings.key_hash = Some(legacy_hash.clone());
        save_settings_at(dir.path(), &settings).unwrap();

        assert!(verify_and_upgrade_key(key, &legacy_hash, dir.path()));

        let reloaded = load_settings_at(dir.path());
        assert!(reloaded
            .key_hash
            .as_deref()
            .unwrap()
            .starts_with(crate::password_kdf::ARGON2_MARKER));
    }

    #[test]
    fn verify_and_upgrade_key_rejects_invalid_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = "legacy-key";
        let mut hasher = sha2::Sha256::new();
        hasher.update(key.as_bytes());
        let legacy_hash = format!("{:x}", hasher.finalize());

        let mut settings = ApiSettingsFile::default();
        settings.key_hash = Some(legacy_hash.clone());
        save_settings_at(dir.path(), &settings).unwrap();

        assert!(!verify_and_upgrade_key("wrong", &legacy_hash, dir.path()));

        let reloaded = load_settings_at(dir.path());
        // Invalid key should not trigger an upgrade
        assert_eq!(reloaded.key_hash.as_deref(), Some(legacy_hash.as_str()));
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_get_api_settings(app: AppHandle) -> Result<ApiSettingsResponse, String> {
    let settings = load_settings(&app);
    let running = {
        let state = app.try_state::<crate::ApiServerRunning>();
        state
            .map(|s| s.0.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    };
    Ok(ApiSettingsResponse {
        enabled: settings.enabled,
        port: settings.port,
        key_set: settings.key_hash.is_some(),
        running,
        local_access_pwd: if settings.enabled {
            settings.local_access_pwd.clone()
        } else {
            None
        },
    })
}

#[cfg(feature = "desktop")]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiHealthSnapshot {
    pub status: String,
    pub version: String,
    pub telegram_connected: bool,
    pub ready: bool,
    pub transport_mode: String,
    pub upload_queue: serde_json::Value,
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_get_api_health(app: AppHandle) -> Result<ApiHealthSnapshot, String> {
    let settings = load_settings(&app);
    let running = app
        .try_state::<crate::ApiServerRunning>()
        .map(|s| s.0.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false);
    if !running {
        return Err("API server is not running".to_string());
    }
    let url = format!("http://127.0.0.1:{}/api/v1/health", settings.port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("health fetch failed: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(ApiHealthSnapshot {
        status: body
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        version: body
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        telegram_connected: body
            .get("telegram_connected")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ready: body.get("ready").and_then(|v| v.as_bool()).unwrap_or(false),
        transport_mode: body
            .get("transport_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        upload_queue: body
            .get("upload_queue")
            .cloned()
            .unwrap_or(serde_json::json!({})),
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_update_api_settings(
    enabled: bool,
    port: u16,
    app: AppHandle,
) -> Result<ApiSettingsResponse, String> {
    // Validate port range
    if port < 1024 {
        return Err("Port must be 1024 or higher".to_string());
    }

    // Prevent collision with streaming server
    if port == crate::STREAM_PORT {
        return Err(format!(
            "Port {} is used by the media streaming server",
            port
        ));
    }

    let mut settings = load_settings(&app);
    let port_changed = settings.port != port;
    let enabled_changed = settings.enabled != enabled;

    settings.enabled = enabled;
    settings.port = port;
    if settings.enabled {
        ensure_local_access_pwd(&mut settings);
    }
    save_settings(&app, &settings)?;

    // Restart server if anything changed
    if port_changed || enabled_changed {
        crate::restart_api_server(&app);
        wait_for_api_server(&app, settings.enabled);
    }

    let running = {
        let state = app.try_state::<crate::ApiServerRunning>();
        state
            .map(|s| s.0.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
    };

    Ok(ApiSettingsResponse {
        enabled: settings.enabled,
        port: settings.port,
        key_set: settings.key_hash.is_some(),
        running,
        local_access_pwd: if settings.enabled {
            settings.local_access_pwd.clone()
        } else {
            None
        },
    })
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_regenerate_api_key(app: AppHandle) -> Result<String, String> {
    let mut settings = load_settings(&app);

    // Generate a secure 32-byte random key as hex
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rand::Rng::gen(&mut rng)).collect();
    let plaintext_key: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();

    // Store only the hash
    settings.key_hash = Some(hash_key_public(&plaintext_key));
    save_settings(&app, &settings)?;

    crate::restart_api_server(&app);
    wait_for_api_server(&app, settings.enabled);

    Ok(plaintext_key)
}

#[cfg(feature = "desktop")]
#[tauri::command]
pub async fn cmd_regenerate_local_access_pwd(app: AppHandle) -> Result<String, String> {
    let mut settings = load_settings(&app);
    if !settings.enabled {
        return Err("Enable the API server first".into());
    }
    settings.local_access_pwd = None;
    ensure_local_access_pwd(&mut settings);
    let pwd = settings
        .local_access_pwd
        .clone()
        .ok_or_else(|| "Failed to generate local access password".to_string())?;
    save_settings(&app, &settings)?;
    crate::restart_api_server(&app);
    wait_for_api_server(&app, true);
    Ok(pwd)
}
