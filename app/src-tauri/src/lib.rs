pub mod models;

pub mod bandwidth;
pub mod commands;
pub mod vpn_optimizer;

#[cfg(not(feature = "headless-server"))]
use crate::db::DbConnection;
#[cfg(not(feature = "headless-server"))]
use commands::streaming::StreamConfig;
use commands::TelegramState;
#[cfg(not(feature = "headless-server"))]
use rand::Rng;
#[cfg(not(feature = "headless-server"))]
use std::collections::{HashMap, HashSet};
#[cfg(not(feature = "headless-server"))]
use std::sync::Arc;
#[cfg(not(feature = "headless-server"))]
use tokio::sync::Mutex;

pub mod access_lockout;
pub mod admin_routes;
pub mod api_routes;
pub mod asset_locator;
pub mod auth_routes;
pub mod bot_pool;
pub mod db;
pub mod file_access;
pub mod http_download;
pub mod http_middleware;
pub mod http_upload;
pub mod legacy_form;
pub mod legacy_routes;
pub mod local_api;
pub mod logging;
pub mod metadata_cache;
pub mod metrics;
pub mod password_kdf;
pub mod postgres_control_plane;
pub mod postgres_download_accounting;
pub mod postgres_upload_saga;
pub mod presigned_url;
pub mod route_registry;
pub mod secure_download;
pub mod server;
pub mod server_config;
pub mod server_http;
pub mod server_maintenance;
pub mod server_uptime;
pub mod share_api_routes;
pub mod share_routes;
pub mod sharing_core;
pub mod telegram_error;
pub mod telegram_transport;
pub mod tenant_auth;
pub mod upload_gate;
pub mod webdav_routes;
use bot_pool::BotPool;
pub mod chunk_index;
#[cfg(not(feature = "headless-server"))]
pub mod desktop_api_server;
pub mod download_counter;
pub mod download_degradation;
pub mod durable_scheduler;
pub mod progress_distributed;
pub mod session_backup;
pub mod settings_routes;
pub mod storage_factory;
pub mod ui_settings;
pub mod upload_progress;
pub mod upload_saga_recovery;

#[cfg(not(feature = "headless-server"))]
use tauri::Manager;
/// Single source of truth for the Actix streaming server port.
/// Referenced in lib.rs (server startup) and exposed to the frontend
/// via cmd_get_stream_info so no component ever hardcodes the port.
pub const STREAM_PORT: u16 = 14201;

/// Generate a random 32-character hex token for streaming server auth
#[cfg(not(feature = "headless-server"))]
fn generate_stream_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Holds the Actix-web server stop handle so we can shut it down
/// from the RunEvent::Exit handler for graceful Ctrl+C termination.
#[cfg(not(feature = "headless-server"))]
pub struct ActixServerHandle(pub Arc<std::sync::Mutex<Option<actix_web::dev::ServerHandle>>>);

/// Tracks whether the API server is currently running (for the frontend status dot)
#[cfg(not(feature = "headless-server"))]
pub struct ApiServerRunning(pub Arc<std::sync::atomic::AtomicBool>);

/// Holds the API server stop handle separately so we can restart it independently
#[cfg(not(feature = "headless-server"))]
pub struct ApiServerHandle(pub Arc<std::sync::Mutex<Option<actix_web::dev::ServerHandle>>>);

/// Restart (or stop) the API server based on current settings.
/// Called from Tauri commands when the user changes API settings.
#[cfg(not(feature = "headless-server"))]
pub fn restart_api_server(app: &tauri::AppHandle) {
    let api_handle_arc = app.state::<ApiServerHandle>().0.clone();
    let old_handle = api_handle_arc.lock().ok().and_then(|mut g| g.take());
    if let Some(handle) = old_handle {
        log::info!("Stopping existing API server...");
        drop(handle.stop(true));
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let settings = commands::api_settings::prepare_settings_for_runtime(app);
    let running_flag = app.state::<ApiServerRunning>().0.clone();

    if !settings.enabled {
        running_flag.store(false, std::sync::atomic::Ordering::Relaxed);
        log::info!("API server disabled");
        return;
    }

    let tg_state = Arc::new(app.state::<TelegramState>().inner().clone());
    let db = app.state::<DbConnection>().inner().clone();
    let net_config = app
        .state::<Arc<vpn_optimizer::NetworkConfig>>()
        .inner()
        .clone();
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("telegram-drive-data"));
    let resource_dir = app.path().resource_dir().ok();

    crate::desktop_api_server::start_desktop_api_server(
        settings,
        tg_state,
        db,
        net_config,
        data_dir,
        resource_dir,
        api_handle_arc,
        running_flag,
    );
}

