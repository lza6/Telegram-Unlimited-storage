use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::admin_routes::{
    check_access_pwd, check_pwd_form, host_base, AdminState, LegacyUploadResult,
};
use crate::commands::TelegramState;
use crate::db;

#[derive(Serialize)]
struct ChunkResult {
    file_id: String,
    sha256: String,
}

#[derive(Serialize)]
struct UploadStatusResponse {
    session_id: String,
    filename: String,
    total_chunks: i32,
    uploaded_chunks: i32,
    status: String,
    chunks: Vec<ChunkStatusItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_url: Option<String>,
}

#[derive(Serialize)]
struct ChunkStatusItem {
    chunk_index: i32,
    status: String,
    sha256: Option<String>,
}

async fn ensure_tg(
    admin: &AdminState,
    tg_state: &TelegramState,
    net_config: &Arc<crate::vpn_optimizer::NetworkConfig>,
    transport: &crate::telegram_transport::TransportHandle,
) -> Result<(), HttpResponse> {
    crate::telegram_transport::ensure_transport_ready(
        transport,
        &admin.config,
        &admin.config.data_dir,
        tg_state,
        net_config,
    )
    .await
    .map(|_| ())
    .map_err(|e| HttpResponse::ServiceUnavailable().body(e))
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[post("/upload_chunk")]
async fn upload_chunk(
    req: HttpRequest,
    mut payload: Multipart,
    admin: web::Data<AdminState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
    progress_hub: web::Data<Arc<crate::upload_progress::UploadProgressHub>>,
) -> impl Responder {
    let _chunk_slot = match upload_gate.try_acquire_chunk() {
        Some(p) => p,
        None => return crate::upload_gate::response_upload_busy(2),
    };
    let mut pwd_ok = check_access_pwd(&req, &admin.config);
    let mut chunk_data: Option<Vec<u8>> = None;
    let mut chunk_index = String::new();
    let mut total_chunks = String::new();
    let mut filename = String::new();
    let mut session_id = String::new();
    let max_chunk_bytes = admin.config.max_chunk_bytes();

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => return HttpResponse::BadRequest().body(format!("multipart error: {e}")),
        };
        let name = field.name().unwrap_or("").to_string();
        let mut bytes = Vec::new();
        while let Some(chunk) = field.next().await {
            if let Ok(b) = chunk {
                bytes.extend_from_slice(&b);
                if bytes.len() > max_chunk_bytes {
                    return HttpResponse::PayloadTooLarge().body(format!(
                        "chunk exceeds {} MB limit",
                        admin.config.chunk_size_mb
                    ));
                }
            }
        }
        match name.as_str() {
            "pwd" => pwd_ok = check_pwd_form(String::from_utf8_lossy(&bytes).trim(), &admin.config),
            "chunk_index" => chunk_index = String::from_utf8_lossy(&bytes).trim().to_string(),
            "total_chunks" => total_chunks = String::from_utf8_lossy(&bytes).trim().to_string(),
            "filename" => filename = String::from_utf8_lossy(&bytes).trim().to_string(),
            "session_id" => session_id = String::from_utf8_lossy(&bytes).trim().to_string(),
            "chunk" => chunk_data = Some(bytes),
            _ => {}
        }
    }

    if !pwd_ok {
        return HttpResponse::Unauthorized().body("密码错误");
    }
    let data = match chunk_data {
        Some(d) if !d.is_empty() => d,
        _ => return HttpResponse::BadRequest().body("missing chunk"),
    };

    let idx: i32 = match chunk_index.parse() {
        Ok(v) => v,
        Err(_) => return HttpResponse::BadRequest().body("invalid chunk_index"),
    };
    let total: i32 = match total_chunks.parse() {
        Ok(v) => v,
        Err(_) => return HttpResponse::BadRequest().body("invalid total_chunks"),
    };
    if idx < 0 || total <= 0 || idx >= total || total > 10000 {
        return HttpResponse::BadRequest().body("invalid chunk parameters");
    }
    if session_id.is_empty() {
        return HttpResponse::BadRequest().body("missing session_id");
    }
    if filename.is_empty() {
        return HttpResponse::BadRequest().body("missing filename");
    }

    if let Err(e) = ensure_tg(&admin, &tg_state, &net_config, &transport).await {
        return e;
    }

    // Compute SHA256 before uploading
    let sha256_hash = compute_sha256(&data);

    // Ensure upload session exists in DB (idempotent)
    if let Err(e) = db::create_upload_session(&admin.db_pool, &session_id, &filename, total) {
        log::error!("Failed to create upload session: {}", e);
        return HttpResponse::InternalServerError().body("failed to create upload session");
    }

    let caption = format!("blob [{}/{}] - {}", idx, total, filename);
    match crate::http_upload::upload_bytes_with_caption(
        data,
        "blob",
        &caption,
        None,
        &tg_state,
        &net_config,
        &admin.config,
        &admin.db_pool,
        &transport,
    )
    .await
    {
        Ok(message_id) => {
            let file_id = message_id.to_string();
            if let Err(e) =
                db::record_upload_chunk(&admin.db_pool, &session_id, idx, &file_id, &sha256_hash)
            {
                log::error!("Failed to record upload chunk: {}", e);
                return HttpResponse::InternalServerError().body("failed to record chunk");
            }
            crate::upload_progress::emit_chunk_progress(
                &progress_hub,
                &admin.db_pool,
                &session_id,
                &filename,
            )
            .await;
            HttpResponse::Ok().json(ChunkResult {
                file_id,
                sha256: sha256_hash,
            })
        }
        Err(e) => {
            log::error!("Internal error: {}", e);
            HttpResponse::InternalServerError().body("internal error")
        }
    }
}

