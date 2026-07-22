use crate::commands::utils::resolve_peer_with_limit;
use crate::commands::TelegramState;
use crate::http_middleware::ShareBruteForceLimiter;
use crate::vpn_optimizer::NetworkConfig;
use actix_cors::Cors;
use actix_web::middleware::Compress;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use grammers_client::types::Media;

use std::sync::Arc;
use std::time::Duration;

/// Holds the per-session streaming token for Actix validation
pub struct StreamTokenData {
    pub token: String,
}

#[derive(serde::Deserialize)]
struct StreamQuery {
    token: Option<String>,
}

pub fn parse_range_header(header_val: &str, total_size: u64) -> Option<(u64, u64)> {
    if !header_val.starts_with("bytes=") {
        return None;
    }
    let s = &header_val["bytes=".len()..];
    let parts: Vec<&str> = s.split('-').collect();
    if parts.is_empty() {
        return None;
    }
    let start = parts[0].trim().parse::<u64>().ok()?;
    let end = if parts.len() > 1 && !parts[1].trim().is_empty() {
        let parsed_end = parts[1].trim().parse::<u64>().ok()?;
        std::cmp::min(parsed_end, total_size - 1)
    } else {
        total_size - 1
    };
    if start <= end {
        Some((start, end))
    } else {
        None
    }
}

fn mime_from_filename(filename: &str) -> String {
    let ext = crate::local_api::extension_from_filename(filename);
    match ext.as_str() {
        "mp4" => "video/mp4".into(),
        "webm" => "video/webm".into(),
        "mkv" => "video/x-matroska".into(),
        "mp3" => "audio/mpeg".into(),
        "wav" => "audio/wav".into(),
        "pdf" => "application/pdf".into(),
        "jpg" | "jpeg" => "image/jpeg".into(),
        "png" => "image/png".into(),
        "gif" => "image/gif".into(),
        "webp" => "image/webp".into(),
        _ => "application/octet-stream".into(),
    }
}

#[cfg(not(feature = "headless-server"))]
async fn stream_via_local_api(
    req: &actix_web::HttpRequest,
    message_id: i32,
    folder_id: Option<i64>,
    bridge: &crate::local_api::LocalApiBridge,
    db_pool: &crate::db::DbConnection,
) -> Option<HttpResponse> {
    use futures_util::StreamExt;

    if !bridge.is_usable() {
        return None;
    }

    let filename = crate::db::get_file_asset(db_pool, message_id)
        .ok()
        .flatten()
        .map(|r| r.file_name)
        .or_else(|| {
            crate::db::get_bot_file_map(db_pool, message_id)
                .ok()
                .flatten()
                .map(|r| r.file_name)
        })
        .unwrap_or_else(|| format!("file_{message_id}.bin"));

    let mime = mime_from_filename(&filename);
    let url = crate::local_api::build_download_url(bridge.port, message_id, folder_id);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()
        .ok()?;

    let mut req_builder = client.get(&url).header("X-Access-Pwd", &bridge.access_pwd);
    if let Some(range) = req.headers().get(actix_web::http::header::RANGE) {
        if let Ok(s) = range.to_str() {
            req_builder = req_builder.header(reqwest::header::RANGE, s);
        }
    }

    let resp = req_builder.send().await.ok()?;
    let status = resp.status();
    if !status.is_success() && status.as_u16() != 206 {
        log::error!(
            "Local API stream proxy failed for msg {}: {}",
            message_id,
            status
        );
        return None;
    }

    let is_partial = status.as_u16() == 206;
    let content_length = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let content_range = resp
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let byte_stream = resp.bytes_stream().map(|chunk| {
        chunk
            .map(actix_web::web::Bytes::from)
            .map_err(actix_web::error::ErrorInternalServerError)
    });

    let mut builder = if is_partial {
        HttpResponse::PartialContent()
    } else {
        HttpResponse::Ok()
    };
    if let Some(range) = content_range {
        builder.insert_header(("Content-Range", range));
    }
    if let Some(len) = content_length {
        builder.insert_header(("Content-Length", len));
    }
    Some(
        builder
            .insert_header(("Content-Type", mime))
            .insert_header(("Accept-Ranges", "bytes"))
            .insert_header(("Cache-Control", "private, max-age=120"))
            .streaming(byte_stream),
    )
}

