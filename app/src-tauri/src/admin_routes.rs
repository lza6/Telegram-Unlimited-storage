use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use futures_util::StreamExt;
use serde::Serialize;
use std::sync::Arc;

use crate::commands::utils::TempFileGuard;
use crate::commands::TelegramState;
use crate::server_config::ServerConfig;

#[derive(Clone)]
pub struct AdminState {
    pub config: Arc<ServerConfig>,
    pub db_pool: crate::db::DbConnection,
    pub access_lockout: Arc<crate::access_lockout::AccessLockout>,
}

#[derive(Serialize)]
pub struct LegacyUploadResult {
    pub filename: String,
    pub file_id: String,
    pub download_url: String,
}

#[derive(Serialize)]
struct ConfigResponse {
    chunk_size_mb: u32,
    chunk_concurrent: u32,
    files_concurrent: u32,
    download_threads: u32,
    stream_port: u16,
    api_version: String,
    transport_mode: String,
    bot_configured: bool,
    user_configured: bool,
    upload_queue: crate::upload_gate::UploadQueueStatus,
    metadata_cache_enabled: bool,
    metadata_cache_ttl_secs: u64,
    public_file_id_download: bool,
    upload_share_ttl_hours: i64,
}

pub fn check_access_pwd(req: &HttpRequest, config: &ServerConfig) -> bool {
    if let Some(h) = req.headers().get("X-Access-Pwd") {
        if let Ok(v) = h.to_str() {
            return crate::http_middleware::constant_time_eq(v, &config.access_pwd);
        }
    }
    false
}

pub fn check_pwd_form(pwd: &str, config: &ServerConfig) -> bool {
    // Trim only the stored password, not the input, to prevent timing attacks
    // on password format (e.g., whether it has trailing whitespace)
    crate::http_middleware::constant_time_eq(pwd, config.access_pwd.trim())
}

/// Web admin password or valid API key (hashed).
pub fn require_admin_or_api_key(req: &HttpRequest, config: &ServerConfig) -> Option<HttpResponse> {
    if check_access_pwd(req, config) {
        return None;
    }
    if let Some(key) = req.headers().get("X-API-Key").and_then(|v| v.to_str().ok()) {
        if let Some(ref hash) = config.api_key_hash {
            if crate::commands::api_settings::verify_key(key, hash) {
                return None;
            }
        }
    }
    Some(HttpResponse::Unauthorized().json(serde_json::json!({
        "error": {
            "code": "UNAUTHORIZED",
            "message": "Missing or invalid X-Access-Pwd or X-API-Key"
        }
    })))
}

pub fn host_base(req: &HttpRequest, config: &ServerConfig) -> String {
    crate::ui_settings::effective_base_url(req, config)
}

/// Public legacy download (no API key). `merged=true` for tg-disk manifest (`fileAll.txt`).
pub fn legacy_download_url(base: &str, message_id: i32, filename: &str, merged: bool) -> String {
    if merged {
        format!("{base}/d?file_id={message_id}")
    } else {
        format!(
            "{base}/d?file_id={message_id}&filename={}",
            urlencoding::encode(filename)
        )
    }
}

/// REST download (requires `X-API-Key`).
pub fn api_download_url(base: &str, message_id: i32, folder_id: Option<i64>) -> String {
    match folder_id {
        Some(fid) => format!("{base}/api/v1/files/{message_id}/download?folder_id={fid}"),
        None => format!("{base}/api/v1/files/{message_id}/download"),
    }
}

fn verify_client_key(req: &HttpRequest) -> String {
    let conn = req.connection_info();
    let forwarded = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok());
    crate::access_lockout::client_key_from_request(
        conn.host(),
        conn.realip_remote_addr(),
        forwarded,
    )
}

#[post("/verify")]
async fn verify(
    req: HttpRequest,
    payload: web::Payload,
    admin: web::Data<AdminState>,
) -> impl Responder {
    let client_key = verify_client_key(&req);
    if let Some(secs) = admin.access_lockout.lockout_remaining_secs(&client_key) {
        return HttpResponse::TooManyRequests()
            .insert_header(("Retry-After", secs.to_string()))
            .body(format!("登录尝试过多，请 {secs} 秒后重试"));
    }

    let fields = match crate::legacy_form::parse_request_form(&req, payload).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let pwd = fields.get("pwd").map(|s| s.as_str()).unwrap_or("");
    if check_pwd_form(pwd, &admin.config) || check_access_pwd(&req, &admin.config) {
        admin.access_lockout.clear(&client_key);
        HttpResponse::Ok().body("ok")
    } else {
        admin.access_lockout.record_failure(&client_key);
        HttpResponse::Unauthorized().body("密码错误")
    }
}

