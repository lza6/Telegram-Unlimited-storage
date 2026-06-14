use crate::commands::utils::{resolve_peer, TempFileGuard};
use crate::commands::TelegramState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use futures_util::StreamExt;
use grammers_client::types::{Media, Peer};
use serde::Serialize;
use std::sync::Arc;

/// Shared state for the API server — holds the key hash for auth checks
pub struct ApiState {
    pub key_hash: Option<String>,
    pub max_upload_size_mb: u32,
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

fn json_error(code: &str, message: &str, status: u16) -> HttpResponse {
    let body = ErrorBody {
        error: ErrorDetail {
            code: code.to_string(),
            message: message.to_string(),
        },
    };
    let status = actix_web::http::StatusCode::from_u16(status)
        .unwrap_or(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
    HttpResponse::build(status).json(body)
}

fn include_field(fields: Option<&Vec<String>>, name: &str) -> bool {
    fields.map(|f| f.iter().any(|x| x == name)).unwrap_or(true)
}

/// Validate API access: tenant key, global `X-API-Key`, or Web 管理台 `X-Access-Pwd`.
pub(crate) fn check_auth(
    req: &HttpRequest,
    api_state: &web::Data<ApiState>,
    db: &crate::db::DbConnection,
    config: &crate::server_config::ServerConfig,
) -> Result<(), HttpResponse> {
    if config.multi_tenant_enabled {
        if crate::tenant_auth::api_key_tenant(req, db, config).is_some() {
            return Ok(());
        }
    }

    if let Some(key) = req.headers().get("X-API-Key").and_then(|v| v.to_str().ok()) {
        if let Some(h) = &api_state.key_hash {
            if crate::commands::api_settings::verify_and_upgrade_key(key, h, &config.data_dir) {
                return Ok(());
            }
            return Err(json_error("UNAUTHORIZED", "Invalid API key", 401));
        }
    }

    if crate::admin_routes::check_access_pwd(req, config) {
        return Ok(());
    }

    if req.headers().get("X-API-Key").is_some() {
        return Err(json_error("UNAUTHORIZED", "Invalid API key", 401));
    }

    if api_state.key_hash.is_some() {
        return Err(json_error(
            "UNAUTHORIZED",
            "Missing X-API-Key header or X-Access-Pwd",
            401,
        ));
    }

    Err(json_error(
        "NO_KEY_CONFIGURED",
        "No API key has been configured. Generate one in Settings.",
        401,
    ))
}

fn api_caller(
    req: &HttpRequest,
    db: &crate::db::DbConnection,
    config: &crate::server_config::ServerConfig,
) -> crate::tenant_auth::CallerIdentity {
    crate::tenant_auth::api_key_tenant(req, db, config)
        .map(|tenant_id| crate::tenant_auth::CallerIdentity::Tenant { tenant_id })
        .unwrap_or(crate::tenant_auth::CallerIdentity::Tenant {
            tenant_id: "default".to_string(),
        })
}

fn asset_record_to_api_file(r: crate::db::FileAssetRecord) -> ApiFile {
    ApiFile {
        id: r.message_id as i64,
        folder_id: r.folder_id,
        name: r.file_name,
        size: r.file_size.max(0) as u64,
        mime_type: None,
        created_at: chrono::DateTime::from_timestamp(r.created_at, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| r.created_at.to_string()),
    }
}

fn uses_asset_index(
    mode: crate::telegram_transport::TelegramTransportMode,
    config: &crate::server_config::ServerConfig,
    db: &crate::db::DbConnection,
) -> bool {
    crate::file_access::asset_index_authoritative(mode, config, db)
}

fn bot_storage_folder_id(config: &crate::server_config::ServerConfig) -> Option<i64> {
    config
        .storage_channel_id
        .as_ref()
        .and_then(|ch| ch.trim().parse::<i64>().ok())
}

fn file_from_bot_map(record: crate::db::BotFileRecord, folder_id: Option<i64>) -> ApiFile {
    ApiFile {
        id: record.message_id as i64,
        folder_id,
        name: record.file_name,
        size: record.file_size,
        mime_type: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

// ──────────────────────────────── Endpoints ────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    /// Same as `ready`: Telegram transport can accept uploads/downloads.
    pub telegram_connected: bool,
    pub uptime_secs: u64,
    pub build: String,
    pub ready: bool,
    pub transport_mode: String,
    pub bot_configured: bool,
    pub user_configured: bool,
    pub upload_queue: crate::upload_gate::UploadQueueStatus,
    pub metadata_cache_enabled: bool,
    pub metadata_cache_ttl_secs: u64,
    pub public_file_id_download: bool,
    pub upload_share_ttl_hours: i64,
    pub presigned_download_enabled: bool,
    pub multi_tenant_enabled: bool,
}

#[get("/api/v1/health")]
async fn api_health(
    tg_state: web::Data<Arc<TelegramState>>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
) -> impl Responder {
    let mode = transport.effective_mode(&auth.config).await;
    let ready = match mode {
        crate::telegram_transport::TelegramTransportMode::Bot => {
            crate::telegram_transport::bot_connection_ready(&auth.config).await
        }
        crate::telegram_transport::TelegramTransportMode::User => {
            crate::auth_routes::user_telegram_connected(&tg_state).await
        }
    };
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        telegram_connected: ready,
        uptime_secs: crate::server_uptime::uptime_secs(),
        build: format!(
            "{}-{}",
            env!("CARGO_PKG_VERSION"),
            option_env!("GITHUB_SHA").unwrap_or("local")
        ),
        ready,
        transport_mode: mode.as_str().to_string(),
        bot_configured: crate::telegram_transport::TransportHandle::bot_configured(&auth.config),
        user_configured: crate::telegram_transport::TransportHandle::user_configured(&auth.config),
        upload_queue: upload_gate.status(),
        metadata_cache_enabled: auth.config.metadata_cache_enabled,
        metadata_cache_ttl_secs: auth.config.metadata_cache_ttl_secs,
        public_file_id_download: auth.config.public_file_id_download,
        upload_share_ttl_hours: auth.config.upload_share_ttl_hours,
        presigned_download_enabled: auth.config.download_signing_secret.is_some(),
        multi_tenant_enabled: auth.config.multi_tenant_enabled,
    })
}

fn metadata_cache_header(
    layer: crate::metadata_cache::CacheLayer,
) -> Option<(&'static str, &'static str)> {
    match layer {
        crate::metadata_cache::CacheLayer::Hit => Some(("X-Metadata-Cache", "HIT")),
        crate::metadata_cache::CacheLayer::Miss => Some(("X-Metadata-Cache", "MISS")),
        crate::metadata_cache::CacheLayer::Bypass => None,
    }
}

fn files_list_cacheable(q: &FilesQuery) -> bool {
    q.search.is_none()
        && q.mime_type.is_none()
        && q.size_min.is_none()
        && q.size_max.is_none()
        && q.created_after.is_none()
        && q.created_before.is_none()
        && q.offset_id.is_none()
}

fn parse_files_folder_scope(query_string: &str) -> (bool, Option<i64>) {
    let has_folder_id = query_string
        .split('&')
        .any(|p| p.starts_with("folder_id=") || p == "folder_id");
    let mut parsed_id: Option<i64> = None;
    if has_folder_id {
        for pair in query_string.split('&') {
            let mut parts = pair.split('=');
            if parts.next() == Some("folder_id") {
                if let Some(val) = parts.next() {
                    if !val.is_empty() && val != "null" && val != "none" && val != "None" {
                        if let Ok(id) = val.parse::<i64>() {
                            parsed_id = Some(id);
                        }
                    }
                }
            }
        }
    }
    (has_folder_id, parsed_id)
}

fn cached_file_to_api(c: crate::metadata_cache::CachedFile) -> ApiFile {
    ApiFile {
        id: c.id,
        folder_id: c.folder_id,
        name: c.name,
        size: c.size,
        mime_type: c.mime_type,
        created_at: c.created_at,
    }
}

fn api_file_to_cached(f: &ApiFile) -> crate::metadata_cache::CachedFile {
    crate::metadata_cache::CachedFile {
        id: f.id,
        folder_id: f.folder_id,
        name: f.name.clone(),
        size: f.size,
        mime_type: f.mime_type.clone(),
        created_at: f.created_at.clone(),
    }
}

#[derive(serde::Deserialize, Clone)]
struct FilesQuery {
    #[allow(dead_code)]
    folder_id: Option<String>,
    page: Option<u32>,
    limit: Option<u32>,
    search: Option<String>,
    offset_id: Option<i32>,
    sort: Option<String>,
    order: Option<String>,
    mime_type: Option<String>,
    created_after: Option<String>,
    created_before: Option<String>,
    size_min: Option<u64>,
    size_max: Option<u64>,
    fields: Option<String>,
}

#[derive(Serialize)]
struct FilesResponse {
    data: Vec<serde_json::Value>,
    files: Vec<serde_json::Value>, // For backwards compatibility
    page: u32,
    limit: u32,
    total: usize,
    pagination: PaginationInfo,
}

#[derive(Serialize)]
struct PaginationInfo {
    page: u32,
    limit: u32,
    total: usize,
    total_pages: u32,
    has_next: bool,
    has_prev: bool,
}

#[derive(Serialize, Clone)]
struct ApiFile {
    id: i64,
    folder_id: Option<i64>,
    name: String,
    size: u64,
    mime_type: Option<String>,
    created_at: String,
}

#[get("/api/v1/files")]
async fn api_list_files(
    req: HttpRequest,
    query: web::Query<FilesQuery>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    db: web::Data<crate::db::DbConnection>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let mode = transport.effective_mode(&auth.config).await;
    let use_asset_index = uses_asset_index(mode, &auth.config, &db);

    if use_asset_index {
        let page = query.page.unwrap_or(1).max(1);
        let limit = query.limit.unwrap_or(20).min(100).max(1);
        let offset = ((page - 1) * limit) as usize;
        let (has_folder_id, parsed_folder_id) = parse_files_folder_scope(req.query_string());

        let owner_id = if auth.config.multi_tenant_enabled {
            Some(api_caller(&req, &db, &auth.config).owner_id_for_asset())
        } else {
            None
        };

        let name_filter = query
            .search
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let records = match crate::db::list_file_assets_scoped(
            &db,
            owner_id.as_deref(),
            parsed_folder_id,
            has_folder_id,
            name_filter,
            limit as usize,
            offset,
        ) {
            Ok(r) => r,
            Err(e) => return json_error("DB_ERROR", &e, 500),
        };

        let total = match crate::db::count_file_assets_scoped(
            &db,
            owner_id.as_deref(),
            parsed_folder_id,
            has_folder_id,
            name_filter,
        ) {
            Ok(n) => n,
            Err(e) => return json_error("DB_ERROR", &e, 500),
        };

        let files: Vec<ApiFile> = records.into_iter().map(asset_record_to_api_file).collect();
        let total_pages = ((total + limit as usize - 1) / limit as usize).max(1) as u32;
        let has_next = page < total_pages;
        let has_prev = page > 1;
        let final_data: Vec<serde_json::Value> = files
            .iter()
            .map(|f| {
                serde_json::json!({
                    "id": f.id,
                    "folder_id": f.folder_id,
                    "name": f.name,
                    "size": f.size,
                    "mime_type": f.mime_type,
                    "created_at": f.created_at,
                })
            })
            .collect();
        let mut res = HttpResponse::Ok();
        res.insert_header(("X-File-Index", "assets"));
        return res.json(FilesResponse {
            data: final_data.clone(),
            files: final_data,
            page,
            limit,
            total,
            pagination: PaginationInfo {
                page,
                limit,
                total,
                total_pages,
                has_next,
                has_prev,
            },
        });
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let query_string = req.query_string();
    let (has_folder_id, parsed_folder_id) = parse_files_folder_scope(query_string);
    let use_cache = auth.config.metadata_cache_enabled && files_list_cacheable(&query);
    let cache_key = crate::metadata_cache::files_cache_key(parsed_folder_id);
    let mut cache_layer = crate::metadata_cache::CacheLayer::Bypass;

    let mut peers_to_scan = Vec::new();
    if !has_folder_id {
        // Return files from ALL folders: scan dialogs + root folder
        if let Ok(me_peer) = resolve_peer(&client, None, &tg_state.peer_cache).await {
            peers_to_scan.push((None, me_peer));
        }
        let mut dialogs = client.iter_dialogs();
        while let Some(dialog) = dialogs.next().await.ok().flatten() {
            if let Peer::Channel(ref c) = dialog.peer {
                let name = c.raw.title.clone();
                if name.to_lowercase().contains("[td]") {
                    peers_to_scan.push((Some(c.raw.id), dialog.peer.clone()));
                }
            }
        }
    } else {
        let resolved = match resolve_peer(&client, parsed_folder_id, &tg_state.peer_cache).await {
            Ok(p) => p,
            Err(e) => return json_error("PEER_ERROR", &e, 400),
        };
        peers_to_scan.push((parsed_folder_id, resolved));
    }

    let mut all_files: Vec<ApiFile> = Vec::new();
    if use_cache {
        if let Some(cached) =
            crate::metadata_cache::get_files(&db, &cache_key, auth.config.metadata_cache_ttl_secs)
        {
            all_files = cached.into_iter().map(cached_file_to_api).collect();
            cache_layer = crate::metadata_cache::CacheLayer::Hit;
        }
    }

    if cache_layer != crate::metadata_cache::CacheLayer::Hit {
        for (fid, peer) in &peers_to_scan {
            let mut msgs = client.iter_messages(peer);
            if let Some(offset_id) = query.offset_id {
                msgs = msgs.offset_id(offset_id);
            }

            // When listing all, limit scan per folder to prevent rate limit timeouts
            if !has_folder_id {
                msgs = msgs.limit(100);
            } else if query.search.is_none() {
                let page = query.page.unwrap_or(1).max(1);
                let limit = query.limit.unwrap_or(20).min(100).max(1);
                if query.offset_id.is_some() {
                    msgs = msgs.limit(limit as usize * 2);
                } else {
                    msgs = msgs.limit(page as usize * limit as usize * 2);
                }
            } else {
                msgs = msgs.limit(2000);
            }

            while let Some(msg) = msgs.next().await.ok().flatten() {
                if let Some(doc) = msg.media() {
                    let (name, size, mime) = match doc {
                        Media::Document(d) => (
                            d.name().to_string(),
                            d.size(),
                            d.mime_type().map(|s| s.to_string()),
                        ),
                        Media::Photo(_) => ("Photo.jpg".to_string(), 0, Some("image/jpeg".into())),
                        _ => ("Unknown".to_string(), 0, None),
                    };

                    all_files.push(ApiFile {
                        id: msg.id() as i64,
                        folder_id: *fid,
                        name,
                        size: size as u64,
                        mime_type: mime,
                        created_at: msg.date().to_string(),
                    });
                }
            }
        }
        if use_cache && !all_files.is_empty() {
            let to_store: Vec<_> = all_files.iter().map(api_file_to_cached).collect();
            let _ = crate::metadata_cache::put_files(&db, &cache_key, &to_store);
            cache_layer = crate::metadata_cache::CacheLayer::Miss;
        }
    }

    // Apply filters
    let mut filtered_files: Vec<ApiFile> = Vec::new();
    for file in all_files {
        if let Some(ref search) = query.search {
            if !file.name.to_lowercase().contains(&search.to_lowercase()) {
                continue;
            }
        }
        if let Some(ref mt) = query.mime_type {
            if let Some(ref fmt) = file.mime_type {
                if !fmt.to_lowercase().contains(&mt.to_lowercase()) {
                    continue;
                }
            } else {
                continue;
            }
        }
        if let Some(min) = query.size_min {
            if file.size < min {
                continue;
            }
        }
        if let Some(max) = query.size_max {
            if file.size > max {
                continue;
            }
        }
        if let Some(ref after) = query.created_after {
            if file.created_at < *after {
                continue;
            }
        }
        if let Some(ref before) = query.created_before {
            if file.created_at > *before {
                continue;
            }
        }
        filtered_files.push(file);
    }

    // Sort
    let sort_field = query.sort.as_deref().unwrap_or("created_at");
    let sort_order = query.order.as_deref().unwrap_or("asc");
    filtered_files.sort_by(|a, b| {
        let cmp = match sort_field {
            "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            "size" => a.size.cmp(&b.size),
            _ => a.created_at.cmp(&b.created_at),
        };
        if sort_order.to_lowercase() == "desc" {
            cmp.reverse()
        } else {
            cmp
        }
    });

    // Pagination
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).min(100).max(1);
    let total = filtered_files.len();
    let total_pages = ((total + limit as usize - 1) / limit as usize) as u32;
    let start = ((page - 1) * limit) as usize;

    let paginated_files: Vec<ApiFile> = filtered_files
        .into_iter()
        .skip(start)
        .take(limit as usize)
        .collect();

    let has_next = page < total_pages;
    let has_prev = page > 1;

    // Sparse fieldsets
    let mut final_data = Vec::new();
    let fields_list: Option<Vec<String>> = query
        .fields
        .as_ref()
        .map(|f| f.split(',').map(|s| s.trim().to_string()).collect());

    for file in paginated_files {
        let mut map = serde_json::Map::new();
        let fields = fields_list.as_ref();

        if include_field(fields, "id") {
            map.insert("id".to_string(), serde_json::json!(file.id));
        }
        if include_field(fields, "folder_id") {
            map.insert("folder_id".to_string(), serde_json::json!(file.folder_id));
        }
        if include_field(fields, "name") {
            map.insert("name".to_string(), serde_json::json!(file.name));
        }
        if include_field(fields, "size") {
            map.insert("size".to_string(), serde_json::json!(file.size));
        }
        if include_field(fields, "mime_type") {
            map.insert("mime_type".to_string(), serde_json::json!(file.mime_type));
        }
        if include_field(fields, "created_at") {
            map.insert("created_at".to_string(), serde_json::json!(file.created_at));
        }

        final_data.push(serde_json::Value::Object(map));
    }

    let res_body = FilesResponse {
        data: final_data.clone(),
        files: final_data,
        page,
        limit,
        total,
        pagination: PaginationInfo {
            page,
            limit,
            total,
            total_pages,
            has_next,
            has_prev,
        },
    };

    let mut res = HttpResponse::Ok();
    if let Some((k, v)) = metadata_cache_header(cache_layer) {
        res.insert_header((k, v));
    }
    res.json(res_body)
}

#[derive(serde::Deserialize)]
struct FolderQuery {
    folder_id: Option<i64>,
}

#[get("/api/v1/files/{message_id}")]
async fn api_get_file(
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<FolderQuery>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    db: web::Data<crate::db::DbConnection>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let message_id = path.into_inner() as i32;
    if auth.config.multi_tenant_enabled {
        let caller = api_caller(&req, &db, &auth.config);
        if let Err(msg) =
            crate::file_access::assert_download_allowed(&db, message_id, &caller, true)
        {
            return json_error("FORBIDDEN", &msg, 403);
        }
    }

    let mode = transport.effective_mode(&auth.config).await;
    if uses_asset_index(mode, &auth.config, &db) {
        if let Ok(Some(asset)) = crate::db::get_file_asset(&db, message_id) {
            return HttpResponse::Ok().json(asset_record_to_api_file(asset));
        }
        if mode == crate::telegram_transport::TelegramTransportMode::Bot {
            if let Ok(Some(record)) = crate::db::get_bot_file_map(&db, message_id) {
                let folder_id = query
                    .folder_id
                    .or_else(|| bot_storage_folder_id(&auth.config));
                return HttpResponse::Ok().json(file_from_bot_map(record, folder_id));
            }
        }
        return json_error("NOT_FOUND", "File not found in index", 404);
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let peer = match resolve_peer(&client, query.folder_id, &tg_state.peer_cache).await {
        Ok(p) => p,
        Err(e) => return json_error("PEER_ERROR", &e, 400),
    };

    match client.get_messages_by_id(peer, &[message_id]).await {
        Ok(messages) => {
            if let Some(Some(msg)) = messages.first() {
                if let Some(doc) = msg.media() {
                    let (name, size, mime) = match doc {
                        Media::Document(d) => (
                            d.name().to_string(),
                            d.size(),
                            d.mime_type().map(|s| s.to_string()),
                        ),
                        Media::Photo(_) => ("Photo.jpg".to_string(), 0, Some("image/jpeg".into())),
                        _ => ("Unknown".to_string(), 0, None),
                    };
                    return HttpResponse::Ok().json(ApiFile {
                        id: msg.id() as i64,
                        folder_id: query.folder_id,
                        name,
                        size: size as u64,
                        mime_type: mime,
                        created_at: msg.date().to_string(),
                    });
                }
            }
            json_error("NOT_FOUND", "File not found", 404)
        }
        Err(e) => json_error("FETCH_ERROR", &format!("Failed to fetch file: {}", e), 500),
    }
}

#[get("/api/v1/files/{message_id}/download")]
async fn api_download_file(
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<FolderQuery>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    db: web::Data<crate::db::DbConnection>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let message_id = path.into_inner() as i32;
    if auth.config.multi_tenant_enabled {
        let caller = api_caller(&req, &db, &auth.config);
        if let Err(msg) =
            crate::file_access::assert_download_allowed(&db, message_id, &caller, true)
        {
            return json_error("FORBIDDEN", &msg, 403);
        }
    }

    match crate::http_download::download_message_stream(
        &req,
        message_id,
        query.folder_id,
        &tg_state,
        false,
        &auth.config,
        &db,
        &transport,
        &net_config,
    )
    .await
    {
        Ok(r) => r,
        Err(r) => r,
    }
}

#[derive(Serialize)]
struct ApiUploadResult {
    id: i32,
    /// Telegram message id (catalog only — not a download credential).
    file_id: String,
    folder_id: Option<i64>,
    name: String,
    filename: String,
    /// Opaque share link `/d/{token}` (time-limited, revocable).
    download_url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    share_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
    /// REST download; requires `X-API-Key`.
    api_download_url: String,
    /// Asset owner (`tenant:…` or `system:web`).
    owner_id: String,
    /// `presigned` | `share` | `legacy`
    link_kind: String,
}

#[derive(serde::Deserialize)]
struct BulkRequest {
    action: String,
    file_ids: Vec<serde_json::Value>,
    folder_id: Option<serde_json::Value>,
    payload: Option<BulkPayload>,
}

#[derive(serde::Deserialize)]
struct BulkPayload {
    folder_id: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct BulkResponse {
    success: bool,
    count: usize,
    /// Per-ID successes when the backend can determine them (Bot partial delete, User full batch).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    succeeded_ids: Vec<i32>,
}

#[post("/api/v1/files/bulk")]
async fn api_bulk_files(
    req: HttpRequest,
    body: web::Json<BulkRequest>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    db: web::Data<crate::db::DbConnection>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let ids: Vec<i32> = body
        .file_ids
        .iter()
        .filter_map(|val| {
            if let Some(i) = val.as_i64() {
                Some(i as i32)
            } else if let Some(s) = val.as_str() {
                s.parse::<i32>().ok()
            } else {
                None
            }
        })
        .collect();

    let source_folder: Option<i64> = body.folder_id.as_ref().and_then(|val| {
        if let Some(i) = val.as_i64() {
            Some(i)
        } else if let Some(s) = val.as_str() {
            s.parse::<i64>().ok()
        } else {
            None
        }
    });

    let target_folder: Option<i64> = body
        .payload
        .as_ref()
        .and_then(|p| p.folder_id.as_ref())
        .and_then(|val| {
            if let Some(i) = val.as_i64() {
                Some(i)
            } else if let Some(s) = val.as_str() {
                s.parse::<i64>().ok()
            } else {
                None
            }
        });

    let mode = transport.effective_mode(&auth.config).await;
    if mode == crate::telegram_transport::TelegramTransportMode::Bot {
        if body.action != "delete" {
            return json_error(
                "NOT_SUPPORTED",
                "Bot mode only supports bulk delete (removes index; Telegram messages remain)",
                400,
            );
        }
        let owner_filter = if auth.config.multi_tenant_enabled {
            Some(api_caller(&req, &db, &auth.config).owner_id_for_asset())
        } else {
            None
        };
        let mut deleted = 0usize;
        let mut succeeded_ids: Vec<i32> = Vec::new();
        for mid in &ids {
            if auth.config.multi_tenant_enabled {
                let caller = api_caller(&req, &db, &auth.config);
                if crate::file_access::assert_download_allowed(&db, *mid, &caller, true).is_err() {
                    continue;
                }
            }
            if crate::db::delete_file_asset(&db, *mid, owner_filter.as_deref()).unwrap_or(false) {
                deleted += 1;
                succeeded_ids.push(*mid);
            }
        }
        return HttpResponse::Ok().json(BulkResponse {
            success: true,
            count: deleted,
            succeeded_ids,
        });
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    match body.action.as_str() {
        "delete" => {
            let peer = match resolve_peer(&client, source_folder, &tg_state.peer_cache).await {
                Ok(p) => p,
                Err(e) => return json_error("PEER_ERROR", &e, 400),
            };
            if let Err(e) = client.delete_messages(&peer, &ids).await {
                return json_error("DELETE_FAILED", &e.to_string(), 500);
            }
            for mid in &ids {
                let _ = crate::db::delete_file_asset(&db, *mid, None);
            }
            crate::metadata_cache::invalidate_files(&db, source_folder);
        }
        "move" => {
            let source_peer = match resolve_peer(&client, source_folder, &tg_state.peer_cache).await
            {
                Ok(p) => p,
                Err(e) => return json_error("PEER_ERROR", &e, 400),
            };
            let target_peer = match resolve_peer(&client, target_folder, &tg_state.peer_cache).await
            {
                Ok(p) => p,
                Err(e) => return json_error("PEER_ERROR", &e, 400),
            };
            if source_folder != target_folder {
                let forwarded = match client
                    .forward_messages(&target_peer, &ids, &source_peer)
                    .await
                {
                    Ok(msgs) => msgs,
                    Err(e) => {
                        return json_error(
                            "MOVE_FORWARD_FAILED",
                            &format!("Forward failed: {}", e),
                            500,
                        )
                    }
                };
                let new_ids: Vec<i32> = forwarded
                    .iter()
                    .filter_map(|m| m.as_ref().map(|msg| msg.id()))
                    .collect();
                if new_ids.len() != ids.len() {
                    return json_error(
                        "MOVE_FORWARD_MISMATCH",
                        &format!(
                            "Forward returned {} message(s), expected {} — originals not deleted",
                            new_ids.len(),
                            ids.len()
                        ),
                        500,
                    );
                }
                if let Err(e) = client.delete_messages(&source_peer, &ids).await {
                    return json_error(
                        "MOVE_DELETE_FAILED",
                        &format!("Delete original failed: {}", e),
                        500,
                    );
                }
                if let Err(e) = crate::file_access::remap_file_assets_after_move(
                    &db,
                    &ids,
                    &new_ids,
                    target_folder,
                ) {
                    return json_error(
                        "MOVE_REMAP_FAILED",
                        &format!("Index remap after move failed: {e}"),
                        500,
                    );
                }
                crate::metadata_cache::invalidate_files(&db, source_folder);
                crate::metadata_cache::invalidate_files(&db, target_folder);
            }
        }
        _ => return json_error("INVALID_ACTION", "Unsupported bulk action", 400),
    }

    HttpResponse::Ok().json(BulkResponse {
        success: true,
        count: ids.len(),
        succeeded_ids: ids.clone(),
    })
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    q: Option<String>,
    #[allow(dead_code)]
    folder_id: Option<String>,
    #[allow(dead_code)]
    recursive: Option<bool>,
}

#[get("/api/v1/files/search")]
async fn api_search_files(
    req: HttpRequest,
    query: web::Query<SearchQuery>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    db: web::Data<crate::db::DbConnection>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let search_q = match query.q.as_deref() {
        Some(q) if !q.trim().is_empty() => q.trim(),
        _ => {
            return json_error(
                "INVALID_QUERY",
                "Search query parameter 'q' is required and cannot be empty",
                400,
            )
        }
    };

    let mode = transport.effective_mode(&auth.config).await;
    let (has_folder_id, parsed_folder_id) = parse_files_folder_scope(req.query_string());

    if uses_asset_index(mode, &auth.config, &db) {
        let owner = if auth.config.multi_tenant_enabled {
            Some(api_caller(&req, &db, &auth.config).owner_id_for_asset())
        } else {
            None
        };
        match crate::db::search_file_assets(
            &db,
            search_q,
            owner.as_deref(),
            parsed_folder_id,
            has_folder_id,
            200,
        ) {
            Ok(records) => {
                let files: Vec<ApiFile> =
                    records.into_iter().map(asset_record_to_api_file).collect();
                return HttpResponse::Ok().json(files);
            }
            Err(e) => return json_error("DB_ERROR", &e, 500),
        }
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let mut peers_to_scan = Vec::new();
    if !has_folder_id {
        if let Ok(me_peer) = resolve_peer(&client, None, &tg_state.peer_cache).await {
            peers_to_scan.push((None, me_peer));
        }
        let mut dialogs = client.iter_dialogs();
        while let Some(dialog) = dialogs.next().await.ok().flatten() {
            if let Peer::Channel(ref c) = dialog.peer {
                let name = c.raw.title.clone();
                if name.to_lowercase().contains("[td]") {
                    peers_to_scan.push((Some(c.raw.id), dialog.peer.clone()));
                }
            }
        }
    } else {
        let resolved = match resolve_peer(&client, parsed_folder_id, &tg_state.peer_cache).await {
            Ok(p) => p,
            Err(e) => return json_error("PEER_ERROR", &e, 400),
        };
        peers_to_scan.push((parsed_folder_id, resolved));
    }

    let mut matching_files = Vec::new();
    for (fid, peer) in &peers_to_scan {
        let mut msgs = client.iter_messages(peer).limit(200);
        while let Some(msg) = msgs.next().await.ok().flatten() {
            if let Some(doc) = msg.media() {
                let (name, size, mime) = match doc {
                    Media::Document(d) => (
                        d.name().to_string(),
                        d.size(),
                        d.mime_type().map(|s| s.to_string()),
                    ),
                    Media::Photo(_) => ("Photo.jpg".to_string(), 0, Some("image/jpeg".into())),
                    _ => ("Unknown".to_string(), 0, None),
                };

                if name.to_lowercase().contains(&search_q.to_lowercase()) {
                    matching_files.push(ApiFile {
                        id: msg.id() as i64,
                        folder_id: *fid,
                        name,
                        size: size as u64,
                        mime_type: mime,
                        created_at: msg.date().to_string(),
                    });
                }
            }
        }
    }

    HttpResponse::Ok().json(matching_files)
}

#[derive(Serialize)]
struct FolderItem {
    id: i64,
    name: String,
}

#[get("/api/v1/folders")]
async fn api_list_folders(
    req: HttpRequest,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    db: web::Data<crate::db::DbConnection>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let mode = transport.effective_mode(&auth.config).await;
    if mode == crate::telegram_transport::TelegramTransportMode::Bot {
        let folders: Vec<FolderItem> = bot_storage_folder_id(&auth.config)
            .map(|id| FolderItem {
                id,
                name: "Storage Channel".to_string(),
            })
            .into_iter()
            .collect();
        return HttpResponse::Ok().json(folders);
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    use grammers_client::types::Peer;
    use std::collections::HashMap;

    let mut cache_layer = crate::metadata_cache::CacheLayer::Bypass;
    if auth.config.metadata_cache_enabled {
        if let Some(cached) =
            crate::metadata_cache::get_folders(&db, auth.config.metadata_cache_ttl_secs)
        {
            let folders: Vec<FolderItem> = cached
                .into_iter()
                .map(|c| FolderItem {
                    id: c.id,
                    name: c.name,
                })
                .collect();
            let mut res = HttpResponse::Ok();
            res.insert_header(("X-Metadata-Cache", "HIT"));
            return res.json(folders);
        }
        cache_layer = crate::metadata_cache::CacheLayer::Miss;
    }

    let mut folders = Vec::new();
    let mut dialogs = client.iter_dialogs();
    let mut discovered = HashMap::new();

    while let Some(dialog) = dialogs.next().await.ok().flatten() {
        if let Peer::Channel(c) = &dialog.peer {
            let id = c.raw.id;
            discovered.insert(id, dialog.peer.clone());
            let name = c.raw.title.clone();
            if name.to_lowercase().contains("[td]") {
                let display_name = name
                    .replace(" [TD]", "")
                    .replace(" [td]", "")
                    .replace("[TD]", "")
                    .replace("[td]", "")
                    .trim()
                    .to_string();
                folders.push(FolderItem {
                    id,
                    name: display_name,
                });
            }
        }
    }
    {
        let mut cache = tg_state.peer_cache.write().await;
        cache.extend(discovered);
    }
    if auth.config.metadata_cache_enabled {
        let to_store: Vec<crate::metadata_cache::CachedFolder> = folders
            .iter()
            .map(|f| crate::metadata_cache::CachedFolder {
                id: f.id,
                name: f.name.clone(),
            })
            .collect();
        let _ = crate::metadata_cache::put_folders(&db, &to_store);
    }
    let mut res = HttpResponse::Ok();
    if let Some((k, v)) = metadata_cache_header(cache_layer) {
        res.insert_header((k, v));
    }
    res.json(folders)
}

#[post("/api/v1/files")]
async fn api_upload_file(
    req: HttpRequest,
    mut payload: actix_multipart::Multipart,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    net_config: web::Data<Arc<crate::vpn_optimizer::NetworkConfig>>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    db: web::Data<crate::db::DbConnection>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
    upload_gate: web::Data<Arc<crate::upload_gate::UploadGate>>,
    share_state: web::Data<crate::share_api_routes::ShareApiState>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let owner_id = api_caller(&req, &db, &auth.config).owner_id_for_asset();

    let _file_slot = match upload_gate.try_acquire_file() {
        Some(p) => p,
        None => return crate::upload_gate::response_upload_busy(3),
    };

    let max_upload_bytes = (api_state.max_upload_size_mb as usize).saturating_mul(1024 * 1024);

    let mut folder_id: Option<i64> = None;
    let mut temp_guard: Option<TempFileGuard> = None;
    let mut filename = String::from("upload.bin");

    while let Some(item) = payload.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => return json_error("MULTIPART_ERROR", &e.to_string(), 400),
        };
        let name = field.name().unwrap_or("").to_string();
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
            if let Some(cd) = field.content_disposition() {
                if let Some(fname) = cd.get_filename() {
                    filename = fname.to_string();
                }
            }
            let tmp = std::env::temp_dir().join(format!("td-api-{}", uuid::Uuid::new_v4()));
            let mut f = match std::fs::File::create(&tmp) {
                Ok(file) => file,
                Err(e) => return json_error("IO_ERROR", &e.to_string(), 500),
            };
            let mut total_read = 0usize;
            while let Some(chunk) = field.next().await {
                match chunk {
                    Ok(b) => {
                        total_read += b.len();
                        if total_read > max_upload_bytes {
                            let _ = std::fs::remove_file(&tmp);
                            return json_error(
                                "PAYLOAD_TOO_LARGE",
                                &format!(
                                    "file exceeds {} MB limit",
                                    auth.config.max_upload_size_mb
                                ),
                                413,
                            );
                        }
                        if std::io::Write::write_all(&mut f, &b).is_err() {
                            let _ = std::fs::remove_file(&tmp);
                            return json_error("IO_ERROR", "write failed", 500);
                        }
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(&tmp);
                        return json_error("MULTIPART_ERROR", &e.to_string(), 400);
                    }
                }
            }
            temp_guard = Some(TempFileGuard::new(tmp));
        }
    }

    let guard = match temp_guard {
        Some(g) => g,
        None => return json_error("MISSING_FILE", "file field required", 400),
    };

    let path_str = guard.path().to_string_lossy().to_string();
    let file_size = std::fs::metadata(&path_str)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    if let Err(e) = crate::telegram_transport::ensure_transport_ready(
        &transport,
        &auth.config,
        &auth.config.data_dir,
        &tg_state,
        &net_config,
    )
    .await
    {
        return json_error("TRANSPORT_NOT_READY", &e, 503);
    }

    match crate::http_upload::upload_file_path(
        path_str,
        folder_id,
        &tg_state,
        &net_config,
        &auth.config,
        &db,
        &transport,
    )
    .await
    {
        Ok((message_id, saved_name)) => {
            guard.keep();
            let share_base = if share_state.use_stream_port_for_shares {
                crate::share_api_routes::share_link_base(&req, &share_state)
            } else {
                crate::admin_routes::host_base(&req, &auth.config)
            };
            let api_base = if share_state.use_stream_port_for_shares {
                format!("http://127.0.0.1:{}", auth.config.port)
            } else {
                share_base.clone()
            };
            let display_name = if saved_name.is_empty() {
                filename.clone()
            } else {
                saved_name.clone()
            };
            let size = file_size;
            match crate::secure_download::issue_upload_download_link(
                &db,
                &auth.config,
                &share_base,
                folder_id,
                message_id,
                display_name.clone(),
                size,
                &owner_id,
                false,
            ) {
                Ok(link) => HttpResponse::Ok().json(ApiUploadResult {
                    id: message_id,
                    file_id: link.file_id,
                    folder_id,
                    name: display_name.clone(),
                    filename: display_name,
                    download_url: link.download_url,
                    share_id: link.share_id,
                    expires_at: link.expires_at,
                    api_download_url: crate::admin_routes::api_download_url(
                        &api_base, message_id, folder_id,
                    ),
                    owner_id: link.owner_id,
                    link_kind: link.link_kind.to_string(),
                }),
                Err(e) => json_error("DOWNLOAD_LINK_FAILED", &e, 500),
            }
        }
        Err(e) => json_error("UPLOAD_FAILED", &e, 500),
    }
}

#[derive(serde::Deserialize)]
struct RebuildIndexRequest {
    folder_ids: Option<Vec<Option<i64>>>,
}

#[post("/api/v1/files/rebuild-index")]
async fn api_rebuild_file_index(
    req: HttpRequest,
    body: web::Json<RebuildIndexRequest>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
    db: web::Data<crate::db::DbConnection>,
    auth: web::Data<crate::auth_routes::AuthRouteState>,
    transport: web::Data<Arc<crate::telegram_transport::TransportHandle>>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state, &db, &auth.config) {
        return e;
    }

    let mode = transport.effective_mode(&auth.config).await;
    if mode == crate::telegram_transport::TelegramTransportMode::Bot {
        return json_error(
            "NOT_SUPPORTED",
            "Rebuild index applies to User mode file_assets only",
            400,
        );
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let folder_ids = body.folder_ids.clone().unwrap_or_default();
    match crate::commands::fs::rebuild_file_index_for_folders(
        &client,
        &tg_state.peer_cache,
        &db,
        folder_ids,
        crate::tenant_auth::OWNER_WEB,
    )
    .await
    {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => json_error("REBUILD_FAILED", &e, 500),
    }
}

/// Register all API routes on the Actix App
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.service(api_health)
        .service(api_list_folders)
        .service(api_list_files)
        .service(api_upload_file)
        .service(api_get_file)
        .service(api_download_file)
        .service(api_bulk_files)
        .service(api_search_files)
        .service(api_rebuild_file_index);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    fn test_auth_context() -> (
        crate::db::DbConnection,
        std::sync::Arc<crate::server_config::ServerConfig>,
        web::Data<ApiState>,
    ) {
        let dir = std::env::temp_dir().join(format!("td-auth-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = crate::db::init_db_at(&dir).unwrap();
        let config = crate::server_config::test_config();
        let api_state = web::Data::new(ApiState {
            key_hash: Some(crate::commands::api_settings::hash_key_public("secret")),
            max_upload_size_mb: 100,
        });
        (db, config, api_state)
    }

    #[test]
    fn check_auth_rejects_missing_key() {
        let (db, config, api_state) = test_auth_context();
        let req = TestRequest::get().uri("/api/v1/files").to_http_request();
        let err = check_auth(&req, &api_state, &db, &config).unwrap_err();
        assert_eq!(err.status(), 401);
    }

    #[test]
    fn check_auth_accepts_valid_key() {
        let key = "secret";
        let (db, config, api_state) = test_auth_context();
        let req = TestRequest::get()
            .uri("/api/v1/files")
            .insert_header(("X-API-Key", key))
            .to_http_request();
        assert!(check_auth(&req, &api_state, &db, &config).is_ok());
    }

    #[test]
    fn check_auth_accepts_access_pwd() {
        let (db, config, api_state) = test_auth_context();
        let req = TestRequest::get()
            .uri("/api/v1/files")
            .insert_header(("X-Access-Pwd", config.access_pwd.as_str()))
            .to_http_request();
        assert!(check_auth(&req, &api_state, &db, &config).is_ok());
    }

    #[test]
    fn include_field_sparse_vs_all() {
        let all = vec!["id".to_string(), "name".to_string()];
        assert!(include_field(None, "id"));
        assert!(include_field(Some(&all), "id"));
        assert!(!include_field(Some(&all), "size"));
    }

    #[test]
    fn bulk_response_serializes_succeeded_ids() {
        let resp = BulkResponse {
            success: true,
            count: 2,
            succeeded_ids: vec![10, 20],
        };
        let json: serde_json::Value = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["count"], 2);
        assert_eq!(json["succeeded_ids"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn bulk_response_omits_empty_succeeded_ids() {
        let resp = BulkResponse {
            success: true,
            count: 0,
            succeeded_ids: vec![],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("succeeded_ids"));
    }

    #[test]
    fn parse_files_folder_scope_variants() {
        let (has, id) = parse_files_folder_scope("page=1&folder_id=42");
        assert!(has);
        assert_eq!(id, Some(42));
        let (has, id) = parse_files_folder_scope("folder_id=");
        assert!(has);
        assert_eq!(id, None);
        let (has, id) = parse_files_folder_scope("page=2");
        assert!(!has);
        assert_eq!(id, None);
    }
}
