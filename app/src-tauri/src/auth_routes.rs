use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use grammers_client::SignInError;
use grammers_tl_types as tl;

use crate::commands::TelegramState;
use crate::server_config::ServerConfig;

#[derive(Clone)]
pub struct AuthRouteState {
    pub config: Arc<ServerConfig>,
}

fn require_admin_access(req: &HttpRequest, config: &ServerConfig) -> Option<HttpResponse> {
    if crate::admin_routes::check_access_pwd(req, config) {
        return None;
    }
    Some(HttpResponse::Unauthorized().json(serde_json::json!({
        "error": {
            "code": "UNAUTHORIZED",
            "message": "Missing or invalid X-Access-Pwd (Web admin login password required)"
        }
    })))
}

#[derive(Serialize)]
struct AuthError {
    error: String,
}

#[derive(Serialize)]
struct AuthStatus {
    connected: bool,
    user: Option<String>,
    credentials_ok: bool,
    transport_mode: String,
    bot_configured: bool,
    user_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

#[derive(Deserialize)]
struct PhoneRequest {
    phone: String,
}

#[derive(Deserialize)]
struct PhoneSignIn {
    code: String,
}

#[derive(Deserialize)]
struct PhonePassword {
    password: String,
}

#[derive(Serialize)]
struct QrStartResponse {
    url: String,
    authorized: bool,
}

fn json_error(status: actix_web::http::StatusCode, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status).json(AuthError {
        error: message.into(),
    })
}

fn bad_request(message: impl Into<String>) -> HttpResponse {
    json_error(actix_web::http::StatusCode::BAD_REQUEST, message)
}

fn service_unavailable(message: impl Into<String>) -> HttpResponse {
    json_error(actix_web::http::StatusCode::SERVICE_UNAVAILABLE, message)
}

fn credentials_guard(config: &ServerConfig) -> Option<HttpResponse> {
    config
        .telegram_credentials_placeholder()
        .map(|hint| bad_request(hint))
}

/// 将区号+本地号或纯数字整理为 Telegram 要求的 +国际格式。
fn normalize_phone(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("请输入手机号".into());
    }
    let compact: String = trimmed
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect();
    if compact.starts_with('+') {
        if compact.len() < 9 {
            return Err("国际号码过短，请检查国家/地区区号与手机号".into());
        }
        return Ok(compact);
    }
    let digits: String = compact.chars().filter(|c| c.is_ascii_digit()).collect();
    // 中国大陆 11 位手机号（1 开头）
    if digits.len() == 11 && digits.starts_with('1') {
        return Ok(format!("+86{}", digits));
    }
    // 已带 86 前缀但未写 +
    if digits.len() == 13 && digits.starts_with("86") {
        return Ok(format!("+{}", digits));
    }
    Err(
        "无法识别手机号：请选择国家/地区并填写号码，或直接输入 +8613800138000 格式".into(),
    )
}

#[cfg(test)]
mod phone_tests {
    use super::normalize_phone;

    #[test]
    fn china_local_eleven_digits() {
        assert_eq!(normalize_phone("13800138000").unwrap(), "+8613800138000");
    }

    #[test]
    fn e164_passthrough() {
        assert_eq!(normalize_phone("+86 138 0013 8000").unwrap(), "+8613800138000");
    }
}

/// True when User-mode Telegram client exists and `get_me()` succeeds (same bar as auth status).
pub async fn user_telegram_connected(tg_state: &Arc<crate::commands::TelegramState>) -> bool {
    match tg_state.client.lock().await.clone() {
        Some(client) => client.get_me().await.is_ok(),
        None => false,
    }
}

