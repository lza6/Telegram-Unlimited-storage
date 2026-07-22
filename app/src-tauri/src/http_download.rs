use actix_web::{HttpRequest, HttpResponse};
use async_stream::stream;
use futures_util::StreamExt;
use grammers_client::types::Media;

use crate::commands::TelegramState;
use crate::db::DbConnection;
use crate::server::parse_range_header;
use crate::server_config::ServerConfig;
use crate::telegram_transport::{self, TelegramTransportMode, TransportHandle};
use crate::vpn_optimizer::{self, NetworkConfig};

pub fn is_previewable(content_type: &str) -> bool {
    content_type.starts_with("image/")
        || content_type.starts_with("video/")
        || content_type.starts_with("audio/")
        || content_type == "application/pdf"
        || content_type.starts_with("text/")
        || content_type == "application/json"
}

/// Sanitize filename for safe embedding in Content-Disposition header.
/// Escapes backslash and quote characters per RFC 2616 quoted-string rules.
/// Also strips control characters to prevent header injection.
fn sanitize_filename_for_header(filename: &str) -> String {
    let cleaned: String = filename.chars().filter(|c| !c.is_control()).collect();
    cleaned.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Encode a filename for RFC 5987 `filename*` parameter.
/// Keeps unreserved characters ([A-Za-z0-9-._~]) and %-encodes the rest.
fn rfc5987_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Build a safe Content-Disposition header value.
///
/// Returns `attachment` or `inline` with both a legacy ASCII `filename` and a
/// RFC 5987 `filename*=UTF-8''...` parameter. The legacy name keeps only
/// ASCII letters, digits, and safe punctuation to avoid header injection.
fn build_content_disposition(kind: &str, filename: &str) -> String {
    // Legacy filename: keep a conservative set of ASCII characters.
    let legacy: String = filename
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    *c,
                    ' ' | '-' | '_' | '.' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
                )
        })
        .collect();
    let legacy = sanitize_filename_for_header(&legacy);

    // RFC 5987 filename* for modern browsers (preserves Unicode).
    let encoded = rfc5987_encode(filename);

    format!("{kind}; filename=\"{legacy}\"; filename*=UTF-8''{encoded}")
}

pub async fn download_message_stream(
    req: &HttpRequest,
    message_id: i32,
    folder_id: Option<i64>,
    tg_state: &TelegramState,
    force_attachment: bool,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    net_config: &NetworkConfig,
) -> Result<HttpResponse, HttpResponse> {
    let mode = transport.effective_mode(config).await;
    download_message_stream_for_mode(
        req,
        message_id,
        folder_id,
        tg_state,
        force_attachment,
        config,
        db,
        transport,
        net_config,
        mode,
        None,
        None,
    )
    .await
}

#[derive(Clone)]
pub struct SchedulerStreamContext {
    pub guard: crate::durable_scheduler::SchedulerLeaseGuard,
}

pub async fn download_asset_locator_stream(
    req: &HttpRequest,
    locator: &crate::asset_locator::AssetLocatorRecord,
    tg_state: &TelegramState,
    force_attachment: bool,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    net_config: &NetworkConfig,
    accounting: Option<(
        crate::postgres_control_plane::PostgresControlPlane,
        crate::postgres_download_accounting::DownloadAccountingContext,
    )>,
    scheduler: Option<SchedulerStreamContext>,
) -> Result<HttpResponse, HttpResponse> {
    download_asset_locator_stream_scheduled(
        req,
        locator,
        tg_state,
        force_attachment,
        config,
        db,
        transport,
        net_config,
        accounting,
        scheduler,
    )
    .await
}

pub async fn download_asset_locator_stream_scheduled(
    req: &HttpRequest,
    locator: &crate::asset_locator::AssetLocatorRecord,
    tg_state: &TelegramState,
    force_attachment: bool,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    net_config: &NetworkConfig,
    accounting: Option<(
        crate::postgres_control_plane::PostgresControlPlane,
        crate::postgres_download_accounting::DownloadAccountingContext,
    )>,
    scheduler: Option<SchedulerStreamContext>,
) -> Result<HttpResponse, HttpResponse> {
    match locator.transport_mode.as_str() {
        "bot" => {
            bot_download_asset_locator(
                req,
                locator,
                config,
                force_attachment,
                accounting,
                scheduler,
            )
            .await
        }
        "user" => {
            download_message_stream_for_mode(
                req,
                locator.message_id,
                locator.legacy_folder_id,
                tg_state,
                force_attachment,
                config,
                db,
                transport,
                net_config,
                TelegramTransportMode::User,
                accounting,
                scheduler,
            )
            .await
        }
        _ => Err(HttpResponse::Conflict().body("Unsupported asset transport mode")),
    }
}

