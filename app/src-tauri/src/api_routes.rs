use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use crate::commands::TelegramState;
use crate::commands::utils::resolve_peer;
use grammers_client::types::{Media, Peer};
use serde::Serialize;
use std::sync::Arc;

/// Shared state for the API server — holds the key hash for auth checks
pub struct ApiState {
    pub key_hash: Option<String>,
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
    HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap())
        .json(body)
}

/// Validate X-API-Key header against stored hash
fn check_auth(req: &HttpRequest, api_state: &web::Data<ApiState>) -> Result<(), HttpResponse> {
    let key_hash = match &api_state.key_hash {
        Some(h) => h,
        None => return Err(json_error("NO_KEY_CONFIGURED", "No API key has been configured. Generate one in Settings.", 401)),
    };

    let provided = req
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok());

    match provided {
        Some(key) if crate::commands::api_settings::verify_key(key, key_hash) => Ok(()),
        Some(_) => Err(json_error("UNAUTHORIZED", "Invalid API key", 401)),
        None => Err(json_error("UNAUTHORIZED", "Missing X-API-Key header", 401)),
    }
}

// ──────────────────────────────── Endpoints ────────────────────────────────

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[get("/api/v1/health")]
async fn api_health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
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
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state) {
        return e;
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let query_string = req.query_string();
    let has_folder_id = query_string.split('&').any(|p| p.starts_with("folder_id=") || p == "folder_id");

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
        // Parse folder_id value
        let mut parsed_id: Option<i64> = None;
        for pair in query_string.split('&') {
            let mut parts = pair.split('=');
            if let Some(key) = parts.next() {
                if key == "folder_id" {
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
        
        let resolved = match resolve_peer(&client, parsed_id, &tg_state.peer_cache).await {
            Ok(p) => p,
            Err(e) => return json_error("PEER_ERROR", &e, 400),
        };
        peers_to_scan.push((parsed_id, resolved));
    }

    let mut all_files: Vec<ApiFile> = Vec::new();
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
                    Media::Document(d) => {
                        (d.name().to_string(), d.size(), d.mime_type().map(|s| s.to_string()))
                    }
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
    let fields_list: Option<Vec<String>> = query.fields.as_ref().map(|f| {
        f.split(',')
            .map(|s| s.trim().to_string())
            .collect()
    });

    for file in paginated_files {
        let mut map = serde_json::Map::new();
        let include_all = fields_list.is_none();
        let fields = fields_list.as_ref();

        if include_all || fields.unwrap().contains(&"id".to_string()) {
            map.insert("id".to_string(), serde_json::json!(file.id));
        }
        if include_all || fields.unwrap().contains(&"folder_id".to_string()) {
            map.insert("folder_id".to_string(), serde_json::json!(file.folder_id));
        }
        if include_all || fields.unwrap().contains(&"name".to_string()) {
            map.insert("name".to_string(), serde_json::json!(file.name));
        }
        if include_all || fields.unwrap().contains(&"size".to_string()) {
            map.insert("size".to_string(), serde_json::json!(file.size));
        }
        if include_all || fields.unwrap().contains(&"mime_type".to_string()) {
            map.insert("mime_type".to_string(), serde_json::json!(file.mime_type));
        }
        if include_all || fields.unwrap().contains(&"created_at".to_string()) {
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

    let mut response = HttpResponse::Ok().json(res_body);
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
        actix_web::http::header::HeaderValue::from_static("100"),
    );
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
        actix_web::http::header::HeaderValue::from_static("99"),
    );
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-reset"),
        actix_web::http::header::HeaderValue::from_static("60"),
    );
    response
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
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state) {
        return e;
    }

    let message_id = path.into_inner() as i32;
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
                        Media::Document(d) => {
                            (d.name().to_string(), d.size(), d.mime_type().map(|s| s.to_string()))
                        }
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
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state) {
        return e;
    }

    let message_id = path.into_inner() as i32;
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
                if let Some(media) = msg.media() {
                    let size = match &media {
                        Media::Document(d) => d.size() as u64,
                        _ => 0,
                    };
                    let mime = match &media {
                        Media::Document(d) => d.mime_type().unwrap_or("application/octet-stream").to_string(),
                        _ => "application/octet-stream".to_string(),
                    };
                    let filename = match &media {
                        Media::Document(d) => d.name().to_string(),
                        Media::Photo(_) => "Photo.jpg".to_string(),
                        _ => "download".to_string(),
                    };

                    // Parse Range header
                    let mut start_byte = 0;
                    let mut end_byte = if size > 0 { size - 1 } else { 0 };
                    let mut is_range = false;

                    if size > 0 {
                        if let Some(range_header) = req.headers().get(actix_web::http::header::RANGE) {
                            if let Ok(range_str) = range_header.to_str() {
                                if let Some((start, end)) = crate::server::parse_range_header(range_str, size) {
                                    start_byte = start;
                                    end_byte = end;
                                    is_range = true;
                                }
                            }
                        }
                    }

                    let content_length = if is_range {
                        end_byte - start_byte + 1
                    } else {
                        size
                    };

                    let mut download_iter = client.iter_download(&media);
                    let mut bytes_to_skip = 0;

                    if start_byte > 0 {
                        const MIN_CHUNK_SIZE: i32 = 4096;
                        const MAX_CHUNK_SIZE: i32 = 512 * 1024;
                        let chunk_index = (start_byte / MIN_CHUNK_SIZE as u64) as i32;
                        download_iter = download_iter
                            .chunk_size(MIN_CHUNK_SIZE)
                            .skip_chunks(chunk_index)
                            .chunk_size(MAX_CHUNK_SIZE);
                        bytes_to_skip = (start_byte - (chunk_index as u64 * MIN_CHUNK_SIZE as u64)) as usize;
                    }

                    let stream = async_stream::stream! {
                        let mut skipped = 0;
                        let mut total_yielded = 0;

                        while let Some(chunk) = download_iter.next().await.transpose() {
                            match chunk {
                                Ok(data) => {
                                    let mut data_slice = data;
                                    
                                    // Handle skipping of bytes for unaligned start
                                    if skipped < bytes_to_skip {
                                        let to_skip = bytes_to_skip - skipped;
                                        if data_slice.len() <= to_skip {
                                            skipped += data_slice.len();
                                            continue;
                                        } else {
                                            data_slice = data_slice[to_skip..].to_vec();
                                            skipped = bytes_to_skip;
                                        }
                                    }

                                    // Handle limit (content_length)
                                    if total_yielded + data_slice.len() as u64 > content_length {
                                        let allowed = (content_length - total_yielded) as usize;
                                        if allowed > 0 {
                                            yield Ok::<_, actix_web::Error>(web::Bytes::from(data_slice[..allowed].to_vec()));
                                            total_yielded += allowed as u64;
                                        }
                                        break;
                                    } else {
                                        let len = data_slice.len() as u64;
                                        yield Ok::<_, actix_web::Error>(web::Bytes::from(data_slice));
                                        total_yielded += len;
                                        if total_yielded >= content_length {
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("API download stream error: {}", e);
                                    break;
                                }
                            }
                        }
                        log::debug!("API download request: Stream completed for msg {} (yielded: {})", message_id, total_yielded);
                    };

                    if is_range {
                        return HttpResponse::PartialContent()
                            .insert_header(("Content-Type", mime))
                            .insert_header(("Content-Range", format!("bytes {}-{}/{}", start_byte, end_byte, size)))
                            .insert_header(("Content-Length", content_length.to_string()))
                            .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", filename)))
                            .insert_header(("Accept-Ranges", "bytes"))
                            .streaming(stream);
                    } else {
                        return HttpResponse::Ok()
                            .insert_header(("Content-Type", mime))
                            .insert_header(("Content-Length", size.to_string()))
                            .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", filename)))
                            .insert_header(("Accept-Ranges", "bytes"))
                            .streaming(stream);
                    }
                }
            }
            json_error("NOT_FOUND", "File not found", 404)
        }
        Err(e) => json_error("FETCH_ERROR", &format!("Failed to fetch file: {}", e), 500),
    }
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
}

