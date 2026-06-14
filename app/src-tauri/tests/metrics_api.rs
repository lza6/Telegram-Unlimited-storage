use actix_web::{test, web, App};
use app_lib::metrics;
use app_lib::server_config::test_config;
use std::sync::Arc;

#[actix_rt::test]
async fn metrics_endpoint_exposes_upload_queue_gauges() {
    app_lib::server_uptime::mark_started();
    let config = test_config();
    let upload_gate = Arc::new(app_lib::upload_gate::UploadGate::new(4, 2));

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(config))
            .app_data(web::Data::new(upload_gate))
            .configure(metrics::configure_metrics),
    )
    .await;

    let req = test::TestRequest::get().uri("/metrics").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
    let body = test::read_body(resp).await;
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("telegram_drive_upload_chunk_slots_available"));
    assert!(text.contains("telegram_drive_uptime_seconds"));
}
