use actix_web::{test, web, App};
use app_lib::api_routes::{self, ApiState};
use app_lib::auth_routes::AuthRouteState;
use app_lib::bot_pool::BotPool;
use app_lib::commands::TelegramState;
use app_lib::server_config::test_config;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

fn empty_tg_state() -> Arc<TelegramState> {
    Arc::new(TelegramState {
        client: Arc::new(Mutex::new(None)),
        login_token: Arc::new(Mutex::new(None)),
        password_token: Arc::new(Mutex::new(None)),
        api_id: Arc::new(Mutex::new(None)),
        runner_shutdown: Arc::new(std::sync::Mutex::new(None)),
        runner_count: Arc::new(AtomicU32::new(0)),
        peer_cache: Arc::new(RwLock::new(HashMap::new())),
        cancelled_transfers: Arc::new(RwLock::new(HashSet::new())),
        bot_pool: Arc::new(BotPool::new(vec![])),
        user_probe_cache: Arc::new(app_lib::commands::UserProbeCache::default()),
    })
}

#[actix_rt::test]
async fn health_endpoints_distinguish_liveness_and_readiness() {
    app_lib::server_uptime::mark_started();
    let tg = empty_tg_state();
    let upload_gate = Arc::new(app_lib::upload_gate::UploadGate::new(4, 2));
    let auth = web::Data::new(AuthRouteState {
        config: test_config(),
    });
    let transport_dir =
        std::env::temp_dir().join(format!("td-health-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&transport_dir).expect("create transport test directory");
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(tg))
            .app_data(web::Data::new(ApiState {
                key_hash: None,
                max_upload_size_mb: 100,
            }))
            .app_data(auth)
            .app_data(web::Data::new(Arc::new(
                app_lib::telegram_transport::TransportHandle::new(
                    &transport_dir,
                    app_lib::telegram_transport::TelegramTransportMode::User,
                ),
            )))
            .app_data(web::Data::new(upload_gate))
            .configure(api_routes::configure_api),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/v1/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["telegram_connected"], body["ready"]);
    assert!(body["uptime_secs"].as_u64().is_some());
    assert!(body["build"].as_str().unwrap_or("").contains('.'));
    assert!(body.get("upload_queue").is_some());
    assert!(body["upload_queue"]["chunk_slots_total"].as_u64().is_some());

    let req = test::TestRequest::get().uri("/health/live").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "alive");

    let req = test::TestRequest::get().uri("/health/ready").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 503);
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "not_ready");
    assert_eq!(body["ready"], false);
}
