//! VPN Optimizer & Proxy Configuration
//!
//! Stores runtime network configuration that all network operations read from.
//! When vpnMode is off, helpers return hardcoded defaults (zero behaviour change).
//! When vpnMode is on, helpers return user-configured values.

use serde::{Deserialize, Serialize};
use std::future::Future;
#[cfg(feature = "desktop")]
use tauri::Manager;
use tokio::sync::RwLock;

/// Proxy configuration received from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub proxy_type: String, // "socks5" | "mtproto"
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String, // SOCKS5
    pub secret: String,   // MTProto
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_type: "socks5".into(),
            host: String::new(),
            port: 1080,
            username: String::new(),
            password: String::new(),
            secret: String::new(),
        }
    }
}

/// VPN optimizer configuration received from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnConfig {
    pub enabled: bool,
    pub timeout_multiplier: u32,    // 1–5
    pub retry_attempts: u32,        // 0–5
    pub retry_base_backoff_ms: u64, // 500–5000
    pub retry_max_backoff_ms: u64,  // 8000–60000
    pub adaptive_polling: bool,
    pub polling_min_sec: u32,      // 10–30
    pub polling_max_sec: u32,      // 45–120
    pub preferred_dc: String,      // "auto" | "dc1"–"dc5"
    pub dc_fallback_attempts: u32, // 1–4
    pub flood_wait_respect: bool,
    pub peer_cache_size: usize,        // 100–2000
    pub bandwidth_limit_up_kbs: u32,   // 0 = unlimited
    pub bandwidth_limit_down_kbs: u32, // 0 = unlimited
    pub chunk_size_kb: u32,            // 128, 256, 512
    pub keep_alive_interval_sec: u32,  // 0 = disabled, 30–120
    pub auto_detect_vpn: bool,
}

impl Default for VpnConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_multiplier: 3,
            retry_attempts: 3,
            retry_base_backoff_ms: 1000,
            retry_max_backoff_ms: 30000,
            adaptive_polling: true,
            polling_min_sec: 15,
            polling_max_sec: 60,
            preferred_dc: "auto".into(),
            dc_fallback_attempts: 2,
            flood_wait_respect: true,
            peer_cache_size: 500,
            bandwidth_limit_up_kbs: 0,
            bandwidth_limit_down_kbs: 0,
            chunk_size_kb: 512,
            keep_alive_interval_sec: 0,
            auto_detect_vpn: false,
        }
    }
}

/// Combined network config snapshot (what the frontend receives)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfigSnapshot {
    pub proxy: ProxyConfig,
    pub vpn: VpnConfig,
}

/// Thread-safe global state managed via Tauri's state system
pub struct NetworkConfig {
    pub proxy: RwLock<ProxyConfig>,
    pub vpn: RwLock<VpnConfig>,
}

impl NetworkConfig {
    pub fn new() -> Self {
        Self {
            proxy: RwLock::new(ProxyConfig::default()),
            vpn: RwLock::new(VpnConfig::default()),
        }
    }

    pub fn new_with_config(config: NetworkConfigSnapshot) -> Self {
        Self {
            proxy: RwLock::new(config.proxy),
            vpn: RwLock::new(config.vpn),
        }
    }

