use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::db::DbConnection;
use crate::server_config::ServerConfig;

use crate::commands::TelegramState;
use crate::vpn_optimizer::NetworkConfig;

const BOT_SINGLE_FILE_MAX: usize = 20 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TelegramTransportMode {
    Bot,
    User,
}

impl TelegramTransportMode {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_lowercase().as_str() {
            "bot" => Some(Self::Bot),
            "user" | "app" | "application" => Some(Self::User),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bot => "bot",
            Self::User => "user",
        }
    }
}

#[derive(Clone)]
pub struct TransportHandle {
    active: Arc<RwLock<TelegramTransportMode>>,
    data_dir: PathBuf,
    default_mode: TelegramTransportMode,
}

#[derive(Serialize, Deserialize)]
struct PersistedTransportMode {
    mode: String,
}

impl TransportHandle {
    pub fn new(data_dir: &Path, default_mode: TelegramTransportMode) -> Self {
        let active_mode = load_persisted_mode(data_dir).unwrap_or(default_mode);
        Self {
            active: Arc::new(RwLock::new(active_mode)),
            data_dir: data_dir.to_path_buf(),
            default_mode,
        }
    }

    pub async fn active_mode(&self) -> TelegramTransportMode {
        // Desktop runs REST (8550) and stream (14201) servers in separate threads;
        // reload persisted mode so transport changes on one port apply to both.
        if let Some(mode) = load_persisted_mode(&self.data_dir) {
            let mut guard = self.active.write().await;
            *guard = mode;
        }
        *self.active.read().await
    }

    pub async fn set_mode(&self, mode: TelegramTransportMode) -> Result<(), String> {
        {
            let mut guard = self.active.write().await;
            *guard = mode;
        }
        persist_mode(&self.data_dir, mode)
    }

    pub fn default_mode(&self) -> TelegramTransportMode {
        self.default_mode
    }

    pub fn bot_configured(config: &ServerConfig) -> bool {
        !config.all_bot_tokens().is_empty()
            && config
                .storage_channel_id
                .as_ref()
                .is_some_and(|c| !c.is_empty())
    }

    pub fn user_configured(config: &ServerConfig) -> bool {
        config.telegram_credentials_placeholder().is_none()
    }

    pub async fn effective_mode(&self, config: &ServerConfig) -> TelegramTransportMode {
        let requested = self.active_mode().await;
        match requested {
            TelegramTransportMode::Bot if Self::bot_configured(config) => {
                TelegramTransportMode::Bot
            }
            TelegramTransportMode::User if Self::user_configured(config) => {
                TelegramTransportMode::User
            }
            TelegramTransportMode::Bot => {
                if Self::user_configured(config) {
                    log::warn!(
                        "Bot mode requested but TG_BOT_TOKEN / TG_STORAGE_CHANNEL_ID missing — falling back to user mode"
                    );
                    TelegramTransportMode::User
                } else {
                    TelegramTransportMode::Bot
                }
            }
            TelegramTransportMode::User => {
                if Self::bot_configured(config) {
                    log::warn!(
                        "User mode requested but TELEGRAM_API_ID/HASH invalid — falling back to bot mode"
                    );
                    TelegramTransportMode::Bot
                } else {
                    TelegramTransportMode::User
                }
            }
        }
    }
}

fn load_persisted_mode(data_dir: &Path) -> Option<TelegramTransportMode> {
    let path = data_dir.join("transport_mode.json");
    let raw = std::fs::read_to_string(path).ok()?;
    let parsed: PersistedTransportMode = serde_json::from_str(&raw).ok()?;
    TelegramTransportMode::parse(&parsed.mode)
}

