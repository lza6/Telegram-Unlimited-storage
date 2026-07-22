//! WebDAV gateway — maps PROPFIND/GET/PUT to existing REST/upload logic (enterprise rclone/WinSCP).

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use base64::Engine;
use std::sync::Arc;

use crate::admin_routes::AdminState;
use crate::commands::TelegramState;
use crate::db;
use crate::server_config::ServerConfig;
use crate::tenant_auth;

fn webdav_auth(req: &HttpRequest, config: &ServerConfig, db: &db::DbConnection) -> Option<String> {
    if let Some(auth) = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(b64) = auth.strip_prefix("Basic ") {
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) {
                if let Ok(pair) = String::from_utf8(decoded) {
                    let (_user, pass) = pair.split_once(':').unwrap_or(("", &pair));
                    if let Some(t) = tenant_auth::resolve_tenant_from_api_key(db, config, pass) {
                        return Some(t);
                    }
                    if !config.access_pwd.is_empty()
                        && crate::http_middleware::constant_time_eq(pass, &config.access_pwd)
                    {
                        return Some("admin".to_string());
                    }
                }
            }
        }
    }
    if let Some(key) = req.headers().get("X-API-Key").and_then(|v| v.to_str().ok()) {
        return tenant_auth::resolve_tenant_from_api_key(db, config, key);
    }
    None
}

fn owner_scope(tenant_id: Option<&str>, config: &ServerConfig) -> Option<String> {
    if config.multi_tenant_enabled {
        tenant_id.map(|t| format!("tenant:{t}"))
    } else {
        None
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

async fn webdav_propfind(
    req: HttpRequest,
    admin: web::Data<AdminState>,
    path: web::Path<String>,
) -> impl Responder {
    if !admin.config.webdav_enabled {
        return HttpResponse::NotFound().finish();
    }
    let tenant = webdav_auth(&req, &admin.config, &admin.db_pool);
    if tenant.is_none() {
        return HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", r#"Basic realm="Telegram Drive WebDAV""#))
            .finish();
    }
    let owner = owner_scope(tenant.as_deref(), &admin.config);
    let rel = path.into_inner();
    let href = format!(
        "/{}{}",
        admin.config.webdav_prefix.trim_end_matches('/'),
        rel
    );

    let records = if let Some(ref o) = owner {
        db::list_file_assets_by_owner(&admin.db_pool, o, 500, 0).unwrap_or_default()
    } else {
        db::list_all_file_assets(&admin.db_pool, 500, 0).unwrap_or_default()
    };

    let mut entries = String::from(&format!(
        r#"<D:response><D:href>{}</D:href><D:propstat><D:prop><D:displayname>root</D:displayname><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"#,
        xml_escape(&href)
    ));
    for r in records {
        let file_href = format!(
            "{}/{}",
            href.trim_end_matches('/'),
            xml_escape(&r.file_name)
        );
        entries.push_str(&format!(
            r#"<D:response><D:href>{}</D:href><D:propstat><D:prop><D:displayname>{}</D:displayname><D:getcontentlength>{}</D:getcontentlength><D:resourcetype/></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"#,
            file_href,
            xml_escape(&r.file_name),
            r.file_size
        ));
    }

    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?><D:multistatus xmlns:D="DAV:">{entries}</D:multistatus>"#
    );
    HttpResponse::Ok()
        .content_type("application/xml; charset=utf-8")
        .body(body)
}

async fn webdav_options(_req: HttpRequest, admin: web::Data<AdminState>) -> impl Responder {
    if !admin.config.webdav_enabled {
        return HttpResponse::NotFound().finish();
    }
    HttpResponse::Ok()
        .insert_header(("Allow", "OPTIONS, GET, HEAD, PUT, DELETE, PROPFIND, MKCOL"))
        .insert_header(("DAV", "1"))
        .finish()
}

async fn webdav_delete(
    req: HttpRequest,
    admin: web::Data<AdminState>,
    path: web::Path<String>,
) -> impl Responder {
    if !admin.config.webdav_enabled {
        return HttpResponse::NotFound().finish();
    }
    let tenant = webdav_auth(&req, &admin.config, &admin.db_pool);
    if tenant.is_none() {
        return HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", r#"Basic realm="Telegram Drive WebDAV""#))
            .finish();
    }
    let name = path.into_inner();
    if name.is_empty() || name.contains('/') {
        return HttpResponse::BadRequest().body("DELETE expects a single filename segment");
    }
    let owner = owner_scope(tenant.as_deref(), &admin.config)
        .unwrap_or_else(|| crate::tenant_auth::OWNER_WEB.to_string());
    match db::delete_file_asset_by_name(&admin.db_pool, &owner, &name) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => HttpResponse::NotFound().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e),
    }
}

async fn webdav_mkcol(
    req: HttpRequest,
    admin: web::Data<AdminState>,
    path: web::Path<String>,
) -> impl Responder {
    if !admin.config.webdav_enabled {
        return HttpResponse::NotFound().finish();
    }
    let tenant = webdav_auth(&req, &admin.config, &admin.db_pool);
    if tenant.is_none() {
        return HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", r#"Basic realm="Telegram Drive WebDAV""#))
            .finish();
    }
    let rel = path.into_inner();
    if rel.is_empty() {
        return HttpResponse::MethodNotAllowed()
            .insert_header(("Allow", "OPTIONS, GET, PUT, DELETE, PROPFIND, MKCOL"))
            .body("Root collection already exists");
    }
    HttpResponse::Created()
        .insert_header((
            "Location",
            format!(
                "{}{}",
                admin.config.webdav_prefix.trim_end_matches('/'),
                rel
            ),
        ))
        .body("Virtual folder accepted (flat namespace; files stored by basename)")
}