    fn vpn_config(&self) -> VpnConfig {
        self.vpn
            .try_read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn proxy_config(&self) -> ProxyConfig {
        self.proxy
            .try_read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn snapshot(&self) -> NetworkConfigSnapshot {
        NetworkConfigSnapshot {
            proxy: self.proxy_config(),
            vpn: self.vpn_config(),
        }
    }

    // ── Helpers that return effective values ────────────────

    /// Network connect timeout in seconds. Default 5s, multiplied when VPN mode on.
    pub fn connect_timeout_secs(&self) -> u64 {
        let vpn = self.vpn_config();
        if vpn.enabled {
            5 * vpn.timeout_multiplier as u64
        } else {
            5
        }
    }

    /// Network read/write timeout in seconds. Default 10s, multiplied when VPN mode on.
    pub fn rw_timeout_secs(&self) -> u64 {
        let vpn = self.vpn_config();
        if vpn.enabled {
            10 * vpn.timeout_multiplier as u64
        } else {
            10
        }
    }

    /// How many retry attempts for API calls. Default 0 (no retry) when VPN off.
    pub fn retry_attempts(&self) -> u32 {
        let vpn = self.vpn_config();
        if vpn.enabled {
            vpn.retry_attempts
        } else {
            0
        }
    }

    /// Base backoff duration in milliseconds for retries.
    pub fn retry_base_backoff_ms(&self) -> u64 {
        let vpn = self.vpn_config();
        if vpn.enabled {
            vpn.retry_base_backoff_ms
        } else {
            1000
        }
    }

    /// Max backoff duration in milliseconds for retries.
    pub fn retry_max_backoff_ms(&self) -> u64 {
        let vpn = self.vpn_config();
        if vpn.enabled {
            vpn.retry_max_backoff_ms
        } else {
            30000
        }
    }

    /// Whether to automatically sleep on FLOOD_WAIT errors.
    pub fn should_respect_flood_wait(&self) -> bool {
        let vpn = self.vpn_config();
        if vpn.enabled {
            vpn.flood_wait_respect
        } else {
            false
        }
    }

    /// Peer cache size. Default 500.
    pub fn peer_cache_size(&self) -> usize {
        let vpn = self.vpn_config();
        if vpn.enabled {
            vpn.peer_cache_size
        } else {
            500
        }
    }

    /// Whether proxy is active and has a valid host.
    pub fn is_proxy_active(&self) -> bool {
        let proxy = self.proxy_config();
        proxy.enabled && !proxy.host.is_empty()
    }

    /// Get proxy address as "host:port" if active.
    pub fn proxy_addr(&self) -> Option<String> {
        let proxy = self.proxy_config();
        if proxy.enabled && !proxy.host.is_empty() {
            Some(format!("{}:{}", proxy.host, proxy.port))
        } else {
            None
        }
    }

    /// Upload bandwidth limit in bytes/sec. 0 = unlimited.
    pub fn upload_limit_bytes_per_sec(&self) -> u64 {
        let vpn = self.vpn_config();
        if vpn.enabled && vpn.bandwidth_limit_up_kbs > 0 {
            vpn.bandwidth_limit_up_kbs as u64 * 1024
        } else {
            0 // unlimited
        }
    }

    /// Download bandwidth limit in bytes/sec. 0 = unlimited.
    pub fn download_limit_bytes_per_sec(&self) -> u64 {
        let vpn = self.vpn_config();
        if vpn.enabled && vpn.bandwidth_limit_down_kbs > 0 {
            vpn.bandwidth_limit_down_kbs as u64 * 1024
        } else {
            0 // unlimited
        }
    }

    /// Chunk size in bytes for transfers.
    pub fn chunk_size_bytes(&self) -> usize {
        let vpn = self.vpn_config();
        if vpn.enabled {
            (vpn.chunk_size_kb as usize) * 1024
        } else {
            512 * 1024 // default 512KB
        }
    }

    /// Keep-alive ping interval in seconds. 0 = disabled.
    pub fn keep_alive_interval_sec(&self) -> u32 {
        let vpn = self.vpn_config();
        if vpn.enabled {
            vpn.keep_alive_interval_sec
        } else {
            0
        }
    }

    /// Chunk size for grammers `iter_download` (minimum 4096 for range skip alignment).
    pub fn download_chunk_i32(&self) -> i32 {
        self.chunk_size_bytes().max(4096) as i32
    }

    /// Adaptive polling interval for UI / network health checks (milliseconds).
    pub fn polling_interval_ms(&self, last_check_ok: bool) -> u64 {
        let vpn = self.vpn_config();
        if !vpn.enabled || !vpn.adaptive_polling {
            return 10_000;
        }
        if last_check_ok {
            (vpn.polling_min_sec as u64).saturating_mul(1000)
        } else {
            (vpn.polling_max_sec as u64).saturating_mul(1000)
        }
    }
}

/// Sleep to respect a download/upload bytes-per-second cap (best-effort).
pub async fn throttle_transfer_bytes(
    delta_bytes: u64,
    limit_bps: u64,
    window_bytes: &mut u64,
    window_start: &mut std::time::Instant,
) {
    if limit_bps == 0 || delta_bytes == 0 {
        return;
    }
    *window_bytes += delta_bytes;
    let elapsed = window_start.elapsed().as_secs_f64().max(0.001);
    let rate = *window_bytes as f64 / elapsed;
    if rate > limit_bps as f64 {
        let target = *window_bytes as f64 / limit_bps as f64;
        let sleep_secs = target - elapsed;
        if sleep_secs > 0.0 && sleep_secs < 5.0 {
            tokio::time::sleep(std::time::Duration::from_secs_f64(sleep_secs)).await;
        }
    }
}

/// AsyncRead wrapper that enforces an upload bandwidth cap.
pub struct ThrottledReader<R> {
    inner: R,
    limit_bps: u64,
    window_bytes: u64,
    window_start: std::time::Instant,
    sleep: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
}

impl<R: Unpin> Unpin for ThrottledReader<R> {}

impl<R> ThrottledReader<R> {
    pub fn new(inner: R, limit_bps: u64) -> Self {
        Self {
            inner,
            limit_bps,
            window_bytes: 0,
            window_start: std::time::Instant::now(),
            sleep: None,
        }
    }
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for ThrottledReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if let Some(s) = self.sleep.as_mut() {
            match s.as_mut().poll(cx) {
                std::task::Poll::Ready(()) => {
                    self.sleep = None;
                    self.window_bytes = 0;
                    self.window_start = std::time::Instant::now();
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }

        let before = buf.filled().len();
        match std::pin::Pin::new(&mut self.inner).poll_read(cx, buf) {
            std::task::Poll::Ready(Ok(())) => {
                let n = (buf.filled().len().saturating_sub(before)) as u64;
                if self.limit_bps > 0 && n > 0 {
                    self.window_bytes += n;
                    let elapsed = self.window_start.elapsed().as_secs_f64().max(0.001);
                    let rate = self.window_bytes as f64 / elapsed;
                    if rate > self.limit_bps as f64 {
                        let need = self.window_bytes as f64 / self.limit_bps as f64 - elapsed;
                        if need > 0.0 && need < 5.0 {
                            self.sleep = Some(Box::pin(tokio::time::sleep(
                                std::time::Duration::from_secs_f64(need),
                            )));
                            cx.waker().wake_by_ref();
                            return std::task::Poll::Pending;
                        }
                    }
                }
                std::task::Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

/// Parse `FLOOD_WAIT_N` seconds from a mapped error string; capped at 300.
pub fn parse_flood_wait_secs(err: &str) -> Option<u64> {
    if !err.starts_with("FLOOD_WAIT_") {
        return None;
    }
    err.trim_start_matches("FLOOD_WAIT_")
        .parse::<u64>()
        .ok()
        .map(|s| s.min(300))
}

/// Compute exponential backoff with jitter for a given attempt.
/// Returns duration in milliseconds.
pub fn backoff_ms(attempt: u32, base_ms: u64, max_ms: u64) -> u64 {
    let exp = base_ms.saturating_mul(1u64 << attempt.min(10));
    let capped = exp.min(max_ms);
    // Add ~25% jitter
    let jitter = (capped as f64 * 0.25 * rand::random::<f64>()) as u64;
    capped + jitter
}

#[cfg(feature = "desktop")]
fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("network_settings.json"))
}

pub fn load_network_config_at(data_dir: &std::path::Path) -> NetworkConfigSnapshot {
    let path = data_dir.join("network_settings.json");
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| NetworkConfigSnapshot {
            proxy: ProxyConfig::default(),
            vpn: VpnConfig::default(),
        }),
        Err(_) => NetworkConfigSnapshot {
            proxy: ProxyConfig::default(),
            vpn: VpnConfig::default(),
        },
    }
}

#[cfg(feature = "desktop")]
pub fn load_network_config(app: &tauri::AppHandle) -> NetworkConfigSnapshot {
    let path = match settings_path(app) {
        Ok(p) => p,
        Err(_) => {
            return NetworkConfigSnapshot {
                proxy: ProxyConfig::default(),
                vpn: VpnConfig::default(),
            }
        }
    };
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|_| NetworkConfigSnapshot {
            proxy: ProxyConfig::default(),
            vpn: VpnConfig::default(),
        }),
        Err(_) => NetworkConfigSnapshot {
            proxy: ProxyConfig::default(),
            vpn: VpnConfig::default(),
        },
    }
}

#[cfg(feature = "desktop")]
pub fn save_network_config(
    app: &tauri::AppHandle,
    config: &NetworkConfigSnapshot,
) -> Result<(), String> {
    let path = settings_path(app)?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

pub fn save_network_config_at(
    data_dir: &std::path::Path,
    config: &NetworkConfigSnapshot,
) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = data_dir.join("network_settings.json");
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

// ── Unified retry wrapper for all Telegram API calls ───────────────────────

/// Execute an async operation with VPN-aware retry logic.
///
/// - When VPN mode is off: zero retries, fast failure (no latency penalty)
/// - When VPN mode is on: configurable retries with exponential backoff + jitter
/// - FLOOD_WAIT errors are respected if configured (sleep then retry, not counting against retry budget)
///
/// # Usage
/// ```rust,ignore
/// let result = with_retry(&net_config, || async {
///     client.send_message(&peer, message.clone()).await.map_err(|e| e.to_string())
/// }, "send_message").await?;
/// ```
pub async fn with_retry<F, Fut, T>(
    net_config: &std::sync::Arc<NetworkConfig>,
    operation: F,
    context: &str,
) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let max_retries = net_config.retry_attempts();
    let base_ms = net_config.retry_base_backoff_ms();
    let max_ms = net_config.retry_max_backoff_ms();
    let respect_flood = net_config.should_respect_flood_wait();
    let mut last_err = String::new();

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                log::warn!(
                    "[retry] {} attempt {}/{} failed: {}",
                    context,
                    attempt + 1,
                    max_retries + 1,
                    err
                );

                // FLOOD_WAIT is a rate-limit signal, not a failure — sleep and retry without consuming budget
                if respect_flood {
                    if let Some(wait) = parse_flood_wait_secs(&err) {
                        log::info!(
                            "[retry] Respecting FLOOD_WAIT for '{}': sleeping {}s",
                            context,
                            wait
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                        last_err = err;
                        continue; // does NOT count as a retry attempt
                    }
                }

                last_err = err;
                if attempt < max_retries {
                    let delay = backoff_ms(attempt, base_ms, max_ms);
                    log::info!("[retry] Retrying '{}' in {}ms...", context, delay);
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
            }
        }
    }