#[get("/config")]
async fn get_config(
    admin: web::Data<AdminState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
) -> impl Responder {
    let c = &admin.config;
    let mode = transport.active_mode().await;
    HttpResponse::Ok().json(ConfigResponse {
        chunk_size_mb: c.chunk_size_mb,
        chunk_concurrent: c.chunk_concurrent,
        files_concurrent: c.files_concurrent,
        download_threads: c.download_threads,
        stream_port: c.stream_port,
        api_version: env!("CARGO_PKG_VERSION").to_string(),
        transport_mode: mode.as_str().to_string(),
        bot_configured: crate::telegram_transport::TransportHandle::bot_configured(c),
        user_configured: crate::telegram_transport::TransportHandle::user_configured(c),
        upload_queue: upload_gate.status(),
        metadata_cache_enabled: c.metadata_cache_enabled,
        metadata_cache_ttl_secs: c.metadata_cache_ttl_secs,
        public_file_id_download: c.public_file_id_download,
        upload_share_ttl_hours: c.upload_share_ttl_hours,
    })
}

#[post("/upload")]
async fn legacy_upload(
    req: HttpRequest,
    mut payload: Multipart,
    admin: web::Data<AdminState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
) -> impl Responder {
    let _file_slot = match upload_gate.try_acquire_file() {
        Some(p) => p,
        None => return crate::upload_gate::response_upload_busy(3),
    };
    let mut pwd_ok = check_access_pwd(&req, &admin.config);
    let mut folder_id: Option<i64> = None;
    let mut temp_guard: Option<TempFileGuard> = None;
    let max_bytes = (admin.config.max_upload_size_mb as usize).saturating_mul(1024 * 1024);

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => return HttpResponse::BadRequest().body(format!("multipart error: {e}")),
        };
        let name = field.name().unwrap_or("").to_string();
        if name == "pwd" {
            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                if let Ok(b) = chunk {
                    bytes.extend_from_slice(&b);
                }
            }
            pwd_ok = check_pwd_form(String::from_utf8_lossy(&bytes).trim(), &admin.config);
            continue;
        }
        if name == "folder_id" {
            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                if let Ok(b) = chunk {
                    bytes.extend_from_slice(&b);
                }
            }
            if let Ok(id) = String::from_utf8_lossy(&bytes).trim().parse::<i64>() {
                folder_id = Some(id);
            }
            continue;
        }
        if name == "file" {
            let tmp = std::env::temp_dir().join(format!("td-upload-{}", uuid::Uuid::new_v4()));
            let mut f = match std::fs::File::create(&tmp) {
                Ok(file) => file,
                Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
            };
            let mut total_read = 0usize;
            while let Some(chunk) = field.next().await {
                match chunk {
                    Ok(b) => {
                        total_read += b.len();
                        if max_bytes > 0 && total_read > max_bytes {
                            let _ = std::fs::remove_file(&tmp);
                            return HttpResponse::PayloadTooLarge().body(format!(
                                "file exceeds {} MB limit",
                                admin.config.max_upload_size_mb
                            ));
                        }
                        if let Err(e) = std::io::Write::write_all(&mut f, &b) {
                            let _ = std::fs::remove_file(&tmp);
                            return HttpResponse::InternalServerError().body(e.to_string());
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&tmp);
                        return HttpResponse::BadRequest().body(e.to_string());
                    }
                }
            }
            temp_guard = Some(TempFileGuard::new(tmp));
        }
    }

    if !pwd_ok {
        return HttpResponse::Unauthorized().body("密码错误");
    }

    let guard = match temp_guard {
        Some(g) => g,
        None => return HttpResponse::BadRequest().body("missing file"),
    };

    if let Err(e) = crate::telegram_transport::ensure_transport_ready(
        &transport,
        &admin.config,
        &admin.config.data_dir,
        &tg_state,
        &net_config,
    )
    .await
    {
        return HttpResponse::ServiceUnavailable().body(format!("telegram not ready: {e}"));
    }

    let path_str = guard.path().to_string_lossy().to_string();
    let file_size = std::fs::metadata(&path_str)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    match crate::http_upload::upload_file_path(
        path_str,
        folder_id,
        &tg_state,
        &net_config,
        &admin.config,
        &admin.db_pool,
        &transport,
    )
    .await
    {
        Ok((message_id, saved_name)) => {
            guard.keep();
            let base = host_base(&req, &admin.config);
            let size = file_size;
            let owner_id = crate::tenant_auth::CallerIdentity::Admin.owner_id_for_asset();
            match crate::secure_download::issue_upload_download_link(
                &admin.db_pool,
                &admin.config,
                &base,
                folder_id,
                message_id,
                saved_name.clone(),
                size,
                &owner_id,
                false,
            ) {
                Ok(link) => HttpResponse::Ok().json(LegacyUploadResult {
                    filename: saved_name,
                    file_id: link.file_id,
                    download_url: link.download_url,
                }),
                Err(e) => HttpResponse::InternalServerError().body(e),
            }
        }
        Err(e) => HttpResponse::InternalServerError().body(e),
    }
}

pub fn configure_admin(cfg: &mut web::ServiceConfig) {
    cfg.service(verify)
        .service(get_config)
        .service(legacy_upload);
}