#[get("/api/v1/auth/status")]
async fn auth_status(
    auth: web::Data<AuthRouteState>,
    tg_state: web::Data<Arc<TelegramState>>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    let mode = transport.effective_mode(&auth.config).await;
    let bot_configured = crate::telegram_transport::TransportHandle::bot_configured(&auth.config);
    let user_configured = crate::telegram_transport::TransportHandle::user_configured(&auth.config);
    let credentials_ok = match mode {
        crate::telegram_transport::TelegramTransportMode::Bot => bot_configured,
        crate::telegram_transport::TelegramTransportMode::User => {
            auth.config.telegram_credentials_placeholder().is_none()
        }
    };
    let hint = match mode {
        crate::telegram_transport::TelegramTransportMode::Bot if !bot_configured => Some(
            "机器人模式需要 TG_BOT_TOKEN 与 TG_STORAGE_CHANNEL_ID，并将机器人设为频道管理员。",
        ),
        crate::telegram_transport::TelegramTransportMode::User => {
            auth.config.telegram_credentials_placeholder()
        }
        _ => None,
    };

    if mode == crate::telegram_transport::TelegramTransportMode::Bot {
        let bot_name = if bot_configured {
            crate::telegram_transport::bot_test_connection_cached(&auth.config)
                .await
                .ok()
        } else {
            None
        };
        let connected = bot_name.is_some();
        return HttpResponse::Ok().json(AuthStatus {
            connected,
            user: bot_name.map(|n| format!("bot @{n}")),
            credentials_ok,
            transport_mode: mode.as_str().to_string(),
            bot_configured,
            user_configured,
            hint,
        });
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    if let Some(client) = client_opt {
        if let Ok(me) = client.get_me().await {
            let name = format!(
                "{} {}",
                me.first_name().unwrap_or(""),
                me.last_name().unwrap_or("")
            )
            .trim()
            .to_string();
            return HttpResponse::Ok().json(AuthStatus {
                connected: true,
                user: Some(name),
                credentials_ok,
                transport_mode: mode.as_str().to_string(),
                bot_configured,
                user_configured,
                hint,
            });
        }
    }
    HttpResponse::Ok().json(AuthStatus {
        connected: false,
        user: None,
        credentials_ok,
        transport_mode: mode.as_str().to_string(),
        bot_configured,
        user_configured,
        hint,
    })
}

#[post("/api/v1/auth/phone/request")]
async fn phone_request(
    req: HttpRequest,
    body: web::Json<PhoneRequest>,
    auth: web::Data<AuthRouteState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    if let Some(resp) = require_admin_access(&req, &auth.config) {
        return resp;
    }
    if let Some(resp) = credentials_guard(&auth.config) {
        return resp;
    }
    let api_id = auth.config.telegram_api_id;
    let api_hash = auth.config.telegram_api_hash.clone();
    if let Err(e) = crate::commands::auth::ensure_client_initialized_at(
        &auth.config.data_dir,
        &net_config,
        &tg_state,
        api_id,
    )
    .await
    {
        log::error!("Auth service unavailable: {}", e);
        return service_unavailable(format!("Telegram 客户端初始化失败: {e}"));
    }
    let phone = match normalize_phone(&body.phone) {
        Ok(p) => p,
        Err(e) => return bad_request(e),
    };
    let client = match { tg_state.client.lock().await.clone() } {
        Some(c) => c,
        None => return service_unavailable("Telegram 客户端未就绪"),
    };
    log::info!("Requesting Telegram login code for {}", phone);
    match client.request_login_code(&phone, api_hash.as_str()).await {
        Ok(token) => {
            *tg_state.login_token.lock().await = Some(token);
            HttpResponse::Ok().json(serde_json::json!({ "sent": true }))
        }
        Err(e) => {
            log::error!("Auth request failed: {}", e);
            bad_request(format!("发送验证码失败: {e}"))
        }
    }
}

#[post("/api/v1/auth/phone/sign-in")]
async fn phone_sign_in(
    req: HttpRequest,
    body: web::Json<PhoneSignIn>,
    auth: web::Data<AuthRouteState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    if let Some(resp) = require_admin_access(&req, &auth.config) {
        return resp;
    }
    if let Some(resp) = credentials_guard(&auth.config) {
        return resp;
    }
    let api_id = auth.config.telegram_api_id;
    if let Err(e) = crate::commands::auth::ensure_client_initialized_at(
        &auth.config.data_dir,
        &net_config,
        &tg_state,
        api_id,
    )
    .await
    {
        log::error!("Auth service unavailable: {}", e);
        return service_unavailable(format!("Telegram 客户端初始化失败: {e}"));
    }
    let client = match { tg_state.client.lock().await.clone() } {
        Some(c) => c,
        None => return service_unavailable("Telegram 客户端未就绪"),
    };
    let mut token_guard = tg_state.login_token.lock().await;
    let Some(login_token) = token_guard.take() else {
        return bad_request("请先调用 /api/v1/auth/phone/request 发送验证码");
    };
    match client.sign_in(&login_token, &body.code).await {
        Ok(_) => {
            *tg_state.api_id.lock().await = Some(api_id);
            HttpResponse::Ok().json(serde_json::json!({ "connected": true }))
        }
        Err(SignInError::PasswordRequired(token)) => {
            *tg_state.password_token.lock().await = Some(token);
            HttpResponse::Ok().json(serde_json::json!({
                "connected": false,
                "next_step": "password"
            }))
        }
        Err(e) => {
            *token_guard = Some(login_token);
            log::error!("Auth sign-in failed: {}", e);
            bad_request(format!("验证码登录失败: {e}"))
        }
    }
}

#[post("/api/v1/auth/phone/password")]
async fn phone_password(
    req: HttpRequest,
    body: web::Json<PhonePassword>,
    auth: web::Data<AuthRouteState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    if let Some(resp) = require_admin_access(&req, &auth.config) {
        return resp;
    }
    if let Some(resp) = credentials_guard(&auth.config) {
        return resp;
    }
    let api_id = auth.config.telegram_api_id;
    if let Err(e) = crate::commands::auth::ensure_client_initialized_at(
        &auth.config.data_dir,
        &net_config,
        &tg_state,
        api_id,
    )
    .await
    {
        log::error!("Auth service unavailable: {}", e);
        return service_unavailable(format!("Telegram 客户端初始化失败: {e}"));
    }
    let client = match { tg_state.client.lock().await.clone() } {
        Some(c) => c,
        None => return service_unavailable("Telegram 客户端未就绪"),
    };
    let mut pw_guard = tg_state.password_token.lock().await;
    let Some(pw_token) = pw_guard.take() else {
        return bad_request("当前没有待提交的两步验证步骤");
    };
    match client.check_password(pw_token, body.password.as_str()).await {
        Ok(_) => {
            *tg_state.api_id.lock().await = Some(api_id);
            HttpResponse::Ok().json(serde_json::json!({ "connected": true }))
        }
        Err(e) => {
            log::error!("Auth password failed: {}", e);
            bad_request(format!("两步验证失败: {e}"))
        }
    }
}

#[post("/api/v1/auth/qr/start")]
async fn qr_start(
    req: HttpRequest,
    auth: web::Data<AuthRouteState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    if let Some(resp) = require_admin_access(&req, &auth.config) {
        return resp;
    }
    if let Some(resp) = credentials_guard(&auth.config) {
        return resp;
    }
    let api_id = auth.config.telegram_api_id;
    let api_hash = auth.config.telegram_api_hash.clone();
    *tg_state.api_id.lock().await = Some(api_id);
    if let Err(e) = crate::commands::auth::ensure_client_initialized_at(
        &auth.config.data_dir,
        &net_config,
        &tg_state,
        api_id,
    )
    .await
    {
        log::error!("Auth service unavailable: {}", e);
        return service_unavailable(format!("Telegram 客户端初始化失败: {e}"));
    }
    let client = match { tg_state.client.lock().await.clone() } {
        Some(c) => c,
        None => return service_unavailable("Telegram 客户端未就绪"),
    };
    let result = client
        .invoke(&tl::functions::auth::ExportLoginToken {
            api_id,
            api_hash: api_hash.clone(),
            except_ids: vec![],
        })
        .await;

    match result {
        Ok(tl::enums::auth::LoginToken::Token(t)) => {
            let url = format!("tg://login?token={}", URL_SAFE_NO_PAD.encode(&t.token));
            HttpResponse::Ok().json(QrStartResponse {
                url,
                authorized: false,
            })
        }
        Ok(tl::enums::auth::LoginToken::Success(_)) => HttpResponse::Ok().json(QrStartResponse {
            url: String::new(),
            authorized: true,
        }),
        Ok(tl::enums::auth::LoginToken::MigrateTo(m)) => {
            let url = format!("tg://login?token={}", URL_SAFE_NO_PAD.encode(&m.token));
            HttpResponse::Ok().json(QrStartResponse {
                url,
                authorized: false,
            })
        }
        Err(e) => bad_request(format!("生成登录二维码失败: {e}")),
    }
}

#[get("/api/v1/auth/qr/poll")]
async fn qr_poll(
    req: HttpRequest,
    auth: web::Data<AuthRouteState>,
    tg_state: web::Data<Arc<TelegramState>>,
) -> impl Responder {
    if let Some(resp) = require_admin_access(&req, &auth.config) {
        return resp;
    }
    let client = match { tg_state.client.lock().await.clone() } {
        Some(c) => c,
        None => return service_unavailable("Telegram 客户端未就绪"),
    };
    match client.is_authorized().await {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "connected": true,
            "next_step": "dashboard"
        })),
        Ok(false) => HttpResponse::Ok().json(serde_json::json!({
            "connected": false,
            "next_step": "waiting"
        })),
        Err(e) => {
            log::error!("Auth poll failed: {}", e);
            bad_request(format!("检查登录状态失败: {e}"))
        }
    }
}

