use actix_files as fs;
use actix_web::{web, App, HttpServer};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use crate::admin_routes::AdminState;
use crate::api_routes::ApiState;
use crate::auth_routes::AuthRouteState;
use crate::commands::TelegramState;
use crate::http_middleware::{build_cors, RateLimit, RateLimiter, RequestLog, SecurityHeaders, ShareBruteForceLimiter};
use crate::server::StreamTokenData;
use crate::server_config::ServerConfig;

pub struct ServerRuntime {
    pub tg_state: Arc<TelegramState>,
    pub db: crate::db::DbConnection,
    pub net_config: Arc<crate::vpn_optimizer::NetworkConfig>,
    pub transport: Arc<crate::telegram_transport::TransportHandle>,
    pub upload_gate: Arc<crate::upload_gate::UploadGate>,
    pub upload_progress: Arc<crate::upload_progress::UploadProgressHub>,
    pub stream_token: String,
    pub api_running: Arc<AtomicBool>,
}

pub async fn start_unified_server(
    config: Arc<ServerConfig>,
    runtime: Arc<ServerRuntime>,
) -> std::io::Result<actix_web::dev::Server> {
    let bind_host = config.bind_host.clone();
    let port = config.port;
    let bind = (bind_host.as_str(), port);
    let log_host = bind_host.clone();
    let tg = runtime.tg_state.clone();
    let db = runtime.db.clone();
    let stream_token = runtime.stream_token.clone();
    let api_key_hash = config.api_key_hash.clone();
    let static_dir = config.static_dir.clone();
    let docs_dir = config.docs_dir.clone();
    let admin_cfg = config.clone();
    let auth_cfg = config.clone();
    let share_api_cfg = config.clone();
    let settings_cfg = config.clone();
    let net_config = runtime.net_config.clone();
    let cors_origins = config.cors_origins.clone();
    let rate_limiter = Arc::new(RateLimiter::new(&config.rate_limit));
    // Start background cleanup task for rate limiter (prune every 60s)
    RateLimiter::start_cleanup_task(rate_limiter.clone(), 60);
    let share_bf_limiter = web::Data::new(ShareBruteForceLimiter::new(5, 300));
    let transport_data = web::Data::new(runtime.transport.clone());
    let upload_gate = web::Data::new(runtime.upload_gate.clone());
    let upload_progress = web::Data::new(runtime.upload_progress.clone());
    let bot_pool_data = web::Data::new(runtime.tg_state.bot_pool.clone());
    let access_lockout = Arc::new(crate::access_lockout::AccessLockout::new(
        std::env::var("ACCESS_LOCKOUT_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8),
        std::env::var("ACCESS_LOCKOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300),
    ));

    if !config.docs_dir.is_dir() {
        log::warn!(
            "DOCS_DIR {} is missing or not a directory — /docs static files will fail",
            config.docs_dir.display()
        );
    }

    let server = HttpServer::new(move || {
        let cors = build_cors(&cors_origins);

        let api_state = web::Data::new(ApiState {
            key_hash: api_key_hash.clone(),
            max_upload_size_mb: config.max_upload_size_mb,
        });
        let tg_data = web::Data::new(tg.clone());
        let db_data = web::Data::new(db.clone());
        let token_data = web::Data::new(StreamTokenData {
            token: stream_token.clone(),
        });
        let admin = web::Data::new(AdminState {
            config: admin_cfg.clone(),
            db_pool: db.clone(),
            access_lockout: access_lockout.clone(),
        });
        let auth_state = web::Data::new(AuthRouteState {
            config: auth_cfg.clone(),
        });
        let net = web::Data::new(net_config.clone());

        App::new()
            .wrap(SecurityHeaders)
            .wrap(RequestLog)
            .wrap(RateLimit::new(rate_limiter.clone()))
            .wrap(cors)
            .app_data(tg_data.clone())
            .app_data(api_state.clone())
            .app_data(db_data.clone())
            .app_data(token_data)
            .app_data(admin.clone())
            .app_data(auth_state)
            .app_data(net)
            .app_data(transport_data.clone())
            .app_data(upload_gate.clone())
            .app_data(upload_progress.clone())
            .app_data(bot_pool_data.clone())
            .app_data(share_bf_limiter.clone())
            .app_data(web::Data::new(crate::share_api_routes::ShareApiState {
                config: share_api_cfg.clone(),
                use_stream_port_for_shares: false,
            }))
            .app_data(web::Data::new(crate::settings_routes::SettingsRouteState {
                config: settings_cfg.clone(),
                use_stream_port_for_shares: false,
            }))
            .configure(crate::api_routes::configure_api)
            .configure(crate::auth_routes::configure_auth)
            .configure(crate::settings_routes::configure_settings_routes)
            .configure(crate::share_api_routes::configure_share_api)
            .configure(crate::share_routes::configure_share_routes)
            .configure(crate::server::configure_stream)
            .configure(crate::admin_routes::configure_admin)
            .configure(crate::legacy_routes::configure_legacy)
            .configure(crate::upload_progress::configure_upload_progress)
            .configure(|cfg| {
                if config.webdav_enabled {
                    crate::webdav_routes::configure_webdav(cfg, &config.webdav_prefix);
                }
            })
            .configure(crate::metrics::configure_metrics)
            .service(fs::Files::new("/docs", docs_dir.clone()).prefer_utf8(true))
            .service(
                fs::Files::new("/", static_dir.clone())
                    .index_file("index.html")
                    .prefer_utf8(true),
            )
    })
    .keep_alive(Duration::from_secs(5))
    .client_request_timeout(Duration::from_secs(120))
    .workers(std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4))
    .bind(bind)?
    .run();

    log::info!(
        "Telegram Drive API server listening on http://localhost:{} (bind {}:{})",
        port,
        log_host,
        port
    );
    Ok(server)
}

pub async fn bootstrap_telegram(
    config: &ServerConfig,
    runtime: &ServerRuntime,
) -> Result<(), String> {
    let mode = runtime.transport.effective_mode(config).await;
    match mode {
        crate::telegram_transport::TelegramTransportMode::Bot => {
            let username = crate::telegram_transport::bot_test_connection(config).await?;
            log::info!("Bot transport ready (@{username}) — channel {}", config.storage_channel_id.as_deref().unwrap_or("?"));
            Ok(())
        }
        crate::telegram_transport::TelegramTransportMode::User => {
            *runtime.tg_state.api_id.lock().await = Some(config.telegram_api_id);
            let session_path = config.data_dir.join("telegram.session");
            if !session_path.exists() {
                log::warn!(
                    "No telegram.session yet — complete login via Web UI /api/v1/auth/* or mount a session file into {}",
                    config.data_dir.display()
                );
                return Ok(());
            }
            crate::commands::auth::ensure_client_initialized_at(
                &config.data_dir,
                &runtime.net_config,
                &runtime.tg_state,
                config.telegram_api_id,
            )
            .await?;
            log::info!("User/application transport ready (grammers session loaded)");
            Ok(())
        }
    }
}
