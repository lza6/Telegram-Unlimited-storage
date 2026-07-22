use std::path::PathBuf;

use crate::telegram_transport::TelegramTransportMode;

#[derive(Clone, Debug)]
pub struct RateLimitConfig {
    pub ip_rpm: u32,
    pub api_key_rpm: u32,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind_host: String,
    pub port: u16,
    pub stream_port: u16,
    pub data_dir: PathBuf,
    pub access_pwd: String,
    pub api_key: Option<String>,
    pub api_key_hash: Option<String>,
    pub telegram_api_id: i32,
    pub telegram_api_hash: String,
    /// Default transport from env (`bot` | `user` | `auto`). Runtime override via transport_mode.json.
    pub default_transport_mode: TelegramTransportMode,
    pub bot_token: Option<String>,
    /// Additional bots for round-robin uploads (`TG_BOT_TOKENS=token2,token3`).
    pub extra_bot_tokens: Vec<String>,
    pub storage_channel_id: Option<String>,
    pub bot_api_base: String,
    pub bot_rate_limit_ms: u32,
    pub base_url: String,
    pub static_dir: PathBuf,
    pub docs_dir: PathBuf,
    pub download_threads: u32,
    pub chunk_size_mb: u32,
    pub chunk_concurrent: u32,
    pub files_concurrent: u32,
    pub max_upload_size_mb: u32,
    pub cors_origins: Vec<String>,
    pub rate_limit: RateLimitConfig,
    pub metadata_cache_enabled: bool,
    pub metadata_cache_ttl_secs: u64,
    /// Allow `GET /d?file_id=` without share token (legacy tg-disk; insecure for multi-tenant).
    pub public_file_id_download: bool,
    /// Auto-create `/d/{token}` after upload; 0 = only API-key download.
    pub upload_share_ttl_hours: i64,
    /// HMAC secret for presigned URLs (≥32 chars). Preferred over DB share rows.
    pub download_signing_secret: Option<String>,
    /// Enforce file_assets ownership on API list/download.
    pub multi_tenant_enabled: bool,
    /// Presigned / share link TTL in seconds (default 3600). 0 = never expire (rotate secret to revoke).
    pub upload_link_ttl_secs: u64,
    /// Maximum allowed downloads per presigned URL. 0 = unlimited (default).
    pub presigned_max_downloads: Option<u32>,
    /// Enable WebDAV at WEBDAV_PREFIX (maps to file_assets + download).
    pub webdav_enabled: bool,
    pub webdav_prefix: String,
    /// Expose GET /metrics (Prometheus text format).
    pub metrics_enabled: bool,
    /// `memory` (default) or `redis` (distributed gate; requires REDIS_URL).
    pub upload_queue_backend: String,
    pub redis_url: Option<String>,
    /// S3 fallback storage configuration
    pub s3_endpoint: Option<String>,
    pub s3_region: Option<String>,
    pub s3_bucket: Option<String>,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    /// Telegram DC address for keep-alive probes (host:port). Default: 149.154.167.50:443.
    pub tg_dc_addr: String,
}

impl ServerConfig {
    pub fn max_chunk_bytes(&self) -> usize {
        self.chunk_size_mb.saturating_mul(1024 * 1024).max(1024) as usize
    }

    /// Primary + extra bot tokens for multi-bot upload pool.
    pub fn all_bot_tokens(&self) -> Vec<String> {
        let mut tokens = Vec::new();
        if let Some(t) = &self.bot_token {
            let t = t.trim();
            if !t.is_empty() {
                tokens.push(t.to_string());
            }
        }
        for t in &self.extra_bot_tokens {
            let t = t.trim();
            if !t.is_empty() && !tokens.iter().any(|x| x == t) {
                tokens.push(t.to_string());
            }
        }
        tokens
    }

