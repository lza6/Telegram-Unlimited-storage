//! Admin settings REST: share domain + network config for Web/Headless.

use actix_web::{get, put, web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::server_config::ServerConfig;
use crate::ui_settings::{load_ui_settings, save_ui_settings};
use crate::vpn_optimizer::{NetworkConfig, NetworkConfigSnapshot, ProxyConfig, VpnConfig};

#[derive(Clone)]
pub struct SettingsRouteState {
    pub config: Arc<ServerConfig>,
    pub use_stream_port_for_shares: bool,
}

fn effective_share_link_base(req: &HttpRequest, state: &SettingsRouteState) -> String {
    if state.use_stream_port_for_shares {
        crate::ui_settings::share_base_url_from_data_dir(&state.config.data_dir, state.config.stream_port)
    } else {
        crate::ui_settings::effective_base_url(req, &state.config)
    }
}

/// Web admin password or valid global API key.
fn require_admin(req: &HttpRequest, config: &ServerConfig) -> Option<HttpResponse> {
    crate::admin_routes::require_admin_or_api_key(req, config)
}

#[derive(Serialize)]
struct SettingsResponse {
    share_domain: String,
    env_base_url: String,
    effective_base_url: String,
    /// Canonical base for `/d/*` share links (stream port on desktop).
    effective_share_base_url: String,
    /// Actual base used when generating share links for this server instance.
    effective_share_link_base: String,
    chunk_size_mb: u32,
    chunk_concurrent: u32,
    files_concurrent: u32,
    download_threads: u32,
    stream_port: u16,
    max_upload_size_mb: u32,
}

#[derive(Deserialize)]
struct SettingsUpdateBody {
    share_domain: Option<String>,
}

#[derive(Serialize)]
struct NetworkResponse {
    proxy: ProxyConfigPublic,
    vpn: VpnConfig,
}

#[derive(Serialize, Clone)]
struct ProxyConfigPublic {
    pub enabled: bool,
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password_set: bool,
}

fn proxy_public(p: &ProxyConfig) -> ProxyConfigPublic {
    ProxyConfigPublic {
        enabled: p.enabled,
        proxy_type: p.proxy_type.clone(),
        host: p.host.clone(),
        port: p.port,
        username: p.username.clone(),
        password_set: !p.password.is_empty(),
    }
}

fn redacted_network(snap: &NetworkConfigSnapshot) -> NetworkResponse {
    NetworkResponse {
        proxy: proxy_public(&snap.proxy),
        vpn: snap.vpn.clone(),
    }
}

async fn apply_network_snapshot(
    net_config: &Arc<NetworkConfig>,
    data_dir: &std::path::Path,
    snap: NetworkConfigSnapshot,
) -> Result<(), String> {
    *net_config.proxy.write().await = snap.proxy.clone();
    *net_config.vpn.write().await = snap.vpn.clone();
    crate::vpn_optimizer::save_network_config_at(data_dir, &snap)
}

fn merge_proxy(base: &ProxyConfig, patch: &ProxyPatch) -> ProxyConfig {
    let mut next = base.clone();
    if let Some(v) = patch.enabled {
        next.enabled = v;
    }
    if let Some(ref v) = patch.host {
        next.host = v.trim().to_string();
    }
    if let Some(v) = patch.port {
        next.port = v;
    }
    if let Some(ref v) = patch.username {
        next.username = v.clone();
    }
    if let Some(ref v) = patch.password {
        if !v.is_empty() {
            next.password = v.clone();
        }
    }
    if let Some(ref v) = patch.secret {
        if !v.is_empty() {
            next.secret = v.clone();
        }
    }
    next
}

fn merge_vpn(base: &VpnConfig, patch: &VpnPatch) -> VpnConfig {
    let mut next = base.clone();
    if let Some(v) = patch.enabled {
        next.enabled = v;
    }
    if let Some(v) = patch.timeout_multiplier {
        next.timeout_multiplier = v.clamp(1, 5);
    }
    if let Some(v) = patch.retry_attempts {
        next.retry_attempts = v.clamp(0, 5);
    }
    if let Some(v) = patch.retry_base_backoff_ms {
        next.retry_base_backoff_ms = v.clamp(500, 5000);
    }
    if let Some(v) = patch.retry_max_backoff_ms {
        next.retry_max_backoff_ms = v.clamp(8000, 60000);
    }
    if let Some(v) = patch.adaptive_polling {
        next.adaptive_polling = v;
    }
    if let Some(v) = patch.polling_min_sec {
        next.polling_min_sec = v.clamp(10, 30);
    }
    if let Some(v) = patch.polling_max_sec {
        next.polling_max_sec = v.clamp(45, 120);
    }
    if let Some(ref v) = patch.preferred_dc {
        next.preferred_dc = v.clone();
    }
    if let Some(v) = patch.dc_fallback_attempts {
        next.dc_fallback_attempts = v.clamp(1, 4);
    }
    if let Some(v) = patch.flood_wait_respect {
        next.flood_wait_respect = v;
    }
    if let Some(v) = patch.peer_cache_size {
        next.peer_cache_size = v.clamp(100, 2000);
    }
    if let Some(v) = patch.bandwidth_limit_up_kbs {
        next.bandwidth_limit_up_kbs = v;
    }
    if let Some(v) = patch.bandwidth_limit_down_kbs {
        next.bandwidth_limit_down_kbs = v;
    }
    if let Some(v) = patch.chunk_size_kb {
        next.chunk_size_kb = v.clamp(64, 512);
    }
    if let Some(v) = patch.keep_alive_interval_sec {
        next.keep_alive_interval_sec = if v == 0 { 0 } else { v.clamp(30, 120) };
    }
    if let Some(v) = patch.auto_detect_vpn {
        next.auto_detect_vpn = v;
    }
    next
}

fn validate_network(snap: &NetworkConfigSnapshot) -> Result<(), String> {
    if snap.proxy.enabled && snap.proxy.host.trim().is_empty() {
        return Err("Proxy enabled but host is empty".into());
    }
    Ok(())
}

#[derive(Deserialize, Default)]
struct ProxyPatch {
    enabled: Option<bool>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    secret: Option<String>,
}

#[derive(Deserialize, Default)]
struct VpnPatch {
    enabled: Option<bool>,
    timeout_multiplier: Option<u32>,
    retry_attempts: Option<u32>,
    retry_base_backoff_ms: Option<u64>,
    retry_max_backoff_ms: Option<u64>,
    adaptive_polling: Option<bool>,
    polling_min_sec: Option<u32>,
    polling_max_sec: Option<u32>,
    preferred_dc: Option<String>,
    dc_fallback_attempts: Option<u32>,
    flood_wait_respect: Option<bool>,
    peer_cache_size: Option<usize>,
    bandwidth_limit_up_kbs: Option<u32>,
    bandwidth_limit_down_kbs: Option<u32>,
    chunk_size_kb: Option<u32>,
    keep_alive_interval_sec: Option<u32>,
    auto_detect_vpn: Option<bool>,
}

#[get("/api/v1/settings")]
async fn get_settings(
    req: HttpRequest,
    state: web::Data<SettingsRouteState>,
) -> impl Responder {
    if let Some(resp) = require_admin(&req, &state.config) {
        return resp;
    }
    let ui = load_ui_settings(&state.config.data_dir);
    let c = &state.config;
    HttpResponse::Ok().json(SettingsResponse {
        share_domain: ui.share_domain,
        env_base_url: c.base_url.clone(),
        effective_base_url: crate::ui_settings::effective_base_url(&req, &state.config),
        effective_share_base_url: crate::ui_settings::share_base_url_from_data_dir(
            &state.config.data_dir,
            c.stream_port,
        ),
        effective_share_link_base: effective_share_link_base(&req, &state),
        chunk_size_mb: c.chunk_size_mb,
        chunk_concurrent: c.chunk_concurrent,
        files_concurrent: c.files_concurrent,
        download_threads: c.download_threads,
        stream_port: c.stream_port,
        max_upload_size_mb: c.max_upload_size_mb,
    })
}

#[put("/api/v1/settings")]
async fn put_settings(
    req: HttpRequest,
    body: web::Json<SettingsUpdateBody>,
    state: web::Data<SettingsRouteState>,
) -> impl Responder {
    if let Some(resp) = require_admin(&req, &state.config) {
        return resp;
    }
    let mut ui = load_ui_settings(&state.config.data_dir);
    if let Some(domain) = &body.share_domain {
        ui.share_domain = domain.trim().to_string();
    }
    match save_ui_settings(&state.config.data_dir, &ui) {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({
            "ok": true,
            "share_domain": ui.share_domain,
            "effective_base_url": crate::ui_settings::effective_base_url(&req, &state.config),
            "effective_share_base_url": crate::ui_settings::share_base_url_from_data_dir(
                &state.config.data_dir,
                state.config.stream_port,
            ),
            "effective_share_link_base": effective_share_link_base(&req, &state),
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": { "code": "SAVE_FAILED", "message": e }
        })),
    }
}

