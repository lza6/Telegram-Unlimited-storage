use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, RwLock};

use crate::db::DbConnection;
use crate::server_config::ServerConfig;

use crate::commands::TelegramState;
use crate::vpn_optimizer::NetworkConfig;

const BOT_SINGLE_FILE_MAX: usize = 20 * 1024 * 1024;

/// Maximum attempts to wait for a bot to become eligible before giving up.
/// Bounds the worst-case latency when all bots are rate-limited.
const BOT_ACQUIRE_MAX_WAIT_SECS: u64 = 60;

/// Process-wide shared HTTP client with a connection pool.
/// Replacing per-request `reqwest::Client::new()` avoids 500× TLS handshakes
/// under concurrent uploads and reuses keep-alive connections to Telegram.
/// `std::sync::OnceLock` is used (not tokio's OnceCell) so the accessor is
/// synchronous and callable from any context.
static SHARED_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

/// Default per-host idle connections; sized for the 500-concurrency target.
const CLIENT_POOL_IDLE_PER_HOST: usize = 64;

fn shared_client() -> reqwest::Client {
    SHARED_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .pool_idle_timeout(Some(Duration::from_secs(90)))
                .pool_max_idle_per_host(CLIENT_POOL_IDLE_PER_HOST)
                .tcp_keepalive(Some(Duration::from_secs(60)))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

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
    chat: TgChat,
    document: Option<TgDocument>,
}