fn persist_mode(data_dir: &Path, mode: TelegramTransportMode) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = data_dir.join("transport_mode.json");
    let body = serde_json::to_string_pretty(&PersistedTransportMode {
        mode: mode.as_str().to_string(),
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct TgApiResponse<T> {
    ok: bool,
    description: Option<String>,
    result: Option<T>,
}

#[derive(Deserialize)]
struct TgUser {
    username: Option<String>,
    first_name: Option<String>,
}

#[derive(Deserialize)]
struct TgDocument {
    file_id: String,
    file_name: Option<String>,
    file_size: Option<i64>,
    mime_type: Option<String>,
}

#[derive(Deserialize)]
struct TgMessage {
    message_id: i32,
    document: Option<TgDocument>,
}

#[derive(Deserialize)]
struct TgFile {
    file_path: String,
    file_size: Option<i64>,
}

pub struct BotUploadResult {
    pub message_id: i32,
    pub telegram_file_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: String,
}

fn bot_api_url_with_token(config: &ServerConfig, token: &str, method: &str) -> String {
    let base = config.bot_api_base.trim_end_matches('/').to_string();
    format!("{base}/bot{token}/{method}")
}

fn resolve_bot_token_for_pool_index(
    config: &ServerConfig,
    pool_index: u32,
) -> Result<String, String> {
    let pool = crate::bot_pool::BotPool::from_config(config);
    pool.token_at(pool_index)
        .map(|s| s.to_string())
        .or_else(|| pool.token_at(0).map(|s| s.to_string()))
        .ok_or_else(|| "TG_BOT_TOKEN is not configured".to_string())
}

fn bot_file_url_with_token(config: &ServerConfig, token: &str, file_path: &str) -> String {
    let base = config.bot_api_base.trim_end_matches('/').to_string();
    let path = file_path.trim_start_matches('/');
    format!("{base}/file/bot{token}/{path}")
}

pub async fn bot_test_connection(config: &ServerConfig) -> Result<String, String> {
    validate_bot_config(config)?;
    let pool = crate::bot_pool::BotPool::from_config(config);
    if pool.len() > 1 {
        log::info!(
            "Bot pool: {} tokens configured for round-robin uploads",
            pool.len()
        );
    }
    let token = pool
        .token_at(0)
        .ok_or_else(|| "TG_BOT_TOKEN is not configured".to_string())?
        .to_string();
    let url = bot_api_url_with_token(config, &token, "getMe");
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Bot getMe failed: {e}"))?;
    let json: TgApiResponse<TgUser> = resp
        .json()
        .await
        .map_err(|e| format!("Bot getMe parse failed: {e}"))?;
    if !json.ok {
        return Err(json
            .description
            .unwrap_or_else(|| "Bot getMe rejected".to_string()));
    }
    let user = json
        .result
        .ok_or_else(|| "Bot getMe returned empty result".to_string())?;
    Ok(user
        .username
        .or(user.first_name)
        .unwrap_or_else(|| "bot".to_string()))
}

/// Debounced Bot getMe probe — avoids flaky `ready=false` on every health poll.
const BOT_PROBE_CACHE_TTL: Duration = Duration::from_secs(30);
const BOT_PROBE_MAX_CONSECUTIVE_FAILS: u32 = 3;

#[derive(Default)]
struct BotProbeCacheState {
    last_success_at: Option<Instant>,
    last_username: Option<String>,
    consecutive_failures: u32,
}

pub struct BotProbeCache {
    inner: RwLock<BotProbeCacheState>,
}

impl Default for BotProbeCache {
    fn default() -> Self {
        Self {
            inner: RwLock::new(BotProbeCacheState::default()),
        }
    }
}

impl BotProbeCache {
    fn fresh_username(&self, state: &BotProbeCacheState) -> Option<String> {
        let at = state.last_success_at?;
        if at.elapsed() <= BOT_PROBE_CACHE_TTL {
            state.last_username.clone()
        } else {
            None
        }
    }

    fn stale_ok_state(&self, state: &BotProbeCacheState) -> bool {
        let Some(at) = state.last_success_at else {
            return false;
        };
        at.elapsed() <= BOT_PROBE_CACHE_TTL
            && state.consecutive_failures < BOT_PROBE_MAX_CONSECUTIVE_FAILS
    }

    pub async fn record_success(&self, username: String) {
        let mut state = self.inner.write().await;
        state.last_success_at = Some(Instant::now());
        state.last_username = Some(username);
        state.consecutive_failures = 0;
    }

    pub async fn record_failure(&self) {
        let mut state = self.inner.write().await;
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
    }

    pub async fn cached_username_if_fresh(&self) -> Option<String> {
        let state = self.inner.read().await;
        self.fresh_username(&state)
    }

    pub async fn stale_ok(&self) -> bool {
        let state = self.inner.read().await;
        self.stale_ok_state(&state)
    }

    pub async fn consecutive_failures(&self) -> u32 {
        self.inner.read().await.consecutive_failures
    }
}

static BOT_PROBE_CACHE: std::sync::OnceLock<BotProbeCache> = std::sync::OnceLock::new();

fn bot_probe_cache() -> &'static BotProbeCache {
    BOT_PROBE_CACHE.get_or_init(BotProbeCache::default)
}

/// Cached Bot getMe — returns fresh cache within 30s; on transient failure uses stale-while-revalidate.
pub async fn bot_test_connection_cached(config: &ServerConfig) -> Result<String, String> {
    let cache = bot_probe_cache();
    if let Some(username) = cache.cached_username_if_fresh().await {
        return Ok(username);
    }
    match bot_test_connection(config).await {
        Ok(username) => {
            cache.record_success(username.clone()).await;
            Ok(username)
        }
        Err(e) => {
            cache.record_failure().await;
            if cache.stale_ok().await {
                if let Some(username) = cache.cached_username_if_fresh().await {
                    log::warn!("Bot getMe failed ({e}); serving stale probe cache");
                    return Ok(username);
                }
            }
            Err(e)
        }
    }
}

pub async fn bot_connection_ready(config: &ServerConfig) -> bool {
    if !TransportHandle::bot_configured(config) {
        return false;
    }
    bot_test_connection_cached(config).await.is_ok()
}

pub fn validate_bot_config(config: &ServerConfig) -> Result<(), String> {
    if config.all_bot_tokens().is_empty() {
        return Err("TG_BOT_TOKEN is required for bot mode".into());
    }
    if config
        .storage_channel_id
        .as_ref()
        .is_none_or(|c| c.trim().is_empty())
    {
        return Err(
            "TG_STORAGE_CHANNEL_ID is required for bot mode (private channel chat id)".into(),
        );
    }
    Ok(())
}

async fn bot_rate_limit(config: &ServerConfig) {
    if config.bot_rate_limit_ms > 0 {
        tokio::time::sleep(Duration::from_millis(config.bot_rate_limit_ms as u64)).await;
    }
}

/// Extract FloodWait seconds from Telegram error message.
/// Returns None if the error is not a FloodWait error.
fn extract_flood_wait_seconds(error_msg: &str) -> Option<i32> {
    // Telegram error format: "FLOOD_WAIT_X" or "Too Many Requests: retry after X"
    let upper = error_msg.to_uppercase();
    if upper.contains("FLOOD_WAIT") {
        // Try to extract number from "FLOOD_WAIT_X" or "FLOOD_WAIT_X_Y"
        let parts: Vec<&str> = error_msg.split('_').collect();
        if parts.len() >= 3 {
            if let Ok(secs) = parts[2].parse::<i32>() {
                return Some(secs);
            }
        }
        // Try regex-like extraction
        for part in error_msg.split(|c: char| !c.is_ascii_digit()) {
            if let Ok(secs) = part.parse::<i32>() {
                if secs > 0 {
                    return Some(secs);
                }
            }
        }
    }
    if upper.contains("RETRY AFTER") {
        // "retry after X" format
        for part in error_msg.split(|c: char| !c.is_ascii_digit()) {
            if let Ok(secs) = part.parse::<i32>() {
                if secs > 0 {
                    return Some(secs);
                }
            }
        }
    }
    None
}

/// Bot upload using shared BotPool with FloodWait awareness.
/// Uses `next_available_token()` which skips bots in FloodWait state.
/// On FloodWait error, marks the bot and returns error for caller to retry.
pub async fn bot_upload_bytes_with_pool(
    config: &ServerConfig,
    db: &DbConnection,
    data: &[u8],
    upload_name: &str,
    caption: Option<&str>,
    bot_pool: &crate::bot_pool::BotPool,
) -> Result<BotUploadResult, String> {
    validate_bot_config(config)?;
    if data.len() > BOT_SINGLE_FILE_MAX {
        return Err(format!(
            "Bot API single upload limit is {} MB — use /upload_chunk for larger files",
            BOT_SINGLE_FILE_MAX / 1024 / 1024
        ));
    }

    bot_rate_limit(config).await;

    // Use FloodWait-aware token selection
    let (bot_token, bot_pool_index) = bot_pool.next_available_token().ok_or_else(|| {
        let metrics = bot_pool.metrics();
        format!(
            "All {} bot(s) are in FloodWait. Earliest available in {}s",
            metrics.total_bots,
            bot_pool.earliest_availability_secs().unwrap_or(0)
        )
    })?;

    let chat_id = config.storage_channel_id.clone().unwrap_or_default();
    let mime = mime_guess_from_name(upload_name);
    let part = Part::bytes(data.to_vec())
        .file_name(upload_name.to_string())
        .mime_str(&mime)
        .map_err(|e| e.to_string())?;

    let mut form = Form::new().text("chat_id", chat_id).part("document", part);
    if let Some(cap) = caption {
        form = form.text("caption", cap.to_string());
    }

    let client = reqwest::Client::new();
    let url = bot_api_url_with_token(config, &bot_token, "sendDocument");
    let resp = client
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Bot sendDocument failed: {e}"))?;
    let json: TgApiResponse<TgMessage> = resp
        .json()
        .await
        .map_err(|e| format!("Bot sendDocument parse failed: {e}"))?;

    // Check for FloodWait or other errors
    if !json.ok {
        let error_desc = json
            .description
            .clone()
            .unwrap_or_else(|| "Bot sendDocument rejected".to_string());

        // Check if this is a FloodWait error
        if let Some(flood_secs) = extract_flood_wait_seconds(&error_desc) {
            bot_pool.mark_flood_wait(bot_pool_index, flood_secs);
            log::warn!(
                "Bot [index={}] hit FloodWait for {}s. Error: {}",
                bot_pool_index,
                flood_secs,
                error_desc
            );
            return Err(format!("FLOOD_WAIT:{}:{}", bot_pool_index, flood_secs));
        }

        return Err(error_desc);
    }

    let msg = json
        .result
        .ok_or_else(|| "Bot sendDocument returned empty result".to_string())?;
    let doc = msg
        .document
        .ok_or_else(|| "Bot sendDocument succeeded but document missing".to_string())?;

    // Mark success (clears any stale FloodWait state)
    bot_pool.mark_success(bot_pool_index);

    let result = BotUploadResult {
        message_id: msg.message_id,
        telegram_file_id: doc.file_id.clone(),
        file_name: doc.file_name.unwrap_or_else(|| upload_name.to_string()),
        file_size: doc.file_size.unwrap_or(data.len() as i64) as u64,
        mime_type: doc.mime_type.unwrap_or(mime),
    };

    crate::db::upsert_bot_file_map(
        db,
        result.message_id,
        &result.telegram_file_id,
        &result.file_name,
        result.file_size,
        caption,
        bot_pool_index,
    )?;

    Ok(result)
}

/// Legacy bot upload without shared pool (creates new pool each call).
/// Prefer `bot_upload_bytes_with_pool` for FloodWait-aware uploads.
pub async fn bot_upload_bytes(
    config: &ServerConfig,
    db: &DbConnection,
    data: &[u8],
    upload_name: &str,
    caption: Option<&str>,
) -> Result<BotUploadResult, String> {
    let pool = crate::bot_pool::BotPool::from_config(config);
    bot_upload_bytes_with_pool(config, db, data, upload_name, caption, &pool).await
}

pub async fn bot_upload_file_path(
    config: &ServerConfig,
    db: &DbConnection,
    path: &str,
    caption: Option<&str>,
) -> Result<BotUploadResult, String> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| format!("read upload file: {e}"))?;
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "upload.bin".to_string());
    bot_upload_bytes(config, db, &data, &name, caption).await
}