async fn webdav_get(
    req: HttpRequest,
    admin: web::Data<AdminState>,
    tg_state: web::Data<Arc<TelegramState>>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
    path: web::Path<String>,
) -> impl Responder {
    if !admin.config.webdav_enabled {
        return HttpResponse::NotFound().finish();
    }
    let tenant = webdav_auth(&req, &admin.config, &admin.db_pool);
    if tenant.is_none() {
        return HttpResponse::Unauthorized().finish();
    }
    let name = path.into_inner();
    let owner = owner_scope(tenant.as_deref(), &admin.config);
    let records = if let Some(ref o) = owner {
        db::list_file_assets_by_owner(&admin.db_pool, o, 500, 0).unwrap_or_default()
    } else {
        db::list_all_file_assets(&admin.db_pool, 500, 0).unwrap_or_default()
    };
    let Some(asset) = records.into_iter().find(|r| r.file_name == name) else {
        return HttpResponse::NotFound().finish();
    };
    match crate::http_download::download_message_stream(
        &req,
        asset.message_id,
        asset.folder_id,
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

async fn webdav_put(
    req: HttpRequest,
    body: web::Bytes,
    admin: web::Data<AdminState>,
    tg_state: web::Data<Arc<TelegramState>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
    path: web::Path<String>,
) -> impl Responder {
    if !admin.config.webdav_enabled {
        return HttpResponse::NotFound().finish();
    }
    let tenant = webdav_auth(&req, &admin.config, &admin.db_pool);
    if tenant.is_none() {
        return HttpResponse::Unauthorized()
            .insert_header(("WWW-Authenticate", r#"Basic realm="Telegram Drive WebDAV""#))
            .finish();
    }
    let _file_slot = match upload_gate.try_acquire_file() {
        Some(g) => g,
        None => {
            return HttpResponse::ServiceUnavailable()
                .insert_header(("Retry-After", "3"))
                .body("Upload queue busy");
        }
    };
    let name = path.into_inner();
    if name.is_empty() || name.contains('/') {
        return HttpResponse::BadRequest().body("PUT filename must be a single path segment");
    }
    let max_bytes = (admin.config.max_upload_size_mb as usize).saturating_mul(1024 * 1024);
    if body.len() > max_bytes {
        return HttpResponse::PayloadTooLarge().finish();
    }
    let tmp = std::env::temp_dir().join(format!("td-wd-{}", uuid::Uuid::new_v4()));
    if let Err(e) = tokio::fs::write(&tmp, &body).await {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    let owner_id = owner_scope(tenant.as_deref(), &admin.config)
        .unwrap_or_else(|| crate::tenant_auth::OWNER_WEB.to_string());
    let path_str = tmp.to_string_lossy().to_string();
    match crate::http_upload::upload_file_path(
        path_str,
        None,
        &tg_state,
        &net_config,
        &admin.config,
        &admin.db_pool,
        &transport,
    )
    .await
    {
        Ok((message_id, saved_name)) => {
            let display = if saved_name.is_empty() {
                name.clone()
            } else {
                saved_name
            };
            let size = body.len() as i64;
            let _ = crate::file_access::record_uploaded_file(
                &admin.db_pool,
                message_id,
                None,
                &owner_id,
                &display,
                size,
            );
            let _ = tokio::fs::remove_file(&tmp).await;
            HttpResponse::Created()
                .insert_header((
                    "Location",
                    format!(
                        "/{}{}",
                        admin.config.webdav_prefix.trim_end_matches('/'),
                        name
                    ),
                ))
                .finish()
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            HttpResponse::BadGateway().body(e)
        }
    }
}

fn webdav_propfind_method() -> actix_web::http::Method {
    actix_web::http::Method::from_bytes(b"PROPFIND").expect("PROPFIND")
}

pub fn configure_webdav(cfg: &mut web::ServiceConfig, prefix: &str) {
    let propfind = webdav_propfind_method();
    let scope = format!("{prefix}/{{path:.*}}");
    cfg.service(
        web::resource(&scope)
            .route(web::get().to(webdav_get))
            .route(web::put().to(webdav_put))
            .route(web::delete().to(webdav_delete))
            .route(web::method(propfind.clone()).to(webdav_propfind))
            .route(
                web::method(actix_web::http::Method::from_bytes(b"MKCOL").expect("MKCOL"))
                    .to(webdav_mkcol),
            )
            .route(web::method(actix_web::http::Method::OPTIONS).to(webdav_options)),
    );
    cfg.service(
        web::resource(prefix)
            .route(
                web::get().to(|req, admin: web::Data<AdminState>| async move {
                    webdav_propfind(req, admin, web::Path::from("".to_string())).await
                }),
            )
            .route(
                web::method(propfind).to(|req, admin: web::Data<AdminState>| async move {
                    webdav_propfind(req, admin, web::Path::from("".to_string())).await
                }),
            )
            .route(web::method(actix_web::http::Method::OPTIONS).to(webdav_options)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_escapes_specials() {
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert!(!xml_escape("<x>").contains('<'));
    }
}
