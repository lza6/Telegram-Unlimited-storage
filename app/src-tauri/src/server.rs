use actix_web::{get, web, App, HttpServer, HttpResponse, Responder};
use actix_cors::Cors;
use crate::commands::TelegramState;
use crate::commands::utils::resolve_peer;
use grammers_client::types::Media;

use std::sync::Arc;

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

#[get("/stream/{folder_id}/{message_id}")]
async fn stream_media(
    req: actix_web::HttpRequest,
    path: web::Path<(String, i32)>,
    query: web::Query<StreamQuery>,
    data: web::Data<Arc<TelegramState>>,
    token_data: web::Data<StreamTokenData>,
) -> impl Responder {
    let (folder_id_str, message_id) = path.into_inner();

    // Validate session token
    match &query.token {
        Some(t) if t == &token_data.token => {
            log::debug!("Stream request: Token validated successfully for msg {}", message_id);
        },
        _ => {
            log::error!("Stream request failed: Invalid or missing stream token for msg {}", message_id);
            return HttpResponse::Forbidden().body("Invalid or missing stream token")
        },
    }
    
    // Parse folder ID
    let folder_id = if folder_id_str == "me" || folder_id_str == "home" || folder_id_str == "null" {
        log::debug!("Stream request: Using root folder for msg {}", message_id);
        None
    } else {
        match folder_id_str.parse::<i64>() {
            Ok(id) => {
                log::debug!("Stream request: Parsed folder ID {} for msg {}", id, message_id);
                Some(id)
            },
            Err(_) => {
                log::error!("Stream request failed: Invalid folder ID format '{}' for msg {}", folder_id_str, message_id);
                return HttpResponse::BadRequest().body("Invalid folder ID")
            },
        }
    };

    let client_opt = {
        data.client.lock().await.clone()
    };

    if let Some(client) = client_opt {
        log::debug!("Stream request: Client acquired, resolving peer for msg {}...", message_id);
        match resolve_peer(&client, folder_id, &data.peer_cache).await {
            Ok(peer) => {
                log::debug!("Stream request: Peer resolved, fetching message {}...", message_id);
                // Try to fetch message efficiently
                 match client.get_messages_by_id(peer, &[message_id]).await {
                    Ok(messages) => {
                        if let Some(Some(msg)) = messages.first() {
                            if let Some(media) = msg.media() {
                                log::debug!("Stream request: Message and media found for msg {}", message_id);
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
                                    if let Some(range_header) = req.headers().get(actix_web::http::header::RANGE) {
                                        if let Ok(range_str) = range_header.to_str() {
                                            if let Some((start, end)) = parse_range_header(range_str, size) {
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
                                    const MAX_CHUNK_SIZE: i32 = 512 * 1024;
                                    let chunk_index = (start_byte / MIN_CHUNK_SIZE as u64) as i32;
                                    download_iter = download_iter
                                        .chunk_size(MIN_CHUNK_SIZE)
                                        .skip_chunks(chunk_index)
                                        .chunk_size(MAX_CHUNK_SIZE);
                                    bytes_to_skip = (start_byte - (chunk_index as u64 * MIN_CHUNK_SIZE as u64)) as usize;
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
                                        .insert_header(("Content-Range", format!("bytes {}-{}/{}", start_byte, end_byte, size)))
                                        .insert_header(("Content-Length", content_length.to_string()))
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
                                log::error!("Stream request failed: Media not found in message {}", message_id);
                            }
                        } else {
                            log::error!("Stream request failed: Message {} not found", message_id);
                        }
                        HttpResponse::NotFound().body("Message or media not found")
                    },
                    Err(e) => {
                        log::error!("Stream request failed: Error fetching message {}: {}", message_id, e);
                        HttpResponse::InternalServerError().body(format!("Failed to fetch message: {}", e))
                    },
                 }
            },
            Err(e) => {
                log::error!("Stream request failed: Peer resolution error for msg {}: {}", message_id, e);
                HttpResponse::BadRequest().body(format!("Peer resolution failed: {}", e))
            },
        }
    } else {
        log::error!("Stream request failed: Telegram client not connected for msg {}", message_id);
        HttpResponse::ServiceUnavailable().body("Telegram client not connected")
    }
}

fn mime_type_from_media(media: &Media) -> String {
    match media {
        Media::Document(d) => d.mime_type().unwrap_or("application/octet-stream").to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

pub async fn start_server(
    state: Arc<TelegramState>,
    port: u16,
    token: String,
    db_pool: crate::db::DbConnection,
) -> std::io::Result<actix_web::dev::Server> {
    let state_data = web::Data::new(state);
    let token_data = web::Data::new(StreamTokenData { token });
    let db_data = web::Data::new(db_pool);
    
    log::info!("Starting Streaming Server on port {}", port);
    
    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("tauri://localhost")
            .allowed_origin("http://localhost:1420")
            .allowed_origin("https://tauri.localhost")
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .app_data(state_data.clone())
            .app_data(token_data.clone())
            .app_data(db_data.clone())
            .service(stream_media)
            .configure(crate::share_routes::configure_share_routes)
    })
    .bind(("0.0.0.0", port))?
    .run();

    log::info!("Streaming Server started successfully on http://0.0.0.0:{}", port);

    Ok(server)
}
