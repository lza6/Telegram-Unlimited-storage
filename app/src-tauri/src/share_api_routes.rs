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
    if let Err(e) = check_auth(&req, &api_state, &db, &share_state.config) {
        return e;
    }
    let owner_filter = if share_state.config.multi_tenant_enabled {
        crate::tenant_auth::api_key_tenant(&req, &db, &share_state.config)
            .map(|tid| format!("tenant:{tid}"))
    } else {
        None
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
    if let Err(e) = check_auth(&req, &api_state, &db, &share_state.config) {
        return e;
    }
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
        crate::tenant_auth::api_key_tenant(&req, &db, &share_state.config)
            .map(|tid| format!("tenant:{tid}"))
    } else {
        None
    };
    if share_state.config.multi_tenant_enabled {
        let caller = crate::tenant_auth::api_key_tenant(&req, &db, &share_state.config)
            .map(|tenant_id| crate::tenant_auth::CallerIdentity::Tenant { tenant_id })
            .unwrap_or(crate::tenant_auth::CallerIdentity::Tenant {
                tenant_id: "default".to_string(),
            });
        if let Err(msg) =
            crate::file_access::assert_download_allowed(&db, body.message_id, &caller, true)
        {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "error": { "code": "FORBIDDEN", "message": msg }
            }));
        }
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
    if let Err(e) = check_auth(&req, &api_state, &db, &share_state.config) {
        return e;
    }
    let id = path.into_inner();
    if share_state.config.multi_tenant_enabled {
        let owner = crate::tenant_auth::api_key_tenant(&req, &db, &share_state.config)
            .map(|tid| format!("tenant:{tid}"))
            .unwrap_or_else(|| "tenant:default".to_string());
        match revoke_share_for_owner(&db, &id, &owner) {
            Ok(true) => HttpResponse::Ok().json(serde_json::json!({ "revoked": true })),
            Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
                "error": "Share not found",
            })),
            Err(e) if e.contains("not owned") => {
                HttpResponse::Forbidden().json(serde_json::json!({ "error": e }))
            }
            Err(e) => share_json_error(&e),
        }
    } else {
        match revoke_share(&db, &id) {
            Ok(()) => HttpResponse::Ok().json(serde_json::json!({ "revoked": true })),
            Err(e) => share_json_error(&e),
        }
    }
}

pub fn configure_share_api(cfg: &mut web::ServiceConfig) {
    cfg.service(list_share_links)
        .service(create_share_link)
        .service(delete_share_link);
}