#[get("/upload_status")]
async fn get_upload_status(req: HttpRequest, admin: web::Data<AdminState>) -> impl Responder {
    if let Some(resp) =
        crate::upload_progress::verify_upload_progress_request(&req, &admin.config.access_pwd)
    {
        return resp;
    }

    let session_id = req
        .query_string()
        .split('&')
        .find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next()?;
            let v = kv.next()?;
            if k == "session_id" {
                Some(urlencoding::decode(v).unwrap_or_default().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    if session_id.is_empty() {
        return HttpResponse::BadRequest().body("missing session_id");
    }

    let summary = match db::get_upload_session_summary(&admin.db_pool, &session_id) {
        Ok(Some(s)) => s,
        Ok(None) => return HttpResponse::NotFound().body("session not found"),
        Err(e) => return HttpResponse::InternalServerError().body(e),
    };

    let (total, status, filename) = summary;
    let chunks = match db::get_upload_session_chunks(&admin.db_pool, &session_id) {
        Ok(c) => c,
        Err(e) => return HttpResponse::InternalServerError().body(e),
    };

    let uploaded_count = chunks.iter().filter(|c| c.status == "uploaded").count() as i32;

    let mut file_id = None;
    let mut download_url = None;
    if status == "completed" {
        if let Ok(Some(manifest_id)) =
            db::get_upload_session_manifest_file_id(&admin.db_pool, &session_id)
        {
            if let Ok(mid) = manifest_id.parse::<i32>() {
                let base = host_base(&req, &admin.config);
                let owner_id = crate::tenant_auth::CallerIdentity::Admin.owner_id_for_asset();
                if let Ok(link) = crate::secure_download::issue_upload_download_link(
                    &admin.db_pool,
                    &admin.config,
                    &base,
                    None,
                    mid,
                    filename.clone(),
                    0,
                    &owner_id,
                    true,
                ) {
                    download_url = Some(link.download_url);
                    file_id = Some(link.file_id);
                }
            }
        }
    }

    HttpResponse::Ok().json(UploadStatusResponse {
        session_id,
        filename,
        total_chunks: total,
        uploaded_chunks: uploaded_count,
        status,
        chunks: chunks
            .into_iter()
            .map(|c| ChunkStatusItem {
                chunk_index: c.chunk_index,
                status: c.status,
                sha256: c.sha256,
            })
            .collect(),
        file_id,
        download_url,
    })
}

#[post("/merge_chunks")]
async fn merge_chunks(
    req: HttpRequest,
    payload: web::Payload,
    admin: web::Data<AdminState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
    progress_hub: web::Data<Arc<crate::upload_progress::UploadProgressHub>>,
) -> impl Responder {
    let _chunk_slot = match upload_gate.try_acquire_chunk() {
        Some(p) => p,
        None => return crate::upload_gate::response_upload_busy(2),
    };
    let body = match crate::legacy_form::parse_request_form(&req, payload).await {
        Ok(f) => f,
        Err(r) => return r,
    };
    let pwd = body.get("pwd").map(|s| s.as_str()).unwrap_or("");
    if !check_pwd_form(pwd, &admin.config) && !check_access_pwd(&req, &admin.config) {
        return HttpResponse::Unauthorized().body("密码错误");
    }

    let filename = match body.get("filename") {
        Some(f) if !f.is_empty() => f.clone(),
        _ => return HttpResponse::BadRequest().body("missing filename"),
    };

    let session_id = body
        .get("session_id")
        .map(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let folder_id = crate::legacy_form::parse_optional_i64_field(&body, "folder_id");

    let chunk_ids: Vec<String> = if let Some(j) = body.get("chunk_ids") {
        if j.is_empty() && session_id.is_empty() {
            return HttpResponse::BadRequest().body("missing chunk_ids or session_id");
        }
        if !j.is_empty() {
            match serde_json::from_str(j) {
                Ok(v) => v,
                Err(e) => {
                    return HttpResponse::BadRequest().body(format!("chunk_ids invalid: {e}"))
                }
            }
        } else {
            vec![]
        }
    } else {
        if session_id.is_empty() {
            return HttpResponse::BadRequest().body("missing chunk_ids or session_id");
        }
        vec![]
    };

    // If no chunk_ids provided but session_id is present, fetch from DB
    let final_chunk_ids: Vec<String> = if chunk_ids.is_empty() && !session_id.is_empty() {
        let db_chunks = match db::get_upload_session_chunks(&admin.db_pool, &session_id) {
            Ok(c) => c,
            Err(e) => return HttpResponse::InternalServerError().body(e),
        };
        let mut ids = Vec::with_capacity(db_chunks.len());
        for chunk in &db_chunks {
            if chunk.status != "uploaded" {
                return HttpResponse::BadRequest()
                    .body(format!("chunk {} not uploaded yet", chunk.chunk_index));
            }
            if let Some(ref fid) = chunk.file_id {
                ids.push(fid.clone());
            } else {
                return HttpResponse::BadRequest()
                    .body(format!("chunk {} missing file_id", chunk.chunk_index));
            }
        }
        if ids.is_empty() {
            return HttpResponse::BadRequest().body("no chunks found for session");
        }
        ids
    } else {
        if chunk_ids.is_empty() {
            return HttpResponse::BadRequest().body("chunk_ids empty");
        }
        chunk_ids
    };

    if let Err(e) = ensure_tg(&admin, &tg_state, &net_config, &transport).await {
        return e;
    }

    let mut manifest = filename.clone();
    manifest.push('\n');
    for id in &final_chunk_ids {
        manifest.push_str(id);
        manifest.push('\n');
    }

    match crate::http_upload::upload_text_file(
        &manifest,
        "fileAll.txt",
        &filename,
        folder_id,
        &tg_state,
        &net_config,
        &admin.config,
        &admin.db_pool,
        &transport,
    )
    .await
    {
        Ok(manifest_id) => {
            // Mark session as completed if session_id was used
            if !session_id.is_empty() {
                let manifest_file_id = manifest_id.to_string();
                if let Err(e) =
                    db::complete_upload_session(&admin.db_pool, &session_id, &manifest_file_id)
                {
                    log::warn!("Failed to complete upload session {}: {}", session_id, e);
                }
                progress_hub
                    .emit(crate::upload_progress::UploadProgressEvent {
                        session_id: session_id.clone(),
                        filename: filename.clone(),
                        uploaded_chunks: final_chunk_ids.len() as i32,
                        total_chunks: final_chunk_ids.len() as i32,
                        status: "completed".into(),
                    })
                    .await;
            }
            let base = host_base(&req, &admin.config);
            let owner_id =
                crate::tenant_auth::check_pwd_caller(pwd, &admin.config).owner_id_for_asset();
            match crate::secure_download::issue_upload_download_link(
                &admin.db_pool,
                &admin.config,
                &base,
                folder_id,
                manifest_id,
                filename.clone(),
                0,
                &owner_id,
                true,
            ) {
                Ok(link) => HttpResponse::Ok().json(LegacyUploadResult {
                    filename: filename.clone(),
                    file_id: link.file_id,
                    download_url: link.download_url,
                }),
                Err(e) => HttpResponse::InternalServerError().body(e),
            }
        }
        Err(e) => {
            log::error!("Internal error: {}", e);
            HttpResponse::InternalServerError().body("internal error")
        }
    }
}

#[derive(serde::Deserialize)]
struct PresignedDownloadQuery {
    file_id: i32,
    #[serde(default)]
    folder_id: Option<i64>,
    exp: i64,
    owner: String,
    sig: String,
}

#[get("/d/signed")]
async fn presigned_download_query(
    req: HttpRequest,
    query: web::Query<PresignedDownloadQuery>,
    tg_state: web::Data<Arc<TelegramState>>,
    admin: web::Data<AdminState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    let secret = match admin.config.download_signing_secret.as_deref() {
        Some(s) => s,
        None => {
            return HttpResponse::ServiceUnavailable().json(serde_json::json!({
                "error": { "code": "PRESIGN_DISABLED", "message": "DOWNLOAD_SIGNING_SECRET is not configured" }
            }));
        }
    };

    let params = crate::presigned_url::parse_query(
        query.file_id,
        query.folder_id,
        query.exp,
        &query.owner,
        &query.sig,
    );

    if crate::presigned_url::is_expired(params.expires_at) {
        return HttpResponse::Gone().json(serde_json::json!({
            "error": { "code": "LINK_EXPIRED", "message": "Presigned URL has expired" }
        }));
    }

    if !crate::presigned_url::verify(&params, secret, &query.sig) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": { "code": "INVALID_SIGNATURE", "message": "Invalid or tampered download signature" }
        }));
    }

    if let Err(msg) = crate::file_access::assert_presigned_asset(
        &admin.db_pool,
        params.message_id,
        params.folder_id,
        &params.owner_id,
    ) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": { "code": "FORBIDDEN", "message": msg }
        }));
    }

    match crate::http_download::download_message_stream(
        &req,
        params.message_id,
        params.folder_id,
        &tg_state,
        false,
        &admin.config,
        &admin.db_pool,
        &transport,
        &net_config,
    )
    .await
    {
        Ok(r) => r,
        Err(r) => r,
    }
}

