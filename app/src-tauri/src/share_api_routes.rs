use actix_web::{delete, get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::api_routes::{check_auth, ApiState};
use crate::db::DbConnection;
use crate::server_config::ServerConfig;
use crate::sharing_core::{create_share, list_shares, revoke_share, revoke_share_for_owner};
use std::sync::Arc;

#[derive(Clone)]
pub struct ShareApiState {
    pub config: Arc<ServerConfig>,
    /// Desktop REST serves `/d/*` on stream_port (14201), not on the API port.
    pub use_stream_port_for_shares: bool,
}

#[derive(Deserialize)]
struct CreateShareBody {
    folder_id: Option<i64>,
    message_id: i32,
    file_name: String,
    file_size: i64,
    password: Option<String>,
    expiry_hours: Option<i64>,
}

fn share_json_error(message: &str) -> HttpResponse {
    HttpResponse::InternalServerError().json(serde_json::json!({
        "error": {
            "code": "INTERNAL_ERROR",
            "message": message
        }
    }))
}

fn share_owner_filter(
    caller: &crate::tenant_auth::CallerIdentity,
    multi_tenant_enabled: bool,
) -> Result<Option<String>, HttpResponse> {
    if matches!(caller, crate::tenant_auth::CallerIdentity::Anonymous) {
        return Err(HttpResponse::Forbidden().json(serde_json::json!({
            "error": { "code": "FORBIDDEN", "message": "Authenticated identity is required" }
        })));
    }
    if !multi_tenant_enabled {
        return Ok(None);
    }
    match caller {
        crate::tenant_auth::CallerIdentity::Admin => Ok(None),
        crate::tenant_auth::CallerIdentity::Tenant { tenant_id } => {
            Ok(Some(format!("tenant:{tenant_id}")))
        }
        crate::tenant_auth::CallerIdentity::Anonymous => unreachable!("handled above"),
    }
}

pub fn share_link_base(req: &HttpRequest, state: &ShareApiState) -> String {
    if state.use_stream_port_for_shares {
        crate::ui_settings::share_base_url_from_data_dir(
            &state.config.data_dir,
            state.config.stream_port,
        )
    } else {
        crate::ui_settings::effective_base_url(req, &state.config)
    }
}

#[get("/api/v1/shares")]
async fn list_share_links(
    req: HttpRequest,
    api_state: web::Data<ApiState>,
    share_state: web::Data<ShareApiState>,
    db: web::Data<DbConnection>,
) -> impl Responder {
    let caller = match check_auth(&req, &api_state, &db, &share_state.config) {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    let owner_filter = match share_owner_filter(&caller, share_state.config.multi_tenant_enabled) {
        Ok(owner_filter) => owner_filter,
        Err(response) => return response,
    };
    // Lazy cleanup: prune expired shares on every list request
    let _ = crate::sharing_core::cleanup_expired(&db);
    match list_shares(
        &db,
        &share_link_base(&req, &share_state),
        owner_filter.as_deref(),
    ) {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => share_json_error(&e),
    }
}

#[post("/api/v1/shares")]
async fn create_share_link(
    req: HttpRequest,
    body: web::Json<CreateShareBody>,
    api_state: web::Data<ApiState>,
    share_state: web::Data<ShareApiState>,
    db: web::Data<DbConnection>,
) -> impl Responder {
    let caller = match check_auth(&req, &api_state, &db, &share_state.config) {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    if body.message_id <= 0 || body.file_name.trim().is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": {
                "code": "BAD_REQUEST",
                "message": "message_id must be positive and file_name is required"
            }
        }));
    }
    let password = body.password.as_ref().and_then(|p| {
        let t = p.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });
    let owner_id = if share_state.config.multi_tenant_enabled {
        Some(caller.owner_id_for_asset())
    } else {
        None
    };
    let transport = crate::telegram_transport::TransportHandle::new(
        &share_state.config.data_dir,
        share_state.config.default_transport_mode,
    );
    let mode = transport.effective_mode(&share_state.config).await;
    let bot_mode = mode == crate::telegram_transport::TelegramTransportMode::Bot;
    if let Err(msg) = crate::file_access::assert_share_create_allowed(
        &db,
        body.message_id,
        &caller,
        share_state.config.multi_tenant_enabled,
        bot_mode,
    ) {
        let forbidden = share_state.config.multi_tenant_enabled
            && (msg.contains("Access denied") || msg.contains("asset index"));
        if forbidden {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "error": { "code": "FORBIDDEN", "message": msg }
            }));
        }
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": { "code": "NOT_DOWNLOADABLE", "message": msg }
        }));
    }
    match create_share(
        &db,
        &share_link_base(&req, &share_state),
        body.folder_id,
        body.message_id,
        body.file_name.clone(),
        body.file_size,
        password,
        body.expiry_hours,
        owner_id.as_deref(),
    ) {
        Ok(info) => HttpResponse::Ok().json(info),
        Err(e) => share_json_error(&e),
    }
}

#[delete("/api/v1/shares/{id}")]
async fn delete_share_link(
    req: HttpRequest,
    path: web::Path<String>,
    api_state: web::Data<ApiState>,
    share_state: web::Data<ShareApiState>,
    db: web::Data<DbConnection>,
) -> impl Responder {
    let caller = match check_auth(&req, &api_state, &db, &share_state.config) {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    let id = path.into_inner();
    match caller {
        crate::tenant_auth::CallerIdentity::Admin => match revoke_share(&db, &id) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "revoked": true })),
            Err(e) => share_json_error(&e),
        },
        crate::tenant_auth::CallerIdentity::Tenant { tenant_id }
            if share_state.config.multi_tenant_enabled =>
        {
            match revoke_share_for_owner(&db, &id, &format!("tenant:{tenant_id}")) {
                Ok(true) => HttpResponse::Ok().json(serde_json::json!({ "revoked": true })),
                Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
                    "error": "Share not found",
                })),
                Err(e) if e.contains("not owned") => {
                    HttpResponse::Forbidden().json(serde_json::json!({ "error": e }))
                }
                Err(e) => share_json_error(&e),
            }
        }
        crate::tenant_auth::CallerIdentity::Tenant { .. } => match revoke_share(&db, &id) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "revoked": true })),
            Err(e) => share_json_error(&e),
        },
        crate::tenant_auth::CallerIdentity::Anonymous => {
            HttpResponse::Forbidden().json(serde_json::json!({ "error": "Forbidden" }))
        }
    }
}

pub fn configure_share_api(cfg: &mut web::ServiceConfig) {
    cfg.service(list_share_links)
        .service(create_share_link)
        .service(delete_share_link);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_share_filter_uses_authenticated_identity() {
        let caller = crate::tenant_auth::CallerIdentity::Tenant {
            tenant_id: "tenant-a".to_string(),
        };
        assert_eq!(
            share_owner_filter(&caller, true).unwrap().as_deref(),
            Some("tenant:tenant-a")
        );
    }

    #[test]
    fn admin_share_filter_can_view_all_tenants() {
        assert_eq!(
            share_owner_filter(&crate::tenant_auth::CallerIdentity::Admin, true).unwrap(),
            None
        );
    }

    #[test]
    fn anonymous_share_filter_fails_closed() {
        let response = share_owner_filter(&crate::tenant_auth::CallerIdentity::Anonymous, true)
            .expect_err("anonymous owner filtering must not fail open");
        assert_eq!(response.status(), actix_web::http::StatusCode::FORBIDDEN);
    }
}
