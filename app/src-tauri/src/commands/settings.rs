//! Tauri commands for applying proxy and VPN optimizer settings.
//! These are called from the frontend when the user changes network configuration.

use tauri::State;
use crate::vpn_optimizer::{NetworkConfig, ProxyConfig, VpnConfig, NetworkConfigSnapshot};

/// Apply proxy settings from the frontend.
/// Stores the config in global state so network operations can read it.
#[tauri::command]
pub async fn cmd_apply_proxy_settings(
    enabled: bool,
    proxy_type: String,
    host: String,
    port: u16,
    username: String,
    password: String,
    secret: String,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let config = ProxyConfig {
        enabled,
        proxy_type,
        host,
        port,
        username,
        password,
        secret,
    };

    log::info!(
        "Applying proxy settings: enabled={}, type={}, host={}:{}",
        config.enabled, config.proxy_type, config.host, config.port
    );

    *net_config.proxy.write().map_err(|e| e.to_string())? = config;

    let snapshot = net_config.snapshot();
    if let Err(e) = crate::vpn_optimizer::save_network_config(&app, &snapshot) {
        log::error!("Failed to save proxy settings to disk: {}", e);
    }

    Ok("Proxy settings applied".into())
}

/// Apply VPN optimizer settings from the frontend.
/// Stores the config in global state so network operations can read it.
#[tauri::command]
pub async fn cmd_apply_vpn_settings(
    enabled: bool,
    timeout_multiplier: u32,
    retry_attempts: u32,
    retry_base_backoff_ms: u64,
    retry_max_backoff_ms: u64,
    adaptive_polling: bool,
    polling_min_sec: u32,
    polling_max_sec: u32,
    preferred_dc: String,
    dc_fallback_attempts: u32,
    flood_wait_respect: bool,
    peer_cache_size: usize,
    bandwidth_limit_up_kbs: u32,
    bandwidth_limit_down_kbs: u32,
    chunk_size_kb: u32,
    keep_alive_interval_sec: u32,
    auto_detect_vpn: bool,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let config = VpnConfig {
        enabled,
        timeout_multiplier: timeout_multiplier.clamp(1, 5),
        retry_attempts: retry_attempts.clamp(0, 5),
        retry_base_backoff_ms: retry_base_backoff_ms.clamp(500, 5000),
        retry_max_backoff_ms: retry_max_backoff_ms.clamp(8000, 60000),
        adaptive_polling,
        polling_min_sec: polling_min_sec.clamp(10, 30),
        polling_max_sec: polling_max_sec.clamp(45, 120),
        preferred_dc,
        dc_fallback_attempts: dc_fallback_attempts.clamp(1, 4),
        flood_wait_respect,
        peer_cache_size: peer_cache_size.clamp(100, 2000),
        bandwidth_limit_up_kbs,
        bandwidth_limit_down_kbs,
        chunk_size_kb: chunk_size_kb.clamp(64, 512),
        keep_alive_interval_sec: if keep_alive_interval_sec == 0 { 0 } else { keep_alive_interval_sec.clamp(30, 120) },
        auto_detect_vpn,
    };

    log::info!(
        "Applying VPN settings: enabled={}, timeout={}x, retries={}, flood_wait={}",
        config.enabled, config.timeout_multiplier, config.retry_attempts, config.flood_wait_respect
    );

    *net_config.vpn.write().map_err(|e| e.to_string())? = config;

    let snapshot = net_config.snapshot();
    if let Err(e) = crate::vpn_optimizer::save_network_config(&app, &snapshot) {
        log::error!("Failed to save VPN settings to disk: {}", e);
    }

    Ok("VPN settings applied".into())
}

/// Get current network configuration snapshot (called on startup / settings load).
#[tauri::command]
pub async fn cmd_get_network_config(
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
) -> Result<NetworkConfigSnapshot, String> {
    Ok(net_config.snapshot())
}