#[get("/stream/{folder_id}/{message_id}")]
async fn stream_media(
    req: actix_web::HttpRequest,
    path: web::Path<(String, i32)>,
    query: web::Query<StreamQuery>,
    data: web::Data<Arc<TelegramState>>,
    token_data: web::Data<StreamTokenData>,
    net_config: web::Data<Arc<NetworkConfig>>,
    #[allow(unused_variables)] db_data: web::Data<crate::db::DbConnection>,
    #[allow(unused_variables)] local_bridge: web::Data<crate::local_api::LocalApiBridge>,
) -> impl Responder {
    let (folder_id_str, message_id) = path.into_inner();

    // Validate session token (constant-time comparison to prevent timing attacks)
    match &query.token {
        Some(t) if crate::http_middleware::constant_time_eq(t, &token_data.token) => {
            log::debug!(
                "Stream request: Token validated successfully for msg {}",
                message_id
            );
        }
        _ => {
            log::error!(
                "Stream request failed: Invalid or missing stream token for msg {}",
                message_id
            );
            return HttpResponse::Forbidden().body("Invalid or missing stream token");
        }
    }

    // Parse folder ID
    let folder_id = if folder_id_str == "me" || folder_id_str == "home" || folder_id_str == "null" {
        log::debug!("Stream request: Using root folder for msg {}", message_id);
        None
    } else {
        match folder_id_str.parse::<i64>() {
            Ok(id) => {
                log::debug!(
                    "Stream request: Parsed folder ID {} for msg {}",
                    id,
                    message_id
                );
                Some(id)
            }
            Err(_) => {
                log::error!(
                    "Stream request failed: Invalid folder ID format '{}' for msg {}",
                    folder_id_str,
                    message_id
                );
                return HttpResponse::BadRequest().body("Invalid folder ID");
            }
        }
    };

    let client_opt = { data.client.lock().await.clone() };

    if let Some(client) = client_opt {
        log::debug!(
            "Stream request: Client acquired, resolving peer for msg {}...",
            message_id
        );
        match resolve_peer_with_limit(
            &client,
            folder_id,
            &data.peer_cache,
            net_config.peer_cache_size(),
        )
        .await
        {
            Ok(peer) => {
                log::debug!(
                    "Stream request: Peer resolved, fetching message {}...",
                    message_id
                );
                // Try to fetch message efficiently
                match client.get_messages_by_id(peer, &[message_id]).await {
                    Ok(messages) => {
                        if let Some(Some(msg)) = messages.first() {
                            if let Some(media) = msg.media() {
                                log::debug!(
                                    "Stream request: Message and media found for msg {}",
                                    message_id
                                );
                                let size = match &media {
                                    Media::Document(d) => d.size() as u64,
                                    Media::Photo(_) => 0,
                                    _ => 0,
                                };

                                let mime = mime_type_from_media(&media);

                                // Parse Range header
                                let mut start_byte = 0;
                                let mut end_byte = if size > 0 { size - 1 } else { 0 };
                                let mut is_range = false;

                                if size > 0 {
                                    if let Some(range_header) =
                                        req.headers().get(actix_web::http::header::RANGE)
                                    {
                                        if let Ok(range_str) = range_header.to_str() {
                                            if let Some((start, end)) =
                                                parse_range_header(range_str, size)
                                            {
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

                                log::debug!(
                                    "Stream request: Starting download for msg {} (mime: {}, size: {}, range: {}-{}, content_length: {})",
                                    message_id, mime, size, start_byte, end_byte, content_length
                                );

                                // Create chunk-streaming response
                                let mut download_iter = client.iter_download(&media);
                                let mut bytes_to_skip = 0;

                                if start_byte > 0 {
                                    const MIN_CHUNK_SIZE: i32 = 4096;
                                    let max_chunk = net_config.download_chunk_i32();
                                    let chunk_index = (start_byte / MIN_CHUNK_SIZE as u64) as i32;
                                    download_iter = download_iter
                                        .chunk_size(MIN_CHUNK_SIZE)
                                        .skip_chunks(chunk_index)
                                        .chunk_size(max_chunk);
                                    bytes_to_skip = (start_byte
                                        - (chunk_index as u64 * MIN_CHUNK_SIZE as u64))
                                        as usize;
                                } else {
                                    download_iter =
                                        download_iter.chunk_size(net_config.download_chunk_i32());
                                }

                                let stream = async_stream::stream! {
                                    let mut chunk_count = 0;
                                    let mut skipped = 0;
                                    let mut total_yielded = 0;

                                    while let Some(chunk) = download_iter.next().await.transpose() {
                                        match chunk {
                                            Ok(data) => {
                                                chunk_count += 1;
                                                if chunk_count % 100 == 0 {
                                                    log::debug!("Stream request: Streamed {} chunks for msg {}", chunk_count, message_id);
                                                }

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
                                            },
                                            Err(e) => {
                                                log::error!("Stream error on msg {}: {}", message_id, e);
                                                break;
                                            }
                                        }
                                    }
                                    log::debug!("Stream request: Stream completed for msg {} (total chunks: {}, yielded: {})", message_id, chunk_count, total_yielded);
                                };

                                if is_range {
                                    return HttpResponse::PartialContent()
                                        .insert_header(("Content-Type", mime))
                                        .insert_header((
                                            "Content-Range",
                                            format!("bytes {}-{}/{}", start_byte, end_byte, size),
                                        ))
                                        .insert_header((
                                            "Content-Length",
                                            content_length.to_string(),
                                        ))
                                        .insert_header(("Accept-Ranges", "bytes"))
                                        .insert_header(("Cache-Control", "private, max-age=120"))
                                        .streaming(stream);
                                } else {
                                    return HttpResponse::Ok()
                                        .insert_header(("Content-Type", mime))
                                        .insert_header(("Content-Length", size.to_string()))
                                        .insert_header(("Accept-Ranges", "bytes"))
                                        .insert_header(("Cache-Control", "private, max-age=120"))
                                        .streaming(stream);
                                }
                            } else {
                                log::error!(
                                    "Stream request failed: Media not found in message {}",
                                    message_id
                                );
                            }
                        } else {
                            log::error!("Stream request failed: Message {} not found", message_id);
                        }
                        HttpResponse::NotFound().body("Message or media not found")
                    }
                    Err(e) => {
                        log::error!(
                            "Stream request failed: Error fetching message {}: {}",
                            message_id,
                            e
                        );
                        HttpResponse::InternalServerError()
                            .body(format!("Failed to fetch message: {}", e))
                    }
                }
            }
            Err(e) => {
                log::error!(
                    "Stream request failed: Peer resolution error for msg {}: {}",
                    message_id,
                    e
                );
                HttpResponse::BadRequest().body(format!("Peer resolution failed: {}", e))
            }
        }
    } else {
        #[cfg(not(feature = "headless-server"))]
        {
            if let Some(resp) =
                stream_via_local_api(&req, message_id, folder_id, &local_bridge, &db_data).await
            {
                log::debug!(
                    "Stream request: proxied via local API for msg {}",
                    message_id
                );
                return resp;
            }
        }
        log::error!(
            "Stream request failed: Telegram client not connected for msg {}",
            message_id
        );
        HttpResponse::ServiceUnavailable().body("Telegram client not connected")
    }
}

fn mime_type_from_media(media: &Media) -> String {
    match media {
        Media::Document(d) => d
            .mime_type()
            .unwrap_or("application/octet-stream")
            .to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

pub async fn start_server(
    state: Arc<TelegramState>,
    port: u16,
    token: String,
    db_pool: crate::db::DbConnection,
    net_config: Arc<NetworkConfig>,
    admin_state: crate::admin_routes::AdminState,
    transport: Arc<crate::telegram_transport::TransportHandle>,
    local_api_bridge: crate::local_api::LocalApiBridge,
) -> std::io::Result<actix_web::dev::Server> {
    let state_data = web::Data::new(state);
    let token_data = web::Data::new(StreamTokenData { token });
    let db_data = web::Data::new(db_pool);
    let net_data = web::Data::new(net_config);
    let admin_data = web::Data::new(admin_state);
    let transport_data = web::Data::new(transport);
    let share_bf_limiter = web::Data::new(ShareBruteForceLimiter::new(5, 300));
    let local_bridge_data = web::Data::new(local_api_bridge);

    log::info!("Starting Streaming Server on port {}", port);

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("tauri://localhost")
            .allowed_origin("http://localhost:1420")
            .allowed_origin("https://tauri.localhost")
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(Compress::default())
            .wrap(cors)
            .app_data(state_data.clone())
            .app_data(token_data.clone())
            .app_data(db_data.clone())
            .app_data(net_data.clone())
            .app_data(admin_data.clone())
            .app_data(transport_data.clone())
            .app_data(share_bf_limiter.clone())
            .app_data(local_bridge_data.clone())
            .service(stream_media)
            .configure(crate::share_routes::configure_share_routes)
    })
    .keep_alive(Duration::from_secs(5))
    .client_request_timeout(Duration::from_secs(120))
    .workers(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    )
    .bind(("0.0.0.0", port))?
    .run();

    log::info!(
        "Streaming Server started successfully on http://0.0.0.0:{}",
        port
    );

    Ok(server)
}

pub fn configure_stream(cfg: &mut web::ServiceConfig) {
    cfg.service(stream_media);
}