async fn download_message_stream_for_mode(
    req: &HttpRequest,
    message_id: i32,
    folder_id: Option<i64>,
    tg_state: &TelegramState,
    force_attachment: bool,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    net_config: &NetworkConfig,
    mode: TelegramTransportMode,
    accounting: Option<(
        crate::postgres_control_plane::PostgresControlPlane,
        crate::postgres_download_accounting::DownloadAccountingContext,
    )>,
    scheduler: Option<SchedulerStreamContext>,
) -> Result<HttpResponse, HttpResponse> {
    if mode == TelegramTransportMode::Bot {
        return bot_download_message(req, message_id, config, db, force_attachment).await;
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = client_opt.ok_or_else(|| {
        HttpResponse::ServiceUnavailable().body("Telegram client is not connected")
    })?;

    let peer = crate::commands::utils::resolve_peer_with_limit(
        &client,
        folder_id,
        &tg_state.peer_cache,
        net_config.peer_cache_size(),
    )
    .await
    .map_err(|e| HttpResponse::BadRequest().body(e))?;

    let messages = client
        .get_messages_by_id(peer.clone(), &[message_id])
        .await
        .map_err(|e| HttpResponse::InternalServerError().body(format!("{e}")))?;

    let Some(Some(msg)) = messages.first() else {
        return Err(HttpResponse::NotFound().body("File not found"));
    };

    let Some(media) = msg.media() else {
        return Err(HttpResponse::NotFound().body("No media on message"));
    };

    let (size, mime, filename) = media_meta(&media);

    let mut start_byte = 0u64;
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

    let client_for_stream = client.clone();
    let max_chunk = net_config.download_chunk_i32();
    let dl_limit = net_config.download_limit_bytes_per_sec();
    let body_stream = build_media_stream(
        client_for_stream,
        media,
        start_byte,
        content_length,
        bytes_to_skip(start_byte),
        max_chunk,
        dl_limit,
    );
    let body_stream = account_download_stream(body_stream, accounting, scheduler);

    let disposition = if force_attachment || !is_previewable(&mime) {
        build_content_disposition("attachment", &filename)
    } else {
        build_content_disposition("inline", &filename)
    };

    if is_range {
        Ok(HttpResponse::PartialContent()
            .insert_header(("Content-Type", mime))
            .insert_header((
                "Content-Range",
                format!("bytes {}-{}/{}", start_byte, end_byte, size),
            ))
            .insert_header(("Content-Length", content_length.to_string()))
            .insert_header(("Content-Disposition", disposition))
            .insert_header(("Accept-Ranges", "bytes"))
            .streaming(body_stream))
    } else {
        Ok(HttpResponse::Ok()
            .insert_header(("Content-Type", mime))
            .insert_header(("Content-Length", size.to_string()))
            .insert_header(("Content-Disposition", disposition))
            .insert_header(("Accept-Ranges", "bytes"))
            .streaming(body_stream))
    }
}

async fn bot_download_message(
    req: &HttpRequest,
    message_id: i32,
    config: &ServerConfig,
    db: &DbConnection,
    force_attachment: bool,
) -> Result<HttpResponse, HttpResponse> {
    if let Some(record) = crate::db::get_bot_file_map(db, message_id)
        .map_err(|e| HttpResponse::InternalServerError().body(e))?
    {
        if record.file_size > crate::download_degradation::BOT_API_DOWNLOAD_MAX_BYTES {
            return Ok(
                crate::download_degradation::build_bot_download_limit_response(
                    req,
                    config,
                    &record.file_name,
                    record.file_size,
                ),
            );
        }
    }

    let range_header = req
        .headers()
        .get(actix_web::http::header::RANGE)
        .and_then(|v| v.to_str().ok());

    let (resp, filename, size) =
        telegram_transport::bot_download_stream(config, db, message_id, range_header)
            .await
            .map_err(|e| HttpResponse::NotFound().body(e))?;

    let mime = mime_guess(&filename);
    let disposition = if force_attachment || !is_previewable(&mime) {
        build_content_disposition("attachment", &filename)
    } else {
        build_content_disposition("inline", &filename)
    };

    let status = resp.status();
    let is_partial = status.as_u16() == 206;
    let content_length = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(size);

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
    Ok(builder
        .insert_header(("Content-Type", mime))
        .insert_header(("Content-Length", content_length.to_string()))
        .insert_header(("Content-Disposition", disposition))
        .insert_header(("Accept-Ranges", "bytes"))
        .streaming(byte_stream))
}

async fn bot_download_asset_locator(
    req: &HttpRequest,
    locator: &crate::asset_locator::AssetLocatorRecord,
    config: &ServerConfig,
    force_attachment: bool,
    accounting: Option<(
        crate::postgres_control_plane::PostgresControlPlane,
        crate::postgres_download_accounting::DownloadAccountingContext,
    )>,
    scheduler: Option<SchedulerStreamContext>,
) -> Result<HttpResponse, HttpResponse> {
    let telegram_file_id = locator.telegram_file_id.as_deref().ok_or_else(|| {
        HttpResponse::Conflict().body("Bot asset locator is missing telegram_file_id")
    })?;
    if locator.file_size.max(0) as u64 > crate::download_degradation::BOT_API_DOWNLOAD_MAX_BYTES {
        return Ok(
            crate::download_degradation::build_bot_download_limit_response(
                req,
                config,
                &locator.file_name,
                locator.file_size.max(0) as u64,
            ),
        );
    }
    let range_header = req
        .headers()
        .get(actix_web::http::header::RANGE)
        .and_then(|value| value.to_str().ok());
    let (response, filename, size) = telegram_transport::bot_download_stream_for_locator(
        config,
        telegram_file_id,
        &locator.file_name,
        locator.file_size.max(0) as u64,
        locator.bot_pool_index,
        locator.uploader_bot_id.as_deref(),
        range_header,
    )
    .await
    .map_err(|error| HttpResponse::NotFound().body(error))?;
    let mime = mime_guess(&filename);
    let disposition = if force_attachment || !is_previewable(&mime) {
        build_content_disposition("attachment", &filename)
    } else {
        build_content_disposition("inline", &filename)
    };
    let status = response.status();
    let is_partial = status.as_u16() == 206;
    let content_length = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(size);
    let content_range = response
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let byte_stream = response.bytes_stream().map(|chunk| {
        chunk
            .map(actix_web::web::Bytes::from)
            .map_err(actix_web::error::ErrorInternalServerError)
    });
    let byte_stream = account_download_stream(byte_stream, accounting, scheduler);
    let mut builder = if is_partial {
        HttpResponse::PartialContent()
    } else {
        HttpResponse::Ok()
    };
    if let Some(range) = content_range {
        builder.insert_header(("Content-Range", range));
    }
    Ok(builder
        .insert_header(("Content-Type", mime))
        .insert_header(("Content-Length", content_length.to_string()))
        .insert_header(("Content-Disposition", disposition))
        .insert_header(("Accept-Ranges", "bytes"))
        .streaming(byte_stream))
}

fn account_download_stream<S>(
    inner: S,
    accounting: Option<(
        crate::postgres_control_plane::PostgresControlPlane,
        crate::postgres_download_accounting::DownloadAccountingContext,
    )>,
    scheduler: Option<SchedulerStreamContext>,
) -> impl futures_util::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>>
where
    S: futures_util::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>> + 'static,
{
    stream! {
        futures_util::pin_mut!(inner);
        let mut sequence = 0u64;
        while let Some(item) = inner.next().await {
            match item {
                Ok(bytes) => {
                    if let Some(sc) = scheduler.as_ref() {
                        if let Err(error) = sc.guard.ensure_owned() {
                            if let Some((control_plane, context)) = accounting.as_ref() {
                                let _ = control_plane
                                    .finish_download(context, Some("DOWNLOAD_SCHEDULER_LOST"))
                                    .await;
                            }
                            let _ = sc
                                .guard
                                .finish(
                                    crate::durable_scheduler::SchedulerOutcome::Retry {
                                        after_seconds: 5,
                                    },
                                    Some("DOWNLOAD_SCHEDULER_LOST"),
                                    Some(&error),
                                )
                                .await;
                            yield Err(actix_web::error::ErrorServiceUnavailable(error));
                            return;
                        }
                    }
                    sequence += 1;
                    if let Some((control_plane, context)) = accounting.as_ref() {
                        if let Err(error) = control_plane.checkpoint_download(context, sequence, bytes.len()).await {
                            let _ = control_plane.finish_download(context, Some("DOWNLOAD_LEDGER_FAILED")).await;
                            if let Some(sc) = scheduler.as_ref() {
                                let _ = sc
                                    .guard
                                    .finish(
                                        crate::durable_scheduler::SchedulerOutcome::Retry {
                                            after_seconds: 5,
                                        },
                                        Some("DOWNLOAD_LEDGER_FAILED"),
                                        Some(&error.to_string()),
                                    )
                                    .await;
                            }
                            yield Err(actix_web::error::ErrorServiceUnavailable(error));
                            return;
                        }
                    }
                    yield Ok(bytes);
                }
                Err(error) => {
                    if let Some((control_plane, context)) = accounting.as_ref() {
                        let _ = control_plane.finish_download(context, Some("DOWNLOAD_UPSTREAM_FAILED")).await;
                        if let Some(sc) = scheduler.as_ref() {
                            let _ = sc
                                .guard
                                .finish(
                                    crate::durable_scheduler::SchedulerOutcome::Retry {
                                        after_seconds: 5,
                                    },
                                    Some("DOWNLOAD_UPSTREAM_FAILED"),
                                    Some(&error.to_string()),
                                )
                                .await;
                        }
                    }
                    yield Err(error);
                    return;
                }
            }
        }
        if let Some((control_plane, context)) = accounting.as_ref() {
            if let Err(error) = control_plane.finish_download(context, None).await {
                log::error!("download accounting finalization failed: {error}");
            }
        }
        if let Some(sc) = scheduler.as_ref() {
            let _ = sc
                .guard
                .finish(crate::durable_scheduler::SchedulerOutcome::Success, None, None)
                .await;
        }
    }
}
fn media_meta(media: &Media) -> (u64, String, String) {
    match media {
        Media::Document(d) => (
            d.size() as u64,
            d.mime_type()
                .unwrap_or("application/octet-stream")
                .to_string(),
            d.name().to_string(),
        ),
        Media::Photo(_) => (0, "image/jpeg".into(), "Photo.jpg".into()),
        _ => (0, "application/octet-stream".into(), "download".into()),
    }
}

fn bytes_to_skip(start_byte: u64) -> usize {
    if start_byte == 0 {
        return 0;
    }
    const MIN_CHUNK: u64 = 4096;
    let chunk_index = start_byte / MIN_CHUNK;
    (start_byte - chunk_index * MIN_CHUNK) as usize
}

fn build_media_stream(
    client: grammers_client::Client,
    media: Media,
    start_byte: u64,
    content_length: u64,
    bytes_to_skip: usize,
    max_chunk_size: i32,
    download_limit_bps: u64,
) -> impl futures_util::Stream<Item = Result<actix_web::web::Bytes, actix_web::Error>> {
    stream! {
        let mut download_iter = client.iter_download(&media).chunk_size(max_chunk_size);
        if start_byte > 0 {
            const MIN_CHUNK_SIZE: i32 = 4096;
            let chunk_index = (start_byte / MIN_CHUNK_SIZE as u64) as i32;
            download_iter = download_iter
                .chunk_size(MIN_CHUNK_SIZE)
                .skip_chunks(chunk_index)
                .chunk_size(max_chunk_size);
        }

        let mut skipped = 0usize;
        let mut total_yielded = 0u64;
        let mut window_bytes = 0u64;
        let mut window_start = std::time::Instant::now();

        while let Some(chunk) = download_iter.next().await.transpose() {
            match chunk {
                Ok(data) => {
                    let mut data_slice = data;
                    if skipped < bytes_to_skip {
                        let to_skip = bytes_to_skip - skipped;
                        if data_slice.len() <= to_skip {
                            skipped += data_slice.len();
                            continue;
                        }
                        data_slice = data_slice[to_skip..].to_vec();
                        skipped = bytes_to_skip;
                    }
                    if total_yielded + data_slice.len() as u64 > content_length {
                        let allowed = (content_length - total_yielded) as usize;
                        if allowed > 0 {
                            let slice = data_slice[..allowed].to_vec();
                            vpn_optimizer::throttle_transfer_bytes(
                                slice.len() as u64,
                                download_limit_bps,
                                &mut window_bytes,
                                &mut window_start,
                            ).await;
                            yield Ok(actix_web::web::Bytes::from(slice));
                        }
                        break;
                    }
                    let len = data_slice.len() as u64;
                    vpn_optimizer::throttle_transfer_bytes(
                        len,
                        download_limit_bps,
                        &mut window_bytes,
                        &mut window_start,
                    ).await;
                    yield Ok(actix_web::web::Bytes::from(data_slice));
                    total_yielded += len;
                    if total_yielded >= content_length {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("download stream error: {e}");
                    break;
                }
            }
        }
    }
}

pub async fn download_manifest_stream(
    _req: &HttpRequest,
    manifest_message_id: i32,
    folder_id: Option<i64>,
    tg_state: &TelegramState,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    net_config: &NetworkConfig,
) -> Result<HttpResponse, actix_web::HttpResponse> {
    if transport.effective_mode(config).await == TelegramTransportMode::Bot {
        return bot_download_manifest(manifest_message_id, config, db).await;
    }

    let client_opt = { tg_state.client.lock().await.clone() };
    let client = client_opt.ok_or_else(|| {
        HttpResponse::ServiceUnavailable().body("Telegram client is not connected")
    })?;

    let peer = crate::commands::utils::resolve_peer_with_limit(
        &client,
        folder_id,
        &tg_state.peer_cache,
        net_config.peer_cache_size(),
    )
    .await
    .map_err(|e| HttpResponse::BadRequest().body(e))?;

    let messages = client
        .get_messages_by_id(peer.clone(), &[manifest_message_id])
        .await
        .map_err(|e| HttpResponse::InternalServerError().body(format!("{e}")))?;

    let Some(Some(msg)) = messages.first() else {
        return Err(HttpResponse::NotFound().body("Manifest not found"));
    };

    let Some(media) = msg.media() else {
        return Err(HttpResponse::NotFound().body("Manifest has no file"));
    };

    let mut buf = Vec::new();
    let mut iter = client.iter_download(&media);
    const MAX_MANIFEST_BYTES: usize = 1024 * 1024; // 1 MB
    while let Some(chunk) = iter.next().await.transpose() {
        match chunk {
            Ok(data) => {
                buf.extend_from_slice(&data);
                if buf.len() > MAX_MANIFEST_BYTES {
                    return Err(HttpResponse::PayloadTooLarge().body("manifest exceeds 1 MB limit"));
                }
            }
            Err(e) => {
                log::error!("Manifest download failed: {}", e);
                return Err(HttpResponse::InternalServerError().body("manifest download failed"));
            }
        }
    }

    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        return Err(HttpResponse::BadRequest().body("fileAll.txt format invalid"));
    }

    let orig_filename = lines[0].clone();
    let chunk_ids: Vec<i32> = lines[1..]
        .iter()
        .filter_map(|s| s.parse::<i32>().ok())
        .collect();

    if chunk_ids.is_empty() {
        return Err(HttpResponse::BadRequest().body("no chunk ids in manifest"));
    }

    let ext_mime = mime_guess(&orig_filename);
    let disposition = if is_previewable(&ext_mime) {
        build_content_disposition("inline", &orig_filename)
    } else {
        build_content_disposition("attachment", &orig_filename)
    };

    let max_chunk = net_config.download_chunk_i32();
    let dl_limit = net_config.download_limit_bytes_per_sec();
    let stream = async_stream::stream! {
        let mut window_bytes = 0u64;
        let mut window_start = std::time::Instant::now();
        for (idx, mid) in chunk_ids.iter().enumerate() {
            let msgs = match client.get_messages_by_id(peer.clone(), &[*mid]).await {
                Ok(m) => m,
                Err(e) => {
                    log::error!("chunk {idx} fetch failed: {e}");
                    break;
                }
            };
            let Some(Some(chunk_msg)) = msgs.first() else { continue; };
            let Some(chunk_media) = chunk_msg.media() else { continue; };
            let mut iter = client.iter_download(&chunk_media).chunk_size(max_chunk);
            while let Some(chunk) = iter.next().await.transpose() {
                match chunk {
                    Ok(data) => {
                        let len = data.len() as u64;
                        vpn_optimizer::throttle_transfer_bytes(
                            len,
                            dl_limit,
                            &mut window_bytes,
                            &mut window_start,
                        ).await;
                        yield Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(data));
                    }
                    Err(e) => {
                        log::error!("chunk download error: {e}");
                        break;
                    }
                }
            }
        }
    };

    // Manifest stream is sequential chunk concatenation — does NOT support random access.
    // Do NOT claim Accept-Ranges: bytes; browsers would attempt seeking and fail.
    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", ext_mime))
        .insert_header(("Content-Disposition", disposition))
        .insert_header(("X-Accel-Buffering", "no"))
        .insert_header(("Cache-Control", "private, no-store"))
        .insert_header(("Transfer-Encoding", "chunked"))
        .streaming(stream))
}

