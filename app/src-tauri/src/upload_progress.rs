//! Upload session progress hub — SSE `/upload_events` + WebSocket `/upload_ws`.
//!
//! Supports distributed mode via Redis Pub/Sub (see `progress_distributed.rs`).

use std::sync::Arc;

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use actix_ws::Message;
use async_stream::stream;
use futures_util::StreamExt;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::sync::broadcast;

use crate::progress_distributed::{DistributedProgressHub, ProgressEvent};

type HmacSha256 = Hmac<Sha256>;

const CHANNEL_CAPACITY: usize = 64;
const PROGRESS_TOKEN_TTL_SECS: i64 = 300;
const PROGRESS_TOKEN_VERSION: &str = "v1";

/// Upload progress hub wrapper — uses DistributedProgressHub internally.
/// This provides backwards-compatible API while supporting Redis Pub/Sub.
pub struct UploadProgressHub {
    inner: DistributedProgressHub,
}

impl UploadProgressHub {
    pub fn new() -> Self {
        Self {
            inner: DistributedProgressHub::from_env(),
        }
    }

    /// Create memory-only hub (for desktop mode)
    pub fn memory_only() -> Self {
        Self {
            inner: DistributedProgressHub::memory(),
        }
    }

    pub async fn emit(&self, event: UploadProgressEvent) {
        let distributed_event = ProgressEvent {
            session_id: event.session_id,
            filename: event.filename,
            uploaded_chunks: event.uploaded_chunks,
            total_chunks: event.total_chunks,
            status: event.status,
            timestamp: chrono::Utc::now().timestamp(),
        };
        self.inner.emit(distributed_event).await;
    }

    pub async fn subscribe(&self, session_id: &str) -> broadcast::Receiver<ProgressEvent> {
        self.inner.subscribe(session_id).await
    }

    pub async fn remove_session(&self, session_id: &str) {
        self.inner.remove_session(session_id).await;
    }
}

impl Default for UploadProgressHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy event type for backwards compatibility with existing API consumers.
#[derive(Clone, Debug, Serialize)]
pub struct UploadProgressEvent {
    pub session_id: String,
    pub filename: String,
    pub uploaded_chunks: i32,
    pub total_chunks: i32,
    pub status: String,
}