#[cfg(not(feature = "headless-server"))]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    crate::server_uptime::mark_started();

    let stream_token = generate_stream_token();

    // Shared handle for stopping the Actix streaming server during shutdown
    let server_handle: Arc<std::sync::Mutex<Option<actix_web::dev::ServerHandle>>> =
        Arc::new(std::sync::Mutex::new(None));
    let server_handle_for_setup = server_handle.clone();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());
    }

    let app = builder
        .setup(move |app| {
            let initial_bot_pool = server_config::ServerConfig::from_env()
                .map(|cfg| cfg.all_bot_tokens())
                .unwrap_or_default();
            let bot_pool = Arc::new(BotPool::new(initial_bot_pool));

            app.manage(TelegramState {
                client: Arc::new(Mutex::new(None)),
                login_token: Arc::new(Mutex::new(None)),
                password_token: Arc::new(Mutex::new(None)),
                api_id: Arc::new(Mutex::new(None)),
                runner_shutdown: Arc::new(std::sync::Mutex::new(None)),
                runner_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
                peer_cache: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
                cancelled_transfers: Arc::new(tokio::sync::RwLock::new(HashSet::new())),
                bot_pool: bot_pool.clone(),
                user_probe_cache: Arc::new(commands::UserProbeCache::default()),
            });
            app.manage(bandwidth::BandwidthManager::new(app.handle()));
            app.manage(StreamConfig {
                token: stream_token.clone(),
                port: STREAM_PORT,
            });
            app.manage(ActixServerHandle(server_handle_for_setup.clone()));
            app.manage(ApiServerHandle(Arc::new(std::sync::Mutex::new(None))));
            app.manage(ApiServerRunning(Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            )));
            let loaded_config = vpn_optimizer::load_network_config(app.handle());
            let net_config = Arc::new(vpn_optimizer::NetworkConfig::new_with_config(loaded_config));
            app.manage(net_config.clone());

            // Initialize SQLite Database
            let db_pool = db::init_db(app.handle()).map_err(|e| {
                log::error!("Failed to initialize SQLite database: {}", e);
                e
            })?;
            app.manage(db_pool.clone());

            if let Ok(cfg) = server_config::ServerConfig::from_env() {
                if let Err(e) = tenant_auth::bootstrap_tenants(&db_pool, &cfg) {
                    log::warn!("tenant bootstrap: {e}");
                }
            }

            // Clean up expired shares on startup
            if let Err(e) = db::cleanup_expired_shares(&db_pool) {
                log::warn!("Failed to clean up expired shares: {}", e);
            }
            // Clean up stale upload sessions on startup
            if let Err(e) = db::cleanup_stale_uploads(&db_pool) {
                log::warn!("Failed to clean up stale uploads: {}", e);
            }

            // Start periodic session backup for User mode resilience
            {
                let backup_data_dir = app
                    .path()
                    .app_data_dir()
                    .unwrap_or_else(|_| std::env::temp_dir().join("telegram-drive-data"));
                let backup_running = Arc::new(std::sync::atomic::AtomicBool::new(true));
                crate::session_backup::spawn_periodic_backup(backup_data_dir, backup_running);
            }

            // Start Streaming Server on dedicated thread (Actix needs its own runtime)
            let state = Arc::new(app.state::<TelegramState>().inner().clone());
            let token_for_server = stream_token.clone();
            let handle_for_thread = server_handle_for_setup.clone();
            let net_config_for_stream = net_config.clone();
            let db_pool_for_server = db_pool.clone();
            let desktop_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("telegram-drive-data"));
            let stream_data_dir = std::env::var("DATA_DIR")
                .ok()
                .map(std::path::PathBuf::from)
                .unwrap_or(desktop_data_dir);
            let keepalive_dc_addr = match server_config::ServerConfig::from_env() {
                Ok(c) => c.tg_dc_addr,
                Err(_) => server_config::for_desktop_api(
                    stream_data_dir.clone(),
                    STREAM_PORT,
                    None,
                    STREAM_PORT,
                    None,
                )
                .tg_dc_addr
                .clone(),
            };
            std::thread::spawn(move || {
                let sys = actix_rt::System::new();
                sys.block_on(async move {
                    let data_dir = stream_data_dir;
                    let stream_config = match server_config::ServerConfig::from_env() {
                        Ok(c) => {
                            let mut cfg = c.clone();
                            if std::env::var("DATA_DIR").is_err() {
                                cfg.data_dir = data_dir.clone();
                            }
                            Arc::new(cfg)
                        }
                        Err(_) => server_config::for_desktop_api(
                            data_dir.clone(),
                            STREAM_PORT,
                            None,
                            STREAM_PORT,
                            None,
                        ),
                    };
                    let transport = Arc::new(telegram_transport::TransportHandle::new(
                        &data_dir,
                        stream_config.default_transport_mode,
                    ));
                    let admin_state = admin_routes::AdminState {
                        config: stream_config.clone(),
                        db_pool: db_pool_for_server.clone(),
                        access_lockout: Arc::new(access_lockout::AccessLockout::new(8, 300)),
                    };
                    let local_bridge = crate::local_api::LocalApiBridge::from_data_dir(&data_dir);
                    match server::start_server(
                        state,
                        STREAM_PORT,
                        token_for_server,
                        db_pool_for_server,
                        net_config_for_stream,
                        admin_state,
                        transport,
                        local_bridge,
                    )
                    .await
                    {
                        Ok(server) => {
                            // Store the handle so RunEvent::Exit can stop it
                            *handle_for_thread.lock().unwrap() = Some(server.handle());
                            // Now await the server — blocks until stopped
                            server.await.ok();
                        }
                        Err(e) => log::error!("Streaming server failed: {}", e),
                    }
                });
            });

            // Start API server if enabled in settings
            restart_api_server(app.handle());

            // Start VPN keep-alive background task
            {
                let ka_config = net_config.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        let interval = ka_config.keep_alive_interval_sec();
                        if interval == 0 {
                            // Disabled — check again in 10s
                            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                            continue;
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;
                        // TCP ping to Telegram DC2 (best-effort)
                        let dc_addr: std::net::SocketAddr =
                            keepalive_dc_addr.parse().unwrap_or_else(|_| {
                                "149.154.167.50:443".parse().expect("default DC addr")
                            });
                        let _ = tauri::async_runtime::spawn_blocking(move || {
                            use std::net::TcpStream;
                            let _ = TcpStream::connect_timeout(
                                &dc_addr,
                                std::time::Duration::from_secs(5),
                            );
                        })
                        .await;
                    }
                });
            }

            // Auto-enable VPN optimizer when auto_detect_vpn is on and VPN interface is present
            {
                let detect_config = net_config.clone();
                let detect_app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let data_dir = detect_app
                        .path()
                        .app_data_dir()
                        .unwrap_or_else(|_| std::env::temp_dir());
                    crate::settings_routes::maybe_auto_enable_vpn_on_startup(
                        &detect_config,
                        &data_dir,
                    )
                    .await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::cmd_auth_request_code,
            commands::cmd_auth_sign_in,
            commands::cmd_auth_check_password,
            commands::cmd_get_files,
            commands::cmd_rebuild_file_index,
            commands::cmd_invalidate_file_index,
            commands::cmd_upload_file,
            commands::cmd_connect,
            commands::cmd_log,
            commands::cmd_delete_file,
            commands::cmd_download_file,
            commands::cmd_move_files,
            commands::cmd_create_folder,
            commands::cmd_delete_folder,
            commands::cmd_get_bandwidth,
            commands::cmd_get_preview,
            commands::cmd_logout,
            commands::cmd_scan_folders,
            commands::cmd_search_global,
            commands::cmd_check_connection,
            commands::cmd_reconnect_telegram,
            commands::cmd_is_network_available,
            commands::cmd_clean_cache,
            commands::cmd_get_thumbnail,
            commands::cmd_get_stream_info,
            commands::cmd_cancel_transfer,
            commands::cmd_auth_qr_login,
            commands::cmd_auth_qr_poll,
            commands::cmd_get_api_settings,
            commands::cmd_get_api_health,
            commands::cmd_update_api_settings,
            commands::cmd_regenerate_api_key,
            commands::cmd_regenerate_local_access_pwd,
            commands::cmd_delete_image_thumbnail,
            commands::cmd_zip_folder,
            commands::cmd_delete_temp_zip,
            commands::cmd_apply_proxy_settings,
            commands::cmd_apply_vpn_settings,
            commands::cmd_get_network_config,
            commands::cmd_get_ui_share_domain,
            commands::cmd_set_ui_share_domain,
            commands::cmd_get_polling_interval_ms,
            commands::cmd_check_latency,
            commands::cmd_detect_vpn,
            commands::cmd_create_share,
            commands::cmd_list_shares,
            commands::cmd_revoke_share,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            log::info!("Application exiting — shutting down background services...");

            // 1. Shutdown the grammers network runner
            let shutdown_arc = app_handle.state::<TelegramState>().runner_shutdown.clone();
            if crate::commands::signal_runner_shutdown(&shutdown_arc) {
                log::info!("Signaling network runner shutdown...");
            }

            // 2. Stop the Actix streaming server (graceful)
            let server_arc = app_handle.state::<ActixServerHandle>().0.clone();
            let server_handle = server_arc.lock().ok().and_then(|mut g| g.take());
            if let Some(handle) = server_handle {
                log::info!("Stopping Actix streaming server...");
                drop(handle.stop(true));
            }

            // 3. Stop the API server (graceful)
            let api_arc = app_handle.state::<ApiServerHandle>().0.clone();
            let api_handle = api_arc.lock().ok().and_then(|mut g| g.take());
            if let Some(handle) = api_handle {
                log::info!("Stopping API server...");
                drop(handle.stop(true));
            }
        }
    });
}