    pub fn from_env() -> Result<Self, String> {
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/data"));

        let port = env_u16("PORT", 1334)?;
        let stream_port = env_u16("STREAM_PORT", 14201)?;

        let bot_token = std::env::var("TG_BOT_TOKEN")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let storage_channel_id = std::env::var("TG_STORAGE_CHANNEL_ID")
            .ok()
            .or_else(|| std::env::var("TG_CHAT_ID").ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        let default_transport_mode = parse_default_transport_mode(
            std::env::var("TELEGRAM_TRANSPORT_MODE")
                .unwrap_or_else(|_| "auto".to_string())
                .as_str(),
            bot_token.is_some(),
        );

        let (telegram_api_id, telegram_api_hash) =
            load_user_api_credentials(default_transport_mode, bot_token.is_some())?;

        let access_pwd = std::env::var("ACCESS_PWD")
            .map_err(|_| "ACCESS_PWD is required for web admin".to_string())?
            .trim()
            .to_string();

        let api_key = std::env::var("API_KEY")
            .map_err(|_| "API_KEY is required for the headless API".to_string())?
            .trim()
            .to_string();
        if api_key.is_empty() {
            return Err("API_KEY is required for the headless API".to_string());
        }
        let api_key = Some(api_key);
        let api_key_hash = api_key
            .as_ref()
            .map(|k| crate::commands::api_settings::hash_key_public(k));

        let static_dir = std::env::var("STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("deploy/web"));

        let docs_dir = std::env::var("DOCS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                if static_dir.starts_with("/app/") {
                    PathBuf::from("/app/docs")
                } else {
                    PathBuf::from("docs")
                }
            });

        let cors_origins = parse_cors_origins();

        Ok(Self {
            bind_host: std::env::var("BIND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port,
            stream_port,
            data_dir,
            access_pwd,
            api_key,
            api_key_hash,
            telegram_api_id,
            telegram_api_hash,
            default_transport_mode,
            bot_token,
            extra_bot_tokens: std::env::var("TG_BOT_TOKENS")
                .ok()
                .map(|raw| {
                    raw.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            storage_channel_id,
            bot_api_base: std::env::var("CUSTOM_BOT_API_URL")
                .unwrap_or_else(|_| "https://api.telegram.org".to_string())
                .trim()
                .trim_end_matches('/')
                .to_string(),
            bot_rate_limit_ms: env_u32("BOT_RATE_LIMIT_MS", 3500),
            base_url: std::env::var("BASE_URL").unwrap_or_default(),
            static_dir,
            docs_dir,
            download_threads: env_u32("DOWNLOAD_THREADS", 8),
            chunk_size_mb: env_u32("CHUNK_SIZE_MB", 10).min(50),
            chunk_concurrent: env_u32("CHUNK_CONCURRENT", 4),
            files_concurrent: env_u32("FILES_CONCURRENT", 2),
            max_upload_size_mb: env_u32("MAX_UPLOAD_SIZE_MB", 100).max(1),
            cors_origins,
            rate_limit: RateLimitConfig {
                ip_rpm: env_u32("RATE_LIMIT_RPM", 120),
                api_key_rpm: env_u32("RATE_LIMIT_API_RPM", 300),
            },
            metadata_cache_enabled: env_bool("METADATA_CACHE_ENABLED", true),
            metadata_cache_ttl_secs: env_u64("METADATA_CACHE_TTL_SECS", 300),
            public_file_id_download: env_bool("PUBLIC_FILE_ID_DOWNLOAD", false),
            upload_share_ttl_hours: env_i64("UPLOAD_SHARE_TTL_HOURS", 0),
            download_signing_secret: std::env::var("DOWNLOAD_SIGNING_SECRET")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            multi_tenant_enabled: env_bool("MULTI_TENANT_ENABLED", true),
            upload_link_ttl_secs: env_u64("UPLOAD_LINK_TTL_SECS", 0),
            presigned_max_downloads: std::env::var("PRESIGNED_MAX_DOWNLOADS")
                .ok()
                .and_then(|v| v.parse().ok())
                .filter(|v: &u32| *v > 0),
            webdav_enabled: env_bool("WEBDAV_ENABLED", false),
            webdav_prefix: std::env::var("WEBDAV_PREFIX")
                .unwrap_or_else(|_| "/webdav".to_string())
                .trim()
                .trim_end_matches('/')
                .to_string(),
            metrics_enabled: env_bool("METRICS_ENABLED", true),
            upload_queue_backend: std::env::var("UPLOAD_QUEUE_BACKEND")
                .unwrap_or_else(|_| "memory".to_string())
                .trim()
                .to_ascii_lowercase(),
            redis_url: std::env::var("REDIS_URL")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            s3_endpoint: std::env::var("S3_ENDPOINT")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            s3_region: std::env::var("S3_REGION")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            s3_bucket: std::env::var("S3_BUCKET")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            s3_access_key: std::env::var("S3_ACCESS_KEY")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            s3_secret_key: std::env::var("S3_SECRET_KEY")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            tg_dc_addr: std::env::var("TG_DC_ADDR")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "149.154.167.50:443".to_string()),
        })
    }

    /// 仍为 .env.example 占位值时，Telegram 用户模式登录必然失败。
    pub fn telegram_credentials_placeholder(&self) -> Option<&'static str> {
        let hash_placeholder =
            self.telegram_api_hash == "your_api_hash_here" || self.telegram_api_hash.len() < 16;
        if self.telegram_api_id == 123456 || hash_placeholder {
            Some(
                "TELEGRAM_API_ID / TELEGRAM_API_HASH 仍是示例值。用户模式需在 .env 填写 https://my.telegram.org 凭据；机器人模式可改用 TG_BOT_TOKEN + TG_STORAGE_CHANNEL_ID。",
            )
        } else {
            None
        }
    }

    pub fn bot_ready_configured(&self) -> bool {
        crate::telegram_transport::TransportHandle::bot_configured(self)
    }

    pub fn ensure_api_settings_file(&self) -> Result<(), String> {
        let mut settings = crate::commands::api_settings::load_settings_at(&self.data_dir);
        settings.enabled = true;
        settings.port = self.port;
        if settings.key_hash.is_none() {
            settings.key_hash = self.api_key_hash.clone();
        }
        crate::commands::api_settings::save_settings_at(&self.data_dir, &settings)
    }

    /// Log errors for insecure default secrets still present in production-like deploys.
    pub fn warn_insecure_defaults(&self) {
        let insecure_pwd =
            self.access_pwd == "change-me-strong-password" || self.access_pwd.len() < 8;
        if insecure_pwd {
            log::error!(
                "ACCESS_PWD is weak or still the .env.example placeholder — set a strong password before production"
            );
        }
        if let Some(secret) = &self.download_signing_secret {
            if secret.contains("replace-with-at-least-32") || secret.len() < 32 {
                log::error!(
                    "DOWNLOAD_SIGNING_SECRET is placeholder or too short — presigned downloads may fail or be insecure"
                );
            }
        }
        if let Some(key) = &self.api_key {
            if key.contains("generate-a-long") || key.len() < 16 {
                log::error!("API_KEY is placeholder or too short");
            }
        }
    }
}

fn env_u16(key: &str, default: u16) -> Result<u16, String> {
    match std::env::var(key) {
        Ok(v) => v.parse().map_err(|_| format!("{key} must be a number")),
        Err(_) => Ok(default),
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(v) => matches!(
            v.trim().to_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_default_transport_mode(raw: &str, has_bot_token: bool) -> TelegramTransportMode {
    match raw.trim().to_lowercase().as_str() {
        "bot" => TelegramTransportMode::Bot,
        "user" | "app" | "application" => TelegramTransportMode::User,
        _ => {
            if has_bot_token {
                TelegramTransportMode::Bot
            } else {
                TelegramTransportMode::User
            }
        }
    }
}

fn load_user_api_credentials(
    default_mode: TelegramTransportMode,
    has_bot_token: bool,
) -> Result<(i32, String), String> {
    let id_raw = std::env::var("TELEGRAM_API_ID").ok();
    let hash_raw = std::env::var("TELEGRAM_API_HASH").ok();

    let needs_user = default_mode == TelegramTransportMode::User && !has_bot_token;

    let telegram_api_id = match id_raw {
        Some(v) => v
            .parse()
            .map_err(|_| "TELEGRAM_API_ID must be a number".to_string())?,
        None if needs_user => {
            return Err("TELEGRAM_API_ID is required for user/application mode".to_string());
        }
        None => 0,
    };

    let telegram_api_hash = match hash_raw {
        Some(v) => v.trim().to_string(),
        None if needs_user => {
            return Err("TELEGRAM_API_HASH is required for user/application mode".to_string());
        }
        None => String::new(),
    };

    Ok((telegram_api_id, telegram_api_hash))
}

/// True when `dir` looks like `deploy/web` with auth pages (Scheme B static subset).
pub fn desktop_static_servable(dir: &std::path::Path) -> bool {
    dir.is_dir() && dir.join("telegram.html").is_file()
}

/// Resolve `deploy/web` for desktop REST static pages (bundled resource, dev manifest, `STATIC_DIR`, exe-relative).
pub fn resolve_desktop_web_static_dir(
    resource_dir: Option<&std::path::Path>,
) -> Option<std::path::PathBuf> {
    if let Some(res) = resource_dir {
        for sub in ["web", "deploy/web", "web-static", "_up_/deploy/web"] {
            let p = res.join(sub);
            if desktop_static_servable(&p) {
                return p.canonicalize().ok().or(Some(p));
            }
        }
        if desktop_static_servable(res) {
            return res.canonicalize().ok().or(Some(res.to_path_buf()));
        }
    }

    if let Ok(v) = std::env::var("STATIC_DIR") {
        let p = std::path::PathBuf::from(v.trim());
        if desktop_static_servable(&p) {
            return Some(p);
        }
    }

    let from_manifest =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../deploy/web");
    if desktop_static_servable(&from_manifest) {
        return from_manifest.canonicalize().ok().or(Some(from_manifest));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            for rel in ["deploy/web", "../deploy/web", "../../deploy/web"] {
                let p = parent.join(rel);
                if desktop_static_servable(&p) {
                    return p.canonicalize().ok().or(Some(p));
                }
            }
        }
    }

    None
}

/// Minimal ServerConfig for the desktop optional REST API (127.0.0.1, app data dir).
#[cfg(not(feature = "headless-server"))]
pub fn for_desktop_api(
    data_dir: PathBuf,
    port: u16,
    key_hash: Option<String>,
    stream_port: u16,
    resource_dir: Option<PathBuf>,
) -> std::sync::Arc<ServerConfig> {
    let bot_token = std::env::var("TG_BOT_TOKEN").ok();
    let has_bot = bot_token
        .as_ref()
        .map(|t| !t.trim().is_empty())
        .unwrap_or(false);
    let default_mode = parse_default_transport_mode(
        &std::env::var("TG_TRANSPORT_MODE").unwrap_or_default(),
        has_bot,
    );
    let (telegram_api_id, telegram_api_hash) = load_user_api_credentials(default_mode, has_bot)
        .unwrap_or((123456, "your_api_hash_here".to_string()));
    std::sync::Arc::new(ServerConfig {
        bind_host: "127.0.0.1".to_string(),
        port,
        stream_port,
        data_dir: data_dir.clone(),
        access_pwd: crate::commands::api_settings::load_local_access_pwd(&data_dir),
        api_key: None,
        api_key_hash: key_hash,
        telegram_api_id,
        telegram_api_hash,
        default_transport_mode: default_mode,
        bot_token,
        extra_bot_tokens: vec![],
        storage_channel_id: std::env::var("TG_STORAGE_CHANNEL_ID").ok(),
        bot_api_base: "https://api.telegram.org".to_string(),
        bot_rate_limit_ms: 3500,
        base_url: format!("http://127.0.0.1:{port}"),
        static_dir: resolve_desktop_web_static_dir(resource_dir.as_deref())
            .unwrap_or_else(|| data_dir.clone()),
        docs_dir: resolve_desktop_web_static_dir(resource_dir.as_deref())
            .map(|d| d.parent().unwrap_or(&d).join("docs"))
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| data_dir.join("docs")),
        download_threads: 4,
        chunk_size_mb: 20,
        chunk_concurrent: 4,
        files_concurrent: 3,
        max_upload_size_mb: 2000,
        cors_origins: vec![],
        rate_limit: RateLimitConfig {
            ip_rpm: 600,
            api_key_rpm: 600,
        },
        metadata_cache_enabled: true,
        metadata_cache_ttl_secs: 300,
        public_file_id_download: false,
        upload_share_ttl_hours: 72,
        download_signing_secret: None,
        multi_tenant_enabled: false,
        upload_link_ttl_secs: 3600,
        presigned_max_downloads: None,
        webdav_enabled: false,
        webdav_prefix: "/webdav".to_string(),
        metrics_enabled: false,
        upload_queue_backend: "memory".to_string(),
        redis_url: None,
        s3_endpoint: None,
        s3_region: None,
        s3_bucket: None,
        s3_access_key: None,
        s3_secret_key: None,
        tg_dc_addr: std::env::var("TG_DC_ADDR")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "149.154.167.50:443".to_string()),
    })
}

fn parse_cors_origins() -> Vec<String> {
    std::env::var("CORS_ORIGINS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Minimal config for unit/integration tests (no `.env` required).
#[cfg(any(test, feature = "headless-server"))]
pub fn test_config() -> std::sync::Arc<ServerConfig> {
    use crate::telegram_transport::TelegramTransportMode;
    use std::sync::Arc;
    Arc::new(ServerConfig {
        bind_host: "127.0.0.1".to_string(),
        port: 1334,
        stream_port: 14201,
        data_dir: std::env::temp_dir().join("td-test-data"),
        access_pwd: "test-pwd".to_string(),
        api_key: Some("test-api-key".to_string()),
        api_key_hash: Some(crate::commands::api_settings::hash_key_public(
            "test-api-key",
        )),
        telegram_api_id: 12345,
        telegram_api_hash: "ci_dummy_hash_not_for_production".to_string(),
        default_transport_mode: TelegramTransportMode::Bot,
        bot_token: Some("1:fake".to_string()),
        extra_bot_tokens: vec!["2:fake".to_string()],
        storage_channel_id: Some("-1001".to_string()),
        bot_api_base: "https://api.telegram.org".to_string(),
        bot_rate_limit_ms: 3500,
        base_url: "http://127.0.0.1:1334".to_string(),
        static_dir: std::path::PathBuf::from("deploy/web"),
        docs_dir: std::path::PathBuf::from("docs"),
        download_threads: 4,
        chunk_size_mb: 10,
        chunk_concurrent: 4,
        files_concurrent: 2,
        max_upload_size_mb: 100,
        cors_origins: vec![],
        rate_limit: RateLimitConfig {
            ip_rpm: 600,
            api_key_rpm: 600,
        },
        metadata_cache_enabled: true,
        metadata_cache_ttl_secs: 300,
        public_file_id_download: false,
        upload_share_ttl_hours: 0,
        download_signing_secret: Some("test-signing-secret-32chars-min!!".to_string()),
        multi_tenant_enabled: true,
        upload_link_ttl_secs: 0,
        presigned_max_downloads: None,
        webdav_enabled: false,
        webdav_prefix: "/webdav".to_string(),
        metrics_enabled: true,
        upload_queue_backend: "memory".to_string(),
        redis_url: None,
        s3_endpoint: None,
        s3_region: None,
        s3_bucket: None,
        s3_access_key: None,
        s3_secret_key: None,
        tg_dc_addr: "149.154.167.50:443".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_chunk_bytes_from_config() {
        let cfg = test_config();
        assert_eq!(cfg.max_chunk_bytes(), 10 * 1024 * 1024);
    }

    #[test]
    fn all_bot_tokens_merges_primary_and_extra() {
        let cfg = test_config();
        let tokens = cfg.all_bot_tokens();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], "1:fake");
        assert_eq!(tokens[1], "2:fake");
    }

    #[test]
    fn parse_cors_splits_comma_list() {
        std::env::set_var(
            "CORS_ORIGINS",
            "https://a.example.com, http://127.0.0.1:8080",
        );
        let origins = parse_cors_origins();
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0], "https://a.example.com");
        std::env::remove_var("CORS_ORIGINS");
    }

    #[test]
    fn desktop_static_servable_requires_telegram_html() {
        let manifest_web =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../deploy/web");
        if manifest_web.join("telegram.html").is_file() {
            assert!(desktop_static_servable(&manifest_web));
        }
        let tmp = std::env::temp_dir().join(format!("td-no-static-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        assert!(!desktop_static_servable(&tmp));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_desktop_web_static_dir_finds_manifest_deploy_web() {
        let resolved = resolve_desktop_web_static_dir(None);
        let manifest_web =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../deploy/web");
        if manifest_web.join("telegram.html").is_file() {
            assert!(resolved.is_some());
            assert!(desktop_static_servable(resolved.as_ref().unwrap()));
        }
    }
}
