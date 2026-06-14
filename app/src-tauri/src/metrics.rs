//! Prometheus-style metrics (lightweight, no external crate).

use actix_web::{get, web, HttpResponse, Responder};

use crate::bot_pool::BotPool;
use crate::server_config::ServerConfig;
use crate::upload_gate::UploadGate;
use std::sync::Arc;

#[get("/metrics")]
async fn metrics_handler(
    config: web::Data<Arc<ServerConfig>>,
    upload_gate: web::Data<Arc<UploadGate>>,
    bot_pool: web::Data<Arc<BotPool>>,
) -> impl Responder {
    if !config.metrics_enabled {
        return HttpResponse::NotFound().finish();
    }
    let q = upload_gate.status();
    let uptime = crate::server_uptime::uptime_secs();
    let pool_m = bot_pool.metrics();
    let earliest = bot_pool.earliest_availability_secs().unwrap_or(0);
    let body = format!(
        "# HELP telegram_drive_uptime_seconds Process uptime\n\
         # TYPE telegram_drive_uptime_seconds gauge\n\
         telegram_drive_uptime_seconds {uptime}\n\
         # HELP telegram_drive_upload_chunk_slots_available UploadGate chunk slots\n\
         # TYPE telegram_drive_upload_chunk_slots_available gauge\n\
         telegram_drive_upload_chunk_slots_available {}\n\
         # HELP telegram_drive_upload_file_slots_available UploadGate file slots\n\
         # TYPE telegram_drive_upload_file_slots_available gauge\n\
         telegram_drive_upload_file_slots_available {}\n\
         # HELP telegram_drive_metadata_cache_enabled Metadata cache flag\n\
         # TYPE telegram_drive_metadata_cache_enabled gauge\n\
         telegram_drive_metadata_cache_enabled {}\n\
         # HELP telegram_drive_bot_pool_total Total number of bots\n\
         # TYPE telegram_drive_bot_pool_total gauge\n\
         telegram_drive_bot_pool_total {}\n\
         # HELP telegram_drive_bot_pool_available Available bots (not flooded)\n\
         # TYPE telegram_drive_bot_pool_available gauge\n\
         telegram_drive_bot_pool_available {}\n\
         # HELP telegram_drive_bot_pool_flooded Bots currently in FloodWait\n\
         # TYPE telegram_drive_bot_pool_flooded gauge\n\
         telegram_drive_bot_pool_flooded {}\n\
         # HELP telegram_drive_bot_pool_flood_events Total FloodWait events\n\
         # TYPE telegram_drive_bot_pool_flood_events counter\n\
         telegram_drive_bot_pool_flood_events {}\n\
         # HELP telegram_drive_bot_pool_earliest_availability_seconds Earliest bot availability\n\
         # TYPE telegram_drive_bot_pool_earliest_availability_seconds gauge\n\
         telegram_drive_bot_pool_earliest_availability_seconds {}\n",
        q.chunk_slots_available,
        q.file_slots_available,
        if config.metadata_cache_enabled { 1 } else { 0 },
        pool_m.total_bots,
        pool_m.available_bots,
        pool_m.flooded_bots,
        pool_m.total_flood_events,
        earliest,
    );
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body)
}

pub fn configure_metrics(cfg: &mut web::ServiceConfig) {
    cfg.service(metrics_handler);
}
