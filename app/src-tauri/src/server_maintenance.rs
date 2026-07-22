//! Background maintenance for headless 7×24 deployment: periodic DB cleanup.

use std::sync::Arc;
use std::time::Duration;

use crate::db::DbConnection;
use crate::server_config::ServerConfig;

const DEFAULT_INTERVAL_SECS: u64 = 3600;
const METADATA_MAX_AGE_SECS: u64 = 7 * 24 * 3600;

/// Run expired-share / stale-upload / metadata-cache cleanup once.
pub fn run_maintenance_pass(db: &DbConnection, config: &ServerConfig) {
    match crate::db::cleanup_expired_shares(db) {
        Ok(n) if n > 0 => log::info!("maintenance: removed {n} expired share(s)"),
        Err(e) => log::warn!("maintenance: share cleanup failed: {e}"),
        _ => {}
    }
    match crate::db::cleanup_stale_uploads(db) {
        Ok(n) if n > 0 => log::info!("maintenance: removed {n} stale upload session(s)"),
        Err(e) => log::warn!("maintenance: upload cleanup failed: {e}"),
        _ => {}
    }
    if config.metadata_cache_enabled {
        match crate::metadata_cache::cleanup_stale(db, METADATA_MAX_AGE_SECS) {
            Ok(n) if n > 0 => log::info!("maintenance: removed {n} stale metadata cache row(s)"),
            Err(e) => log::warn!("maintenance: metadata cache cleanup failed: {e}"),
            _ => {}
        }
    }
}

fn maintenance_interval_secs() -> u64 {
    std::env::var("MAINTENANCE_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n| n >= 60)
        .unwrap_or(DEFAULT_INTERVAL_SECS)
}

/// Spawn a tokio task that runs maintenance on an interval until cancelled.
pub fn spawn_periodic_maintenance(
    db: DbConnection,
    config: Arc<ServerConfig>,
) -> tokio::task::JoinHandle<()> {
    let interval = maintenance_interval_secs();
    log::info!("maintenance: periodic cleanup every {interval}s (MAINTENANCE_INTERVAL_SECS)");
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(interval));
        tick.tick().await; // skip immediate duplicate (startup already ran once)
        loop {
            tick.tick().await;
            run_maintenance_pass(&db, &config);
        }
    })
}

/// Wait for Ctrl+C (all platforms) or SIGTERM (Unix / Docker stop).
pub async fn wait_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Received Ctrl+C");
            }
            _ = term.recv() => {
                log::info!("Received SIGTERM");
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        log::info!("Received Ctrl+C");
    }
}

fn bot_keepalive_interval_secs() -> u64 {
    std::env::var("BOT_KEEPALIVE_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(|h| h.saturating_mul(3600).max(3600))
        .unwrap_or(24 * 3600)
}

/// Periodic Bot API ping (tgDrive-style keepalive). `BOT_KEEPALIVE_HOURS=0` disables.
pub fn spawn_bot_keepalive(config: Arc<ServerConfig>) -> Option<tokio::task::JoinHandle<()>> {
    let hours_raw = std::env::var("BOT_KEEPALIVE_HOURS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(24);
    if hours_raw == 0 {
        return None;
    }
    if !crate::telegram_transport::TransportHandle::bot_configured(&config) {
        return None;
    }
    let interval = bot_keepalive_interval_secs();
    log::info!(
        "bot keepalive: every {}s (BOT_KEEPALIVE_HOURS={})",
        interval,
        hours_raw
    );
    Some(tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(interval));
        tick.tick().await;
        loop {
            tick.tick().await;
            match crate::telegram_transport::bot_test_connection_cached(&config).await {
                Ok(username) => log::debug!("bot keepalive OK (@{username})"),
                Err(e) => log::warn!("bot keepalive failed: {e}"),
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_interval_defaults_to_one_hour() {
        std::env::remove_var("MAINTENANCE_INTERVAL_SECS");
        assert_eq!(maintenance_interval_secs(), 3600);
    }

    #[test]
    fn maintenance_interval_respects_minimum() {
        std::env::set_var("MAINTENANCE_INTERVAL_SECS", "10");
        assert_eq!(maintenance_interval_secs(), 3600);
        std::env::remove_var("MAINTENANCE_INTERVAL_SECS");
    }

    #[test]
    fn bot_keepalive_zero_disables_spawn() {
        std::env::set_var("BOT_KEEPALIVE_HOURS", "0");
        let cfg = crate::server_config::test_config();
        assert!(spawn_bot_keepalive(cfg).is_none());
        std::env::remove_var("BOT_KEEPALIVE_HOURS");
    }
}