    Err(format!(
        "[retry] {} failed after {} attempts: {}",
        context,
        max_retries + 1,
        last_err
    ))
}

/// Retry wrapper for Telegram client operations that return a displayable error.
/// Maps the error via `map_error` before retry logic.
///
/// # Usage
/// ```rust,ignore
/// let result = with_retry_telegram(
///     &net_config,
///     || client.invoke(&some_tl_function),
///     "invoke",
/// ).await?;
/// ```
pub async fn with_retry_telegram<F, Fut, T, E>(
    net_config: &std::sync::Arc<NetworkConfig>,
    operation: F,
    context: &str,
) -> Result<T, String>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    with_retry(
        net_config,
        || async {
            operation()
                .await
                .map_err(|e| crate::commands::utils::map_error(e))
        },
        context,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_respects_max() {
        let v = backoff_ms(20, 1000, 5000);
        assert!(v >= 5000 && v <= 6250);
    }

    #[test]
    fn flood_wait_parsed_and_capped() {
        assert_eq!(parse_flood_wait_secs("FLOOD_WAIT_42"), Some(42));
        assert_eq!(parse_flood_wait_secs("FLOOD_WAIT_9999"), Some(300));
        assert_eq!(parse_flood_wait_secs("NETWORK"), None);
    }

    #[test]
    fn retry_attempts_zero_when_vpn_off() {
        let cfg = NetworkConfig::new();
        assert_eq!(cfg.retry_attempts(), 0);
    }

    #[tokio::test]
    async fn keep_alive_interval_safe_inside_async_runtime() {
        let cfg = NetworkConfig::new();
        assert_eq!(cfg.keep_alive_interval_sec(), 0);
        assert_eq!(cfg.polling_interval_ms(true), 10_000);
    }

    #[tokio::test]
    async fn config_helpers_safe_inside_async_runtime() {
        let cfg = NetworkConfig::new();
        assert_eq!(cfg.connect_timeout_secs(), 5);
        assert_eq!(cfg.rw_timeout_secs(), 10);
        assert_eq!(cfg.retry_attempts(), 0);
        assert!(!cfg.is_proxy_active());
        assert_eq!(cfg.chunk_size_bytes(), 512 * 1024);
        assert_eq!(cfg.snapshot().vpn.enabled, false);
    }
}