#[get("/api/v1/network")]
async fn get_network(
    req: HttpRequest,
    state: web::Data<SettingsRouteState>,
    net_config: web::Data<Arc<NetworkConfig>>,
) -> impl Responder {
    if let Some(resp) = require_admin(&req, &state.config) {
        return resp;
    }
    let snap = net_config.snapshot();
    HttpResponse::Ok().json(redacted_network(&snap))
}

#[derive(Deserialize, Default)]
struct NetworkUpdateBody {
    proxy: Option<ProxyPatch>,
    vpn: Option<VpnPatch>,
}

#[put("/api/v1/network")]
async fn put_network(
    req: HttpRequest,
    body: web::Json<NetworkUpdateBody>,
    state: web::Data<SettingsRouteState>,
    net_config: web::Data<Arc<NetworkConfig>>,
) -> impl Responder {
    if let Some(resp) = require_admin(&req, &state.config) {
        return resp;
    }

    let mut snap = net_config.snapshot();

    if let Some(ref proxy) = body.proxy {
        snap.proxy = merge_proxy(&snap.proxy, proxy);
    }
    if let Some(ref vpn) = body.vpn {
        snap.vpn = merge_vpn(&snap.vpn, vpn);
    }

    if let Err(msg) = validate_network(&snap) {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": { "code": "INVALID_CONFIG", "message": msg }
        }));
    }

    match apply_network_snapshot(&net_config, &state.config.data_dir, snap.clone()).await {
        Ok(()) => HttpResponse::Ok().json(redacted_network(&snap)),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": { "code": "SAVE_FAILED", "message": e }
        })),
    }
}