async fn bot_download_manifest(
    manifest_message_id: i32,
    config: &ServerConfig,
    db: &DbConnection,
) -> Result<HttpResponse, HttpResponse> {
    let buf = telegram_transport::bot_download_manifest_bytes(config, db, manifest_message_id)
        .await
        .map_err(|e| HttpResponse::NotFound().body(e))?;

    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.len() < 2 {
        return Err(HttpResponse::BadRequest().body("fileAll.txt format invalid"));
    }

    let orig_filename = lines[0].clone();
    let chunk_ids: Vec<i32> = lines[1..]
        .iter()
        .filter_map(|s| s.parse::<i32>().ok())
        .collect();

    if chunk_ids.is_empty() {
        return Err(HttpResponse::BadRequest().body("no chunk ids in manifest"));
    }

    let ext_mime = mime_guess(&orig_filename);
    let disposition = if is_previewable(&ext_mime) {
        build_content_disposition("inline", &orig_filename)
    } else {
        build_content_disposition("attachment", &orig_filename)
    };

    let config = config.clone();
    let db = db.clone();
    let stream = async_stream::stream! {
        for (idx, mid) in chunk_ids.iter().enumerate() {
            let downloaded = telegram_transport::bot_download_stream(&config, &db, *mid, None).await;
            let Ok((resp, _, _)) = downloaded else {
                log::error!("bot chunk {idx} download failed");
                break;
            };
            let mut byte_stream = resp.bytes_stream();
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(data) => yield Ok::<_, actix_web::Error>(actix_web::web::Bytes::from(data)),
                    Err(e) => {
                        log::error!("bot chunk stream error: {e}");
                        break;
                    }
                }
            }
        }
    };

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", ext_mime))
        .insert_header(("Content-Disposition", disposition))
        .insert_header(("X-Accel-Buffering", "no"))
        .insert_header(("Cache-Control", "private, no-store"))
        .insert_header(("Transfer-Encoding", "chunked"))
        .streaming(stream))
}

