use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use app_lib::server_config::ServerConfig;
use app_lib::server_http::{bootstrap_telegram, start_unified_server, ServerRuntime};
use app_lib::commands::TelegramState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    app_lib::logging::init_from_env();
    app_lib::server_uptime::mark_started();

    let config = Arc::new(ServerConfig::from_env()?);
    config.warn_insecure_defaults();
    config.ensure_api_settings_file()?;

    if let Some(hint) = config.telegram_credentials_placeholder() {
        log::warn!("{hint}");
    }
    if !config.docs_dir.is_dir() {
        log::warn!(
            "DOCS_DIR {} is missing — set DOCS_DIR=/app/docs in Docker or mount ./docs",
            config.docs_dir.display()
        );
    }

    let net_snapshot = app_lib::vpn_optimizer::load_network_config_at(&config.data_dir);
    let net_config = Arc::new(app_lib::vpn_optimizer::NetworkConfig::new_with_config(
        net_snapshot,
    ));

    app_lib::settings_routes::maybe_auto_enable_vpn_on_startup(&net_config, &config.data_dir).await;

    let bot_pool = Arc::new(app_lib::bot_pool::BotPool::new(config.all_bot_tokens()));

    let tg_state = Arc::new(TelegramState {
        client: Arc::new(tokio::sync::Mutex::new(None)),
        login_token: Arc::new(tokio::sync::Mutex::new(None)),
        password_token: Arc::new(tokio::sync::Mutex::new(None)),
        api_id: Arc::new(tokio::sync::Mutex::new(None)),
        runner_shutdown: Arc::new(std::sync::Mutex::new(None)),
        runner_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        peer_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        cancelled_transfers: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
        bot_pool: bot_pool.clone(),
    });

    let db = app_lib::db::init_db_at(&config.data_dir)?;

    if let Err(e) = app_lib::tenant_auth::bootstrap_tenants(&db, &config) {
        log::warn!("tenant bootstrap: {e}");
    }
    if let Some(ref secret) = config.download_signing_secret {
        if secret.len() < 32 {
            log::warn!("DOWNLOAD_SIGNING_SECRET is shorter than 32 characters — presigned URLs may be rejected");
        }
    } else if config.upload_share_ttl_hours <= 0 {
        log::warn!(
            "No DOWNLOAD_SIGNING_SECRET and UPLOAD_SHARE_TTL_HOURS=0 — uploads will not receive a public download_url unless PUBLIC_FILE_ID_DOWNLOAD=true"
        );
    }

    app_lib::server_maintenance::run_maintenance_pass(&db, &config);
    let _maintenance_task = app_lib::server_maintenance::spawn_periodic_maintenance(
        db.clone(),
        config.clone(),
    );
    let _bot_keepalive = app_lib::server_maintenance::spawn_bot_keepalive(config.clone());

    // VPN keep-alive (same as desktop) when configured in network_settings.json
    {
        let ka_config = net_config.clone();
        tokio::spawn(async move {
            loop {
                let interval = ka_config.keep_alive_interval_sec();
                if interval == 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    continue;
                }
                tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
                let dc_addr: std::net::SocketAddr = std::env::var("TG_DC_ADDR")
                    .unwrap_or_else(|_| "149.154.167.50:443".to_string())
                    .parse()
                    .unwrap_or_else(|_| "149.154.167.50:443".parse().expect("default DC addr"));
                let _ = tokio::task::spawn_blocking(move || {
                    use std::net::TcpStream;
                    let _ = TcpStream::connect_timeout(
                        &dc_addr,
                        std::time::Duration::from_secs(5),
                    );
                }).await;
            }
        });
    }

    let stream_token: String = (0..16)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();

    let upload_gate = Arc::new(app_lib::upload_gate::build_upload_gate(&config));

    let runtime = Arc::new(ServerRuntime {
        tg_state: tg_state.clone(),
        db,
        net_config: net_config.clone(),
        transport: Arc::new(app_lib::telegram_transport::TransportHandle::new(
            &config.data_dir,
            config.default_transport_mode,
        )),
        upload_gate,
        upload_progress: Arc::new(app_lib::upload_progress::UploadProgressHub::new()),
        stream_token,
        api_running: Arc::new(AtomicBool::new(true)),
    });

    if let Err(e) = bootstrap_telegram(&config, &runtime).await {
        log::error!("Telegram bootstrap: {e}");
    }

    let server = start_unified_server(config.clone(), runtime.clone()).await?;
    let handle = server.handle();

    tokio::spawn(async move {
        app_lib::server_maintenance::wait_shutdown_signal().await;
        log::info!("Graceful shutdown: stopping HTTP server…");
        handle.stop(true).await;
        if app_lib::commands::signal_runner_shutdown(&runtime.tg_state.runner_shutdown) {
            log::info!("Signaled grammers runner shutdown");
        }
    });

    server.await?;
    log::info!("Telegram Drive API server stopped");
    Ok(())
}