pub fn configure_settings_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(get_settings)
        .service(put_settings)
        .service(get_network)
        .service(put_network);
}

/// Headless/Docker: auto-enable VPN optimizer when configured and VPN interface detected.
pub async fn maybe_auto_enable_vpn_on_startup(
    net_config: &Arc<NetworkConfig>,
    data_dir: &std::path::Path,
) {
    let should_detect = {
        let vpn = net_config.vpn.read().await;
        vpn.auto_detect_vpn && !vpn.enabled
    };
    if !should_detect {
        return;
    }
    let found = tokio::task::spawn_blocking(crate::commands::network::detect_vpn_interfaces)
        .await
        .unwrap_or(false);
    if found {
        net_config.vpn.write().await.enabled = true;
        let snapshot = net_config.snapshot();
        if let Err(e) = crate::vpn_optimizer::save_network_config_at(data_dir, &snapshot) {
            log::warn!("headless auto-detect VPN save failed: {e}");
        } else {
            log::info!("Headless: auto-detected VPN interface — VPN optimizer enabled");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_vpn_only_toggles_enabled() {
        let base = VpnConfig {
            enabled: false,
            timeout_multiplier: 4,
            retry_attempts: 2,
            ..VpnConfig::default()
        };
        let patch = VpnPatch {
            enabled: Some(true),
            ..Default::default()
        };
        let merged = merge_vpn(&base, &patch);
        assert!(merged.enabled);
        assert_eq!(merged.timeout_multiplier, 4);
        assert_eq!(merged.retry_attempts, 2);
    }

    #[test]
    fn validate_rejects_enabled_proxy_without_host() {
        let snap = NetworkConfigSnapshot {
            proxy: ProxyConfig {
                enabled: true,
                host: String::new(),
                ..ProxyConfig::default()
            },
            vpn: VpnConfig::default(),
        };
        assert!(validate_network(&snap).is_err());
    }
}