#[derive(serde::Deserialize)]
struct LegacyDownloadQuery {
    file_id: String,
    filename: Option<String>,
    folder_id: Option<i64>,
}

#[get("/d")]
async fn legacy_download_query(
    req: HttpRequest,
    query: web::Query<LegacyDownloadQuery>,
    tg_state: web::Data<Arc<TelegramState>>,
    admin: web::Data<AdminState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    if !crate::secure_download::raw_file_id_download_allowed(&req, &admin.config) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "error": {
                "code": "RAW_FILE_ID_DISABLED",
                "message": "Direct download by file_id is disabled. Use the share link returned after upload (/d/{token})."
            }
        }));
    }

    let message_id = match query.file_id.parse::<i32>() {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().body("invalid file_id"),
    };

    if admin.config.multi_tenant_enabled {
        let caller = if crate::admin_routes::check_access_pwd(&req, &admin.config) {
            crate::tenant_auth::CallerIdentity::Admin
        } else if let Some(tenant_id) =
            crate::tenant_auth::api_key_tenant(&req, &admin.db_pool, &admin.config)
        {
            crate::tenant_auth::CallerIdentity::Tenant { tenant_id }
        } else {
            crate::tenant_auth::CallerIdentity::Anonymous
        };
        if let Err(msg) =
            crate::file_access::assert_download_allowed(&admin.db_pool, message_id, &caller, true)
        {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "error": { "code": "FORBIDDEN", "message": msg }
            }));
        }
    }

    if let Some(ref name) = query.filename {
        if !name.is_empty() && name != "fileAll.txt" {
            return match crate::http_download::download_message_stream(
                &req,
                message_id,
                query.folder_id,
                &tg_state,
                false,
                &admin.config,
                &admin.db_pool,
                &transport,
                &net_config,
            )
            .await
            {
                Ok(r) => r,
                Err(r) => r,
            };
        }
    }

    match crate::http_download::download_manifest_stream(
        &req,
        message_id,
        query.folder_id,
        &tg_state,
        &admin.config,
        &admin.db_pool,
        &transport,
        &net_config,
    )
    .await
    {
        Ok(r) => r,
        Err(r) => r,
    }
}

pub fn configure_legacy(cfg: &mut web::ServiceConfig) {
    cfg.service(upload_chunk)
        .service(get_upload_status)
        .service(merge_chunks)
        .service(presigned_download_query)
        .service(legacy_download_query);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_sha256_hex_is_stable() {
        let h = compute_sha256(b"hello");
        assert_eq!(h.len(), 64);
        assert_eq!(compute_sha256(b"hello"), h);
    }
}