#[derive(Serialize)]
struct TransportInfoResponse {
    active_mode: String,
    default_mode: String,
    bot_configured: bool,
    user_configured: bool,
    available_modes: Vec<String>,
}

#[derive(Deserialize)]
struct SetTransportModeRequest {
    mode: String,
}

#[get("/api/v1/transport")]
async fn transport_info(
    req: HttpRequest,
    auth: web::Data<AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Some(resp) = crate::admin_routes::require_admin_or_api_key(&req, &auth.config) {
        return resp;
    }
    let active = transport.active_mode().await;
    let mut available = Vec::new();
    if crate::telegram_transport::TransportHandle::bot_configured(&auth.config) {
        available.push("bot".to_string());
    }
    if crate::telegram_transport::TransportHandle::user_configured(&auth.config) {
        available.push("user".to_string());
    }
    HttpResponse::Ok().json(TransportInfoResponse {
        active_mode: active.as_str().to_string(),
        default_mode: auth.config.default_transport_mode.as_str().to_string(),
        bot_configured: crate::telegram_transport::TransportHandle::bot_configured(&auth.config),
        user_configured: crate::telegram_transport::TransportHandle::user_configured(&auth.config),
        available_modes: available,
    })
}

#[post("/api/v1/transport/mode")]
async fn transport_set_mode(
    req: HttpRequest,
    auth: web::Data<AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    db: web::Data<crate::db::DbConnection>,
    body: web::Json<SetTransportModeRequest>,
) -> impl Responder {
    if let Some(resp) = crate::admin_routes::require_admin_or_api_key(&req, &auth.config) {
        return resp;
    }

    let mode = match crate::telegram_transport::TelegramTransportMode::parse(&body.mode) {
        Some(m) => m,
        None => return bad_request("mode must be 'bot' or 'user'"),
    };

    match mode {
        crate::telegram_transport::TelegramTransportMode::Bot
            if !crate::telegram_transport::TransportHandle::bot_configured(&auth.config) =>
        {
            return bad_request("Bot mode is not configured (TG_BOT_TOKEN / TG_STORAGE_CHANNEL_ID)");
        }
        crate::telegram_transport::TelegramTransportMode::User
            if !crate::telegram_transport::TransportHandle::user_configured(&auth.config) =>
        {
            return bad_request("User mode is not configured (TELEGRAM_API_ID / TELEGRAM_API_HASH)");
        }
        _ => {}
    }

    if let Err(e) = transport.set_mode(mode).await {
        return bad_request(e);
    }

    if let Err(e) = crate::db::set_file_index_complete(&db, false) {
        log::warn!("file index invalidate on transport switch: {e}");
    }

    HttpResponse::Ok().json(serde_json::json!({
        "ok": true,
        "transport_mode": mode.as_str(),
    }))
}

pub fn configure_auth(cfg: &mut web::ServiceConfig) {
    cfg.service(auth_status)
        .service(transport_info)
        .service(transport_set_mode)
        .service(phone_request)
        .service(phone_sign_in)
        .service(phone_password)
        .service(qr_start)
        .service(qr_poll);
}