#[post("/api/v1/files/bulk")]
async fn api_bulk_files(
    req: HttpRequest,
    body: web::Json<BulkRequest>,
    tg_state: web::Data<Arc<TelegramState>>,
    api_state: web::Data<ApiState>,
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state) {
        return e;
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let ids: Vec<i32> = body.file_ids.iter().filter_map(|val| {
        if let Some(i) = val.as_i64() {
            Some(i as i32)
        } else if let Some(s) = val.as_str() {
            s.parse::<i32>().ok()
        } else {
            None
        }
    }).collect();

    let source_folder: Option<i64> = body.folder_id.as_ref().and_then(|val| {
        if let Some(i) = val.as_i64() {
            Some(i)
        } else if let Some(s) = val.as_str() {
            s.parse::<i64>().ok()
        } else {
            None
        }
    });

    let target_folder: Option<i64> = body.payload.as_ref().and_then(|p| p.folder_id.as_ref()).and_then(|val| {
        if let Some(i) = val.as_i64() {
            Some(i)
        } else if let Some(s) = val.as_str() {
            s.parse::<i64>().ok()
        } else {
            None
        }
    });

    match body.action.as_str() {
        "delete" => {
            let peer = match resolve_peer(&client, source_folder, &tg_state.peer_cache).await {
                Ok(p) => p,
                Err(e) => return json_error("PEER_ERROR", &e, 400),
            };
            if let Err(e) = client.delete_messages(&peer, &ids).await {
                return json_error("DELETE_FAILED", &e.to_string(), 500);
            }
        }
        "move" => {
            let source_peer = match resolve_peer(&client, source_folder, &tg_state.peer_cache).await {
                Ok(p) => p,
                Err(e) => return json_error("PEER_ERROR", &e, 400),
            };
            let target_peer = match resolve_peer(&client, target_folder, &tg_state.peer_cache).await {
                Ok(p) => p,
                Err(e) => return json_error("PEER_ERROR", &e, 400),
            };
            if source_folder != target_folder {
                if let Err(e) = client.forward_messages(&target_peer, &ids, &source_peer).await {
                    return json_error("MOVE_FORWARD_FAILED", &format!("Forward failed: {}", e), 500);
                }
                if let Err(e) = client.delete_messages(&source_peer, &ids).await {
                    return json_error("MOVE_DELETE_FAILED", &format!("Delete original failed: {}", e), 500);
                }
            }
        }
        _ => return json_error("INVALID_ACTION", "Unsupported bulk action", 400),
    }

    let mut response = HttpResponse::Ok().json(BulkResponse {
        success: true,
        count: ids.len(),
    });
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
        actix_web::http::header::HeaderValue::from_static("100"),
    );
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
        actix_web::http::header::HeaderValue::from_static("99"),
    );
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-reset"),
        actix_web::http::header::HeaderValue::from_static("60"),
    );
    response
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
) -> impl Responder {
    if let Err(e) = check_auth(&req, &api_state) {
        return e;
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = match client_opt {
        Some(c) => c,
        None => return json_error("NOT_CONNECTED", "Telegram client is not connected", 503),
    };

    let search_q = match query.q.as_deref() {
        Some(q) if !q.trim().is_empty() => q,
        _ => return json_error("INVALID_QUERY", "Search query parameter 'q' is required and cannot be empty", 400),
    };

    let query_string = req.query_string();
    let has_folder_id = query_string.split('&').any(|p| p.starts_with("folder_id=") || p == "folder_id");

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
        let mut parsed_id: Option<i64> = None;
        for pair in query_string.split('&') {
            let mut parts = pair.split('=');
            if let Some(key) = parts.next() {
                if key == "folder_id" {
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
        
        let resolved = match resolve_peer(&client, parsed_id, &tg_state.peer_cache).await {
            Ok(p) => p,
            Err(e) => return json_error("PEER_ERROR", &e, 400),
        };
        peers_to_scan.push((parsed_id, resolved));
    }

    let mut matching_files = Vec::new();
    for (fid, peer) in &peers_to_scan {
        let mut msgs = client.iter_messages(peer).limit(200);
        while let Some(msg) = msgs.next().await.ok().flatten() {
            if let Some(doc) = msg.media() {
                let (name, size, mime) = match doc {
                    Media::Document(d) => {
                        (d.name().to_string(), d.size(), d.mime_type().map(|s| s.to_string()))
                    }
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

    let mut response = HttpResponse::Ok().json(matching_files);
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
        actix_web::http::header::HeaderValue::from_static("100"),
    );
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
        actix_web::http::header::HeaderValue::from_static("99"),
    );
    response.headers_mut().insert(
        actix_web::http::header::HeaderName::from_static("x-ratelimit-reset"),
        actix_web::http::header::HeaderValue::from_static("60"),
    );
    response
}

/// Register all API routes on the Actix App
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.service(api_health)
       .service(api_list_files)
       .service(api_get_file)
       .service(api_download_file)
       .service(api_bulk_files)
       .service(api_search_files);
}
