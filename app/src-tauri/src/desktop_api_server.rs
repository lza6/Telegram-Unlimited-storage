//! Optional desktop REST API (Settings → API) with full route + state wiring.

#[cfg(not(feature = "headless-server"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(feature = "headless-server"))]
use std::sync::Arc;

#[cfg(not(feature = "headless-server"))]
use actix_files as fs;
use actix_web::{web, App, HttpServer};

#[cfg(not(feature = "headless-server"))]
use crate::api_routes::{self, ApiState};
#[cfg(not(feature = "headless-server"))]
use crate::commands::api_settings::ApiSettingsFile;
#[cfg(not(feature = "headless-server"))]
use crate::commands::TelegramState;
#[cfg(not(feature = "headless-server"))]
use crate::db::DbConnection;
#[cfg(not(feature = "headless-server"))]
use crate::server_config::ServerConfig;
#[cfg(not(feature = "headless-server"))]
use crate::vpn_optimizer::NetworkConfig;

#[cfg(not(feature = "headless-server"))]
pub fn start_desktop_api_server(
    api_settings: ApiSettingsFile,
    tg_state: Arc<TelegramState>,
    db: DbConnection,
    net_config: Arc<NetworkConfig>,
    data_dir: std::path::PathBuf,
    resource_dir: Option<std::path::PathBuf>,
    handle_slot: Arc<std::sync::Mutex<Option<actix_web::dev::ServerHandle>>>,
    running_flag: Arc<AtomicBool>,
) {
    let port = api_settings.port;
    let key_hash = api_settings.key_hash.clone();

    std::thread::spawn(move || {
        let sys = actix_rt::System::new();
        sys.block_on(async move {
            let config: Arc<ServerConfig> = crate::server_config::for_desktop_api(
                data_dir.clone(),
                port,
                key_hash,
                crate::STREAM_PORT,
                resource_dir.clone(),
            );
            let transport = Arc::new(crate::telegram_transport::TransportHandle::new(
                &data_dir,
                config.default_transport_mode,
            ));
            let upload_gate = Arc::new(crate::upload_gate::build_upload_gate(&config));

            let tg_data = web::Data::new(tg_state);
            let db_data = web::Data::new(db);
            let api_state = web::Data::new(ApiState {
                key_hash: config.api_key_hash.clone(),
                max_upload_size_mb: config.max_upload_size_mb,
            });
            let auth_state = web::Data::new(crate::auth_routes::AuthRouteState {
                config: config.clone(),
            });
            let transport_data = web::Data::new(transport);
            let upload_gate_data = web::Data::new(upload_gate);
            let net_data = web::Data::new(net_config);
            let share_state = web::Data::new(crate::share_api_routes::ShareApiState {
                config: config.clone(),
                use_stream_port_for_shares: true,
            });
            let settings_state = web::Data::new(crate::settings_routes::SettingsRouteState {
                config: config.clone(),
                use_stream_port_for_shares: true,
            });

            log::info!("Starting desktop REST API on http://127.0.0.1:{port}");

            let static_dir = config.static_dir.clone();
            let serve_static = crate::server_config::desktop_static_servable(&static_dir);
            if serve_static {
                log::info!(
                    "Desktop REST also serving static Web from {}",
                    static_dir.display()
                );
            } else {
                log::info!(
                    "Desktop REST static Web not found (no telegram.html under {}); User login via Headless :1334 or in-app Auth",
                    static_dir.display()
                );
            }

            match HttpServer::new(move || {
                let cors = actix_cors::Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header();

                let mut app = App::new()
                    .wrap(cors)
                    .app_data(tg_data.clone())
                    .app_data(db_data.clone())
                    .app_data(api_state.clone())
                    .app_data(auth_state.clone())
                    .app_data(transport_data.clone())
                    .app_data(upload_gate_data.clone())
                    .app_data(net_data.clone())
                    .app_data(share_state.clone())
                    .app_data(settings_state.clone())
                    .configure(api_routes::configure_api)
                    .configure(crate::auth_routes::configure_auth)
                    .configure(crate::share_api_routes::configure_share_api)
                    .configure(crate::settings_routes::configure_settings_routes);

                if serve_static {
                    app = app.service(
                        fs::Files::new("/", static_dir.clone())
                            .index_file("index.html")
                            .prefer_utf8(true),
                    );
                }

                app
            })
            .bind(("127.0.0.1", port))
            {
                Ok(bound) => {
                    let server = bound.run();
                    *handle_slot.lock().unwrap() = Some(server.handle());
                    running_flag.store(true, Ordering::Relaxed);
                    server.await.ok();
                    running_flag.store(false, Ordering::Relaxed);
                }
                Err(e) => {
                    running_flag.store(false, Ordering::Relaxed);
                    log::error!("Failed to start desktop API server on port {port}: {e}");
                }
            }
        });
    });
}