fn mime_guess(filename: &str) -> String {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg".into(),
        "png" => "image/png".into(),
        "gif" => "image/gif".into(),
        "mp4" => "video/mp4".into(),
        "mp3" => "audio/mpeg".into(),
        "pdf" => "application/pdf".into(),
        _ => "application/octet-stream".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_escapes_quotes_and_backslash() {
        assert_eq!(
            sanitize_filename_for_header(r#"my\"file.pdf"#),
            r#"my\\\"file.pdf"#
        );
    }

    #[test]
    fn sanitize_filename_strips_control_chars() {
        assert_eq!(
            sanitize_filename_for_header("hello\x00\nworld.txt"),
            "helloworld.txt"
        );
    }

    #[test]
    fn rfc5987_encode_keeps_unreserved() {
        assert_eq!(rfc5987_encode("hello-world_2.0.txt"), "hello-world_2.0.txt");
    }

    #[test]
    fn rfc5987_encode_escapes_unicode() {
        assert_eq!(rfc5987_encode("中文.pdf"), "%E4%B8%AD%E6%96%87.pdf");
    }

    #[test]
    fn build_content_disposition_includes_legacy_and_rfc5987() {
        let cd = build_content_disposition("attachment", "report 中文.pdf");
        assert!(cd.starts_with("attachment; "));
        assert!(cd.contains("filename=\"report .pdf\""));
        assert!(cd.contains("filename*=UTF-8''report%20%E4%B8%AD%E6%96%87.pdf"));
    }

    #[test]
    fn build_content_disposition_prevents_header_injection() {
        let cd = build_content_disposition("attachment", "evil\r\nX-Inject: yes\".txt");
        // Legacy name should have CRLF stripped, quotes escaped, and non-letters removed.
        assert!(!cd.contains("\r\n"));
        assert!(!cd.contains("X-Inject:"));
        assert!(cd.contains("filename=\"evil"));
    }
}