async fn bot_resolve_file_path(
    config: &ServerConfig,
    bot_token: &str,
    telegram_file_id: &str,
) -> Result<String, String> {
    let url = format!(
        "{}?file_id={}",
        bot_api_url_with_token(config, bot_token, "getFile"),
        urlencoding::encode(telegram_file_id)
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Bot getFile failed: {e}"))?;
    let json: TgApiResponse<TgFile> = resp
        .json()
        .await
        .map_err(|e| format!("Bot getFile parse failed: {e}"))?;
    if !json.ok {
        return Err(json
            .description
            .unwrap_or_else(|| "Bot getFile rejected".to_string()));
    }
    let file = json
        .result
        .ok_or_else(|| "Bot getFile returned empty result".to_string())?;
    Ok(file.file_path)
}

pub async fn bot_download_stream(
    config: &ServerConfig,
    db: &DbConnection,
    message_id: i32,
    range_header: Option<&str>,
) -> Result<(reqwest::Response, String, u64), String> {
    let record = crate::db::get_bot_file_map(db, message_id)?
        .ok_or_else(|| format!("No bot file mapping for message_id {message_id}"))?;
    let bot_token = resolve_bot_token_for_pool_index(config, record.bot_pool_index)?;
    let file_path = bot_resolve_file_path(config, &bot_token, &record.telegram_file_id).await?;
    let download_url = bot_file_url_with_token(config, &bot_token, &file_path);

    let mut req = reqwest::Client::new().get(download_url);
    if let Some(range) = range_header {
        req = req.header("Range", range);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("Bot file download failed: {e}"))?;
    if !resp.status().is_success() && resp.status().as_u16() != 206 {
        return Err(format!("Bot file download HTTP {}", resp.status()));
    }

    Ok((resp, record.file_name, record.file_size))
}

pub async fn ensure_transport_ready(
    handle: &TransportHandle,
    config: &ServerConfig,
    data_dir: &Path,
    tg_state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
) -> Result<TelegramTransportMode, String> {
    let mode = handle.effective_mode(config).await;
    match mode {
        TelegramTransportMode::Bot => {
            validate_bot_config(config)?;
            bot_test_connection(config).await?;
            Ok(TelegramTransportMode::Bot)
        }
        TelegramTransportMode::User => {
            crate::commands::auth::ensure_client_initialized_at(
                data_dir,
                net_config,
                tg_state,
                config.telegram_api_id,
            )
            .await?;
            Ok(TelegramTransportMode::User)
        }
    }
}

pub async fn bot_download_manifest_bytes(
    config: &ServerConfig,
    db: &DbConnection,
    manifest_message_id: i32,
) -> Result<Vec<u8>, String> {
    let (resp, _, _) = bot_download_stream(config, db, manifest_message_id, None).await?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read manifest body: {e}"))?;
    Ok(bytes.to_vec())
}

fn mime_guess_from_name(filename: &str) -> String {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg".into(),
        "png" => "image/png".into(),
        "gif" => "image/gif".into(),
        "mp4" => "video/mp4".into(),
        "mp3" => "audio/mpeg".into(),
        "pdf" => "application/pdf".into(),
        "txt" => "text/plain".into(),
        _ => "application/octet-stream".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_transport_modes() {
        assert_eq!(
            TelegramTransportMode::parse("bot"),
            Some(TelegramTransportMode::Bot)
        );
        assert_eq!(
            TelegramTransportMode::parse("application"),
            Some(TelegramTransportMode::User)
        );
    }

    #[test]
    fn mime_guess_from_name_maps_common_types() {
        assert_eq!(mime_guess_from_name("a.png"), "image/png");
        assert_eq!(mime_guess_from_name("doc.PDF"), "application/pdf");
        assert_eq!(
            mime_guess_from_name("x.unknownext"),
            "application/octet-stream"
        );
    }

    #[tokio::test]
    async fn active_mode_reloads_persisted_mode_from_disk() {
        let dir = std::env::temp_dir().join(format!("td-transport-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let handle = TransportHandle::new(&dir, TelegramTransportMode::Bot);
        assert_eq!(handle.active_mode().await, TelegramTransportMode::Bot);
        let path = dir.join("transport_mode.json");
        std::fs::write(&path, r#"{"mode":"user"}"#).unwrap();
        assert_eq!(handle.active_mode().await, TelegramTransportMode::User);
    }

    #[tokio::test]
    async fn bot_probe_cache_serves_fresh_username() {
        let cache = BotProbeCache::default();
        cache.record_success("mybot".into()).await;
        assert_eq!(
            cache.cached_username_if_fresh().await,
            Some("mybot".to_string())
        );
    }

    #[tokio::test]
    async fn bot_probe_stale_ok_until_max_consecutive_failures() {
        let cache = BotProbeCache::default();
        cache.record_success("mybot".into()).await;
        cache.record_failure().await;
        assert!(cache.stale_ok().await);
        cache.record_failure().await;
        cache.record_failure().await;
        assert!(!cache.stale_ok().await);
    }
}