fn parse_query_param(req: &HttpRequest, key: &str) -> String {
    req.query_string()
        .split('&')
        .find_map(|pair| {
            let mut kv = pair.splitn(2, '=');
            let k = kv.next()?;
            let v = kv.next()?;
            if k == key {
                Some(urlencoding::decode(v).unwrap_or_default().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn parse_session_id(req: &HttpRequest) -> String {
    parse_query_param(req, "session_id")
}

fn parse_progress_exp(req: &HttpRequest) -> Option<i64> {
    let raw = parse_query_param(req, "exp");
    if raw.is_empty() {
        return None;
    }
    raw.parse().ok()
}

fn parse_progress_token(req: &HttpRequest) -> String {
    parse_query_param(req, "token")
}

fn progress_hmac_secret(access_pwd: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"upload-progress-v1|");
    hasher.update(access_pwd.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn canonical_progress_payload(session_id: &str, expires_at: i64) -> String {
    format!("{PROGRESS_TOKEN_VERSION}|{session_id}|{expires_at}")
}

pub fn issue_upload_progress_token(
    session_id: &str,
    access_pwd: &str,
) -> Result<(String, i64), String> {
    if session_id.trim().is_empty() {
        return Err("session_id required".into());
    }
    let expires_at = chrono::Utc::now().timestamp() + PROGRESS_TOKEN_TTL_SECS;
    let secret = progress_hmac_secret(access_pwd);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("HMAC init failed: {e}"))?;
    mac.update(canonical_progress_payload(session_id, expires_at).as_bytes());
    let token = format!("{:x}", mac.finalize().into_bytes());
    Ok((token, expires_at))
}

pub fn verify_upload_progress_token(
    session_id: &str,
    expires_at: i64,
    token: &str,
    access_pwd: &str,
) -> bool {
    if session_id.trim().is_empty() || token.trim().is_empty() || expires_at <= 0 {
        return false;
    }
    if chrono::Utc::now().timestamp() >= expires_at {
        return false;
    }
    let secret = progress_hmac_secret(access_pwd);
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(canonical_progress_payload(session_id, expires_at).as_bytes());
    let expected = format!("{:x}", mac.finalize().into_bytes());
    crate::http_middleware::constant_time_eq(&expected, token.trim())
}

/// Shared gate for `/upload_status`, `/upload_events`, `/upload_ws`.
pub fn verify_upload_progress_request(req: &HttpRequest, access_pwd: &str) -> Option<HttpResponse> {
    verify_upload_progress_access(req, access_pwd)
}

fn verify_upload_progress_access(req: &HttpRequest, access_pwd: &str) -> Option<HttpResponse> {
    let session_id = parse_session_id(req);
    let token = parse_progress_token(req);
    if let Some(exp) = parse_progress_exp(req) {
        if verify_upload_progress_token(&session_id, exp, &token, access_pwd) {
            return None;
        }
    }
    Some(HttpResponse::Unauthorized().body("missing or invalid upload progress auth"))
}
async fn forward_progress_to_ws(
    mut session: actix_ws::Session,
    mut msg_stream: actix_ws::MessageStream,
    mut rx: broadcast::Receiver<ProgressEvent>,
    sid: String,
) {
    loop {
        tokio::select! {
            ev = rx.recv() => {
                match ev {
                    Ok(progress) if progress.session_id == sid => {
                        // Convert to legacy format for backwards compatibility
                        let legacy = UploadProgressEvent {
                            session_id: progress.session_id.clone(),
                            filename: progress.filename.clone(),
                            uploaded_chunks: progress.uploaded_chunks,
                            total_chunks: progress.total_chunks,
                            status: progress.status.clone(),
                        };
                        let is_done = progress.status == "completed" || progress.status == "failed";
                        if let Ok(json) = serde_json::to_string(&legacy) {
                            if session.text(json).await.is_err() {
                                break;
                            }
                        }
                        if is_done {
                            let _ = session.close(None).await;
                            break;
                        }
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = msg_stream.next() => {
                match msg {
                    Some(Ok(Message::Ping(bytes))) => {
                        if session.pong(&bytes).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

#[get("/upload_events")]
async fn upload_events(
    req: HttpRequest,
    hub: web::Data<Arc<UploadProgressHub>>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
) -> impl Responder {
    if let Some(resp) = verify_upload_progress_access(&req, &auth.config.access_pwd) {
        return resp;
    }
    let session_id = parse_session_id(&req);
    if session_id.is_empty() {
        return HttpResponse::BadRequest().body("missing session_id");
    }

    let mut rx = hub.subscribe(&session_id).await;
    let sid = session_id.clone();

    let body = stream! {
        loop {
            match rx.recv().await {
                Ok(ev) if ev.session_id == sid => {
                    // Convert to legacy format
                    let legacy = UploadProgressEvent {
                        session_id: ev.session_id.clone(),
                        filename: ev.filename.clone(),
                        uploaded_chunks: ev.uploaded_chunks,
                        total_chunks: ev.total_chunks,
                        status: ev.status.clone(),
                    };
                    let is_done = ev.status == "completed" || ev.status == "failed";
                    if let Ok(json) = serde_json::to_string(&legacy) {
                        yield Ok::<_, actix_web::Error>(web::Bytes::from(format!("data: {json}\n\n")));
                    }
                    if is_done {
                        break;
                    }
                }
                Ok(_) => continue,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };

    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .content_type("text/event-stream; charset=utf-8")
        .streaming(body)
}

#[get("/upload_ws")]
async fn upload_ws(
    req: HttpRequest,
    body: web::Payload,
    hub: web::Data<Arc<UploadProgressHub>>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
) -> Result<HttpResponse, actix_web::Error> {
    if let Some(resp) = verify_upload_progress_access(&req, &auth.config.access_pwd) {
        return Ok(resp);
    }
    let session_id = parse_session_id(&req);
    if session_id.is_empty() {
        return Ok(HttpResponse::BadRequest().body("missing session_id"));
    }

    let (response, session, msg_stream) = actix_ws::handle(&req, body)?;
    let rx = hub.subscribe(&session_id).await;
    actix_web::rt::spawn(forward_progress_to_ws(session, msg_stream, rx, session_id));
    Ok(response)
}

#[derive(Deserialize)]
struct UploadProgressTokenBody {
    session_id: String,
}

#[derive(Serialize)]
struct UploadProgressTokenResponse {
    session_id: String,
    token: String,
    expires_at: i64,
}

fn require_progress_token_admin(
    req: &HttpRequest,
    config: &crate::server_config::ServerConfig,
) -> Option<HttpResponse> {
    if crate::admin_routes::check_access_pwd(req, config) {
        return None;
    }
    Some(HttpResponse::Unauthorized().json(serde_json::json!({
        "error": {
            "code": "ADMIN_REQUIRED",
            "message": "X-Access-Pwd is required to issue an upload progress token"
        }
    })))
}

#[post("/upload_progress_token")]
async fn post_upload_progress_token(
    req: HttpRequest,
    body: web::Json<UploadProgressTokenBody>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
) -> impl Responder {
    // Legacy browser upload progress belongs to the administrator Web session.
    // API keys (especially unscoped global keys in multi-tenant mode) must not
    // mint a token for an arbitrary session_id.
    if let Some(resp) = require_progress_token_admin(&req, &auth.config) {
        return resp;
    }
    let session_id = body.session_id.trim();
    if session_id.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": { "code": "MISSING_SESSION", "message": "session_id required" }
        }));
    }
    match issue_upload_progress_token(session_id, &auth.config.access_pwd) {
        Ok((token, expires_at)) => HttpResponse::Ok().json(UploadProgressTokenResponse {
            session_id: session_id.to_string(),
            token,
            expires_at,
        }),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "error": { "code": "TOKEN_FAILED", "message": e }
        })),
    }
}

pub fn configure_upload_progress(cfg: &mut web::ServiceConfig) {
    cfg.service(upload_events)
        .service(upload_ws)
        .service(post_upload_progress_token);
}

pub async fn emit_chunk_progress(
    hub: &Arc<UploadProgressHub>,
    db: &crate::db::DbConnection,
    session_id: &str,
    filename: &str,
) {
    let Ok(Some((total, status, fname))) = crate::db::get_upload_session_summary(db, session_id)
    else {
        return;
    };
    let Ok(chunks) = crate::db::get_upload_session_chunks(db, session_id) else {
        return;
    };
    let uploaded = chunks.iter().filter(|c| c.status == "uploaded").count() as i32;
    hub.emit(UploadProgressEvent {
        session_id: session_id.to_string(),
        filename: if filename.is_empty() {
            fname
        } else {
            filename.to_string()
        },
        uploaded_chunks: uploaded,
        total_chunks: total,
        status,
    })
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_rt::test]
    async fn hub_delivers_events_to_subscriber() {
        let hub = Arc::new(UploadProgressHub::memory_only());
        let mut rx = hub.subscribe("sess-1").await;
        hub.emit(UploadProgressEvent {
            session_id: "sess-1".into(),
            filename: "a.bin".into(),
            uploaded_chunks: 1,
            total_chunks: 3,
            status: "active".into(),
        })
        .await;
        let ev = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout")
            .expect("closed");
        assert_eq!(ev.uploaded_chunks, 1);
    }

    #[test]
    fn parse_session_id_from_query() {
        let req = actix_web::test::TestRequest::get()
            .uri("/upload_events?session_id=abc%2F123")
            .to_http_request();
        assert_eq!(parse_session_id(&req), "abc/123");
    }

    #[test]
    fn progress_token_roundtrip() {
        let (token, exp) = issue_upload_progress_token("sess-abc", "test-pwd").unwrap();
        assert!(verify_upload_progress_token(
            "sess-abc", exp, &token, "test-pwd"
        ));
        assert!(!verify_upload_progress_token(
            "sess-abc", exp, &token, "wrong"
        ));
        assert!(!verify_upload_progress_token(
            "other", exp, &token, "test-pwd"
        ));
        assert!(!verify_upload_progress_token(
            "sess-abc",
            chrono::Utc::now().timestamp(),
            &token,
            "test-pwd"
        ));
    }

    #[test]
    fn progress_request_rejects_access_password_in_query() {
        let req = actix_web::test::TestRequest::get()
            .uri("/upload_events?session_id=sess-abc&pwd=test-pwd")
            .to_http_request();

        let response = verify_upload_progress_request(&req, "test-pwd")
            .expect("query password must not authorize upload progress");
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn progress_request_accepts_valid_token() {
        let (token, exp) = issue_upload_progress_token("sess-abc", "test-pwd").unwrap();
        let uri = format!("/upload_events?session_id=sess-abc&exp={exp}&token={token}");
        let req = actix_web::test::TestRequest::get()
            .uri(&uri)
            .to_http_request();

        assert!(verify_upload_progress_request(&req, "test-pwd").is_none());
    }

    #[test]
    fn progress_token_issuer_accepts_admin_password() {
        let config = crate::server_config::test_config();
        let req = actix_web::test::TestRequest::post()
            .insert_header(("X-Access-Pwd", config.access_pwd.as_str()))
            .to_http_request();

        assert!(require_progress_token_admin(&req, &config).is_none());
    }

    #[test]
    fn progress_token_issuer_rejects_api_key() {
        let config = crate::server_config::test_config();
        let req = actix_web::test::TestRequest::post()
            .insert_header(("X-API-Key", "test-api-key"))
            .to_http_request();

        let response = require_progress_token_admin(&req, &config)
            .expect("API key must not mint an administrator progress token");
        assert_eq!(response.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }
}