#[derive(Deserialize)]
struct TgChat {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct TgFile {
    file_path: String,
    file_size: Option<i64>,
}

/// Receipt returned by a successful Telegram Bot upload. The storage peer is
/// captured from the active transport configuration rather than inferred later.
pub struct BotUploadResult {
    pub message_id: i32,
    pub telegram_file_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub storage_peer_id: i64,
    pub storage_peer_kind: String,
    pub bot_pool_index: u32,
    pub uploader_bot_id: String,
}

pub fn bot_api_peer_kind(chat_id: i64) -> &'static str {
    if chat_id > 0 {
        "private"
    } else if chat_id >= -999_999_999_999 {
        "group"
    } else {
        "supergroup_or_channel"
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelegramUploadReceipt {
    pub message_id: i32,
    pub telegram_file_id: Option<String>,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: String,
    pub storage_peer_id: i64,
    pub storage_peer_kind: String,
    #[serde(default)]
    pub bot_pool_index: Option<u32>,
    #[serde(default)]
    pub uploader_bot_id: Option<String>,
}

impl From<&BotUploadResult> for TelegramUploadReceipt {
    fn from(result: &BotUploadResult) -> Self {
        Self {
            message_id: result.message_id,
            telegram_file_id: Some(result.telegram_file_id.clone()),
            file_name: result.file_name.clone(),
            file_size: result.file_size,
            mime_type: result.mime_type.clone(),
            storage_peer_id: result.storage_peer_id,
            storage_peer_kind: result.storage_peer_kind.clone(),
            bot_pool_index: Some(result.bot_pool_index),
            uploader_bot_id: Some(result.uploader_bot_id.clone()),
        }
    }
}

fn redacted_reqwest_error(context: &str, error: reqwest::Error) -> String {
    let error = error.without_url();
    format!("{context} failed: {error}")
}

fn bot_api_url_with_token(config: &ServerConfig, token: &str, method: &str) -> String {
    let base = config.bot_api_base.trim_end_matches('/').to_string();
    format!("{base}/bot{token}/{method}")
}

#[derive(Clone, Debug)]
pub struct PreselectedBot {
    pub(crate) token: String,
    pub pool_index: u32,
    pub stable_id: String,
}

pub async fn preselect_bot(pool: &crate::bot_pool::BotPool) -> Result<PreselectedBot, String> {
    let (token, pool_index) = acquire_bot_token(pool).await?;
    let stable_id = bot_token_identity(&token);
    Ok(PreselectedBot {
        token,
        pool_index,
        stable_id,
    })
}

pub(crate) fn bot_token_identity(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let encoded = digest[..12]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("bot-{encoded}")
}

fn resolve_bot_token_by_identity(
    config: &ServerConfig,
    uploader_bot_id: &str,
) -> Result<String, String> {
    config
        .all_bot_tokens()
        .into_iter()
        .find(|token| bot_token_identity(token) == uploader_bot_id)
        .ok_or_else(|| format!("Bot uploader identity {uploader_bot_id} is not configured"))
}

fn resolve_bot_token_for_pool_index(
    config: &ServerConfig,
    pool_index: u32,
) -> Result<String, String> {
    let pool = crate::bot_pool::BotPool::from_config(config);
    pool.token_at(pool_index)
        .map(|token| token.to_string())
        .ok_or_else(|| format!("Bot token pool index {pool_index} is not configured"))
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
    let client = shared_client();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| redacted_reqwest_error("Bot getMe", e))?;
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
const BOT_PROBE_FAILURE_TTL: Duration = Duration::from_secs(5);
const BOT_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const BOT_PROBE_MAX_CONSECUTIVE_FAILS: u32 = 3;

#[derive(Default)]
struct BotProbeCacheState {
    config_key: Option<String>,
    last_attempt_at: Option<Instant>,
    last_success_at: Option<Instant>,
    last_username: Option<String>,
    last_error: Option<String>,
    consecutive_failures: u32,
}

pub struct BotProbeCache {
    inner: RwLock<BotProbeCacheState>,
    probe_lock: Mutex<()>,
}

impl Default for BotProbeCache {
    fn default() -> Self {
        Self {
            inner: RwLock::new(BotProbeCacheState::default()),
            probe_lock: Mutex::new(()),
        }
    }
}

impl BotProbeCache {
    fn reset_for_config(state: &mut BotProbeCacheState, config_key: &str) {
        if state.config_key.as_deref() != Some(config_key) {
            *state = BotProbeCacheState {
                config_key: Some(config_key.to_string()),
                ..BotProbeCacheState::default()
            };
        }
    }

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

    fn cached_result_state(
        &self,
        state: &BotProbeCacheState,
        config_key: &str,
    ) -> Option<Result<String, String>> {
        if state.config_key.as_deref() != Some(config_key) {
            return None;
        }
        let attempted_at = state.last_attempt_at?;
        let ttl = if state.last_error.is_some() {
            BOT_PROBE_FAILURE_TTL
        } else {
            BOT_PROBE_CACHE_TTL
        };
        if attempted_at.elapsed() > ttl {
            return None;
        }
        if self.stale_ok_state(state) {
            if let Some(username) = state.last_username.clone() {
                return Some(Ok(username));
            }
        }
        if let Some(error) = state.last_error.clone() {
            return Some(Err(error));
        }
        state.last_username.clone().map(Ok)
    }

    pub async fn cached_result(&self, config_key: &str) -> Option<Result<String, String>> {
        let state = self.inner.read().await;
        self.cached_result_state(&state, config_key)
    }

    pub async fn record_success(&self, config_key: &str, username: String) {
        let mut state = self.inner.write().await;
        Self::reset_for_config(&mut state, config_key);
        state.last_attempt_at = Some(Instant::now());
        state.last_success_at = Some(Instant::now());
        state.last_username = Some(username);
        state.last_error = None;
        state.consecutive_failures = 0;
    }

    pub async fn record_failure(&self, config_key: &str, error: String) {
        let mut state = self.inner.write().await;
        Self::reset_for_config(&mut state, config_key);
        state.last_attempt_at = Some(Instant::now());
        state.last_error = Some(error);
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

    pub async fn acquire_probe_lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.probe_lock.lock().await
    }
}

static BOT_PROBE_CACHE: std::sync::OnceLock<BotProbeCache> = std::sync::OnceLock::new();

fn bot_probe_cache() -> &'static BotProbeCache {
    BOT_PROBE_CACHE.get_or_init(BotProbeCache::default)
}

fn bot_probe_cache_key(config: &ServerConfig) -> String {
    let mut hasher = Sha256::new();
    hasher.update(config.bot_api_base.as_bytes());
    hasher.update(b"\0");
    if let Some(token) = config.all_bot_tokens().first() {
        hasher.update(token.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// Cached, single-flight Bot getMe. Successes are cached for 30 seconds and
/// failures for 5 seconds so public readiness probes cannot amplify requests.
pub async fn bot_test_connection_cached(config: &ServerConfig) -> Result<String, String> {
    let cache = bot_probe_cache();
    let config_key = bot_probe_cache_key(config);
    if let Some(result) = cache.cached_result(&config_key).await {
        return result;
    }

    let _probe_guard = cache.acquire_probe_lock().await;
    if let Some(result) = cache.cached_result(&config_key).await {
        return result;
    }
    match tokio::time::timeout(BOT_PROBE_TIMEOUT, bot_test_connection(config)).await {
        Ok(Ok(username)) => {
            cache.record_success(&config_key, username.clone()).await;
            Ok(username)
        }
        Ok(Err(e)) => {
            cache.record_failure(&config_key, e.clone()).await;
            if cache.stale_ok().await {
                if let Some(username) = cache.cached_username_if_fresh().await {
                    log::warn!("Bot getMe failed ({e}); serving stale probe cache");
                    return Ok(username);
                }
            }
            Err(e)
        }
        Err(_) => {
            let error = "Bot getMe timed out".to_string();
            cache.record_failure(&config_key, error.clone()).await;
            if cache.stale_ok().await {
                if let Some(username) = cache.cached_username_if_fresh().await {
                    log::warn!("{error}; serving stale probe cache");
                    return Ok(username);
                }
            }
            Err(error)
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

/// Acquire an available bot token, honoring the per-bot minimum interval.
/// Replaces the former global serial `bot_rate_limit` sleep: instead of one
/// process-wide sleep per upload (serializing all bots), each bot now has its
/// own interval, so N bots dispatch in parallel at N× the per-bot rate.
///
/// Returns `Err(FLOOD_WAIT message)` only when every bot is in FloodWait for
/// longer than `BOT_ACQUIRE_MAX_WAIT_SECS`.
async fn acquire_bot_token(bot_pool: &crate::bot_pool::BotPool) -> Result<(String, u32), String> {
    let deadline = Instant::now() + Duration::from_secs(BOT_ACQUIRE_MAX_WAIT_SECS);
    loop {
        if let Some((token, idx)) = bot_pool.try_acquire_now() {
            return Ok((token, idx));
        }
        match bot_pool.earliest_eligible_in() {
            Some(wait) => {
                if Instant::now() + wait > deadline {
                    let metrics = bot_pool.metrics();
                    return Err(format!(
                        "All {} bot(s) busy or in FloodWait beyond {}s (earliest {}s)",
                        metrics.total_bots,
                        BOT_ACQUIRE_MAX_WAIT_SECS,
                        bot_pool.earliest_availability_secs().unwrap_or(0)
                    ));
                }
                tokio::time::sleep(wait).await;
            }
            None => {
                // No eligible wait → all bots in FloodWait.
                let metrics = bot_pool.metrics();
                return Err(format!(
                    "All {} bot(s) are in FloodWait. Earliest available in {}s",
                    metrics.total_bots,
                    bot_pool.earliest_availability_secs().unwrap_or(0)
                ));
            }
        }
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
    bot_upload_bytes_with_pool_and_selection(config, db, data, upload_name, caption, bot_pool, None)
        .await
}

pub async fn bot_upload_bytes_with_pool_and_selection(
    config: &ServerConfig,
    _db: &DbConnection,
    data: &[u8],
    upload_name: &str,
    caption: Option<&str>,
    bot_pool: &crate::bot_pool::BotPool,
    selected: Option<&PreselectedBot>,
) -> Result<BotUploadResult, String> {
    validate_bot_config(config)?;
    if data.len() > BOT_SINGLE_FILE_MAX {
        return Err(format!(
            "Bot API single upload limit is {} MB — use /upload_chunk for larger files",
            BOT_SINGLE_FILE_MAX / 1024 / 1024
        ));
    }

    // Per-bot interval-aware token acquisition (parallel across bots,
    // replacing the former global serial sleep).
    let (bot_token, bot_pool_index) = match selected {
        Some(bot) => (bot.token.clone(), bot.pool_index),
        None => acquire_bot_token(bot_pool).await?,
    };

    let chat_id = config.storage_channel_id.clone().unwrap_or_default();
    let configured_peer_id = chat_id
        .parse::<i64>()
        .map_err(|_| "TG_STORAGE_CHANNEL_ID must be a numeric Telegram dialog id".to_string())?;
    let mime = mime_guess_from_name(upload_name);
    let part = Part::bytes(data.to_vec())
        .file_name(upload_name.to_string())
        .mime_str(&mime)
        .map_err(|e| e.to_string())?;

    let mut form = Form::new().text("chat_id", chat_id).part("document", part);
    if let Some(cap) = caption {
        form = form.text("caption", cap.to_string());
    }

    let client = shared_client();
    let url = bot_api_url_with_token(config, &bot_token, "sendDocument");
    let resp = client
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| redacted_reqwest_error("Bot sendDocument", e))?;
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
    if msg.chat.id != configured_peer_id {
        return Err("Bot sendDocument returned an unexpected storage peer".to_string());
    }

    // Mark success (clears any stale FloodWait state)
    bot_pool.mark_success(bot_pool_index);

    let result = BotUploadResult {
        message_id: msg.message_id,
        telegram_file_id: doc.file_id.clone(),
        file_name: doc.file_name.unwrap_or_else(|| upload_name.to_string()),
        file_size: doc.file_size.unwrap_or(data.len() as i64) as u64,
        mime_type: doc.mime_type.unwrap_or(mime),
        storage_peer_id: msg.chat.id,
        storage_peer_kind: msg.chat.kind,
        bot_pool_index,
        uploader_bot_id: bot_token_identity(&bot_token),
    };

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
    let result = bot_upload_bytes_with_pool(config, db, data, upload_name, caption, &pool).await?;
    crate::db::upsert_bot_file_map(
        db,
        result.message_id,
        &result.telegram_file_id,
        &result.file_name,
        result.file_size,
        caption,
        result.bot_pool_index,
    )?;
    Ok(result)
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
    let client = shared_client();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| redacted_reqwest_error("Bot getFile", e))?;
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

fn bot_delete_chat_id(storage_peer_id: i64, storage_peer_kind: &str) -> Result<String, String> {
    if storage_peer_id == 0 {
        return Err("Bot delete receipt peer is missing".to_string());
    }
    let valid = match storage_peer_kind {
        "private" => storage_peer_id > 0,
        "group" => storage_peer_id < 0 && storage_peer_id >= -999_999_999_999,
        "supergroup" | "channel" | "supergroup_or_channel" => storage_peer_id < -999_999_999_999,
        _ => false,
    };
    if !valid {
        return Err(format!(
            "Bot delete receipt peer kind mismatch: peer_id={storage_peer_id}, kind={storage_peer_kind}"
        ));
    }
    Ok(storage_peer_id.to_string())
}

fn resolve_bot_delete_token(
    config: &ServerConfig,
    db: &DbConnection,
    message_id: i32,
    receipt_telegram_file_id: Option<&str>,
    uploader_bot_id: Option<&str>,
) -> Result<String, String> {
    let receipt_file_id = receipt_telegram_file_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Bot delete receipt telegram_file_id is missing".to_string())?;
    let mapping = crate::db::get_bot_file_map(db, message_id)?;
    if let Some(record) = mapping.as_ref() {
        if record.telegram_file_id != receipt_file_id {
            return Err(format!(
                "Bot delete telegram_file_id mismatch for message_id={message_id}"
            ));
        }
    }
    if let Some(identity) = uploader_bot_id.filter(|value| !value.trim().is_empty()) {
        return resolve_bot_token_by_identity(config, identity);
    }
    let record = mapping
        .ok_or_else(|| format!("Bot delete missing bot_file_map for message_id={message_id}"))?;
    resolve_bot_token_for_pool_index(config, record.bot_pool_index)
}

pub async fn bot_delete_message(
    config: &ServerConfig,
    db: &DbConnection,
    storage_peer_id: i64,
    storage_peer_kind: &str,
    message_id: i32,
    telegram_file_id: Option<&str>,
    uploader_bot_id: Option<&str>,
) -> Result<(), String> {
    let bot_token =
        resolve_bot_delete_token(config, db, message_id, telegram_file_id, uploader_bot_id)?;
    let chat_id = bot_delete_chat_id(storage_peer_id, storage_peer_kind)?;
    let client = shared_client();
    let response = client
        .post(bot_api_url_with_token(config, &bot_token, "deleteMessage"))
        .form(&[("chat_id", chat_id), ("message_id", message_id.to_string())])
        .send()
        .await
        .map_err(|error| redacted_reqwest_error("Bot deleteMessage", error))?;
    let result: TgApiResponse<bool> = response
        .json()
        .await
        .map_err(|error| format!("Bot deleteMessage parse failed: {error}"))?;
    if !result.ok || result.result != Some(true) {
        let description = result
            .description
            .unwrap_or_else(|| "Bot deleteMessage rejected".to_string());
        if !description
            .to_ascii_lowercase()
            .contains("message to delete not found")
        {
            return Err(description);
        }
    }
    Ok(())
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

    let mut req = shared_client().get(download_url);
    if let Some(range) = range_header {
        req = req.header("Range", range);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| redacted_reqwest_error("Bot file download", e))?;
    if !resp.status().is_success() && resp.status().as_u16() != 206 {
        return Err(format!("Bot file download HTTP {}", resp.status()));
    }

    Ok((resp, record.file_name, record.file_size))
}

pub async fn bot_download_stream_for_locator(
    config: &ServerConfig,
    telegram_file_id: &str,
    file_name: &str,
    file_size: u64,
    bot_pool_index: Option<u32>,
    uploader_bot_id: Option<&str>,
    range_header: Option<&str>,
) -> Result<(reqwest::Response, String, u64), String> {
    let bot_token = if let Some(identity) = uploader_bot_id.filter(|value| !value.trim().is_empty())
    {
        resolve_bot_token_by_identity(config, identity)?
    } else {
        resolve_bot_token_for_pool_index(
            config,
            bot_pool_index
                .ok_or_else(|| "Bot asset locator is missing uploader identity".to_string())?,
        )?
    };
    let file_path = bot_resolve_file_path(config, &bot_token, telegram_file_id).await?;
    let download_url = bot_file_url_with_token(config, &bot_token, &file_path);
    let mut request = shared_client().get(download_url);
    if let Some(range) = range_header {
        request = request.header("Range", range);
    }
    let response = request
        .send()
        .await
        .map_err(|error| redacted_reqwest_error("Bot file download", error))?;
    if !response.status().is_success() && response.status().as_u16() != 206 {
        return Err(format!("Bot file download HTTP {}", response.status()));
    }
    Ok((response, file_name.to_string(), file_size))
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

    fn temp_db() -> crate::db::DbConnection {
        let dir = std::env::temp_dir().join(format!("td-transport-test-{}", uuid::Uuid::new_v4()));
        crate::db::init_db_at(&dir).expect("db")
    }

    #[test]
    fn bot_peer_kind_uses_bot_api_dialog_shape() {
        assert_eq!(bot_api_peer_kind(123), "private");
        assert_eq!(bot_api_peer_kind(-123), "group");
        assert_eq!(
            bot_api_peer_kind(-1_000_000_000_123),
            "supergroup_or_channel"
        );
    }

    #[test]
    fn bot_delete_target_uses_receipt_peer_and_rejects_kind_mismatch() {
        for kind in ["supergroup", "channel", "supergroup_or_channel"] {
            assert_eq!(
                bot_delete_chat_id(-1_000_000_000_123, kind).unwrap(),
                "-1000000000123"
            );
        }
        assert_eq!(bot_delete_chat_id(-123, "group").unwrap(), "-123");
        assert_eq!(bot_delete_chat_id(123, "private").unwrap(), "123");
        assert!(bot_delete_chat_id(-1_000_000_000_123, "group").is_err());
        assert!(bot_delete_chat_id(0, "private").is_err());
    }

    #[test]
    fn bot_token_resolution_fails_closed_for_unknown_pool_index() {
        let config = crate::server_config::test_config();
        let error = resolve_bot_token_for_pool_index(&config, 99).unwrap_err();
        assert!(error.contains("pool index 99"));
    }

    #[test]
    fn bot_delete_token_requires_matching_receipt_mapping() {
        let config = crate::server_config::test_config();
        let db = temp_db();
        let missing = resolve_bot_delete_token(&config, &db, 41, Some("tg-41"), None).unwrap_err();
        assert!(missing.contains("missing bot_file_map"));
        crate::db::upsert_bot_file_map(&db, 41, "tg-41", "a.bin", 1, None, 1).expect("map");
        assert_eq!(
            resolve_bot_delete_token(&config, &db, 41, Some("tg-41"), None).unwrap(),
            "2:fake"
        );
        let mismatch =
            resolve_bot_delete_token(&config, &db, 41, Some("different"), None).unwrap_err();
        assert!(mismatch.contains("telegram_file_id mismatch"));
        assert!(resolve_bot_delete_token(&config, &db, 41, None, None)
            .unwrap_err()
            .contains("receipt telegram_file_id"));
    }

    #[test]
    fn bot_delete_token_uses_stable_receipt_identity_without_local_mapping() {
        let config = crate::server_config::test_config();
        let db = temp_db();
        let identity = bot_token_identity("2:fake");
        assert_eq!(
            resolve_bot_delete_token(&config, &db, 77, Some("tg-77"), Some(&identity)).unwrap(),
            "2:fake"
        );
        let mut reordered = (*config).clone();
        reordered.bot_token = Some("2:fake".to_string());
        reordered.extra_bot_tokens = vec!["1:fake".to_string()];
        assert_eq!(
            resolve_bot_delete_token(&reordered, &db, 77, Some("tg-77"), Some(&identity)).unwrap(),
            "2:fake"
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
        cache.record_success("config-a", "mybot".into()).await;
        assert_eq!(
            cache.cached_username_if_fresh().await,
            Some("mybot".to_string())
        );
        assert_eq!(
            cache.cached_result("config-a").await.unwrap().unwrap(),
            "mybot"
        );
    }

    #[tokio::test]
    async fn bot_probe_stale_ok_until_max_consecutive_failures() {
        let cache = BotProbeCache::default();
        cache.record_success("config-a", "mybot".into()).await;
        cache.record_failure("config-a", "temporary".into()).await;
        assert!(cache.stale_ok().await);
        cache.record_failure("config-a", "temporary".into()).await;
        cache.record_failure("config-a", "temporary".into()).await;
        assert!(!cache.stale_ok().await);
    }

    #[tokio::test]
    async fn bot_probe_failure_is_cached_and_scoped_to_config() {
        let cache = BotProbeCache::default();
        cache.record_failure("config-a", "offline".into()).await;
        assert_eq!(
            cache.cached_result("config-a").await.unwrap().unwrap_err(),
            "offline"
        );
        assert!(cache.cached_result("config-b").await.is_none());
    }
}
