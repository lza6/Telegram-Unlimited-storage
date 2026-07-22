use grammers_client::{types::Peer, InputMessage};
use std::path::Path;
use std::sync::Arc;

use crate::commands::utils::{map_error, resolve_peer_with_limit};
use crate::commands::TelegramState;
use crate::db::DbConnection;
use crate::server_config::ServerConfig;
use crate::telegram_error::{classify_telegram_error_message, TelegramErrorClass};
use crate::telegram_transport::{self, TelegramTransportMode, TransportHandle};
use crate::vpn_optimizer::{backoff_ms, NetworkConfig, ThrottledReader};

/// Sanitize a client-provided original filename for storage/download.
/// Keeps basename only, strips path separators and control chars.
pub fn sanitize_upload_filename(raw: &str) -> String {
    let trimmed = raw.trim();
    let base = Path::new(trimmed)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| trimmed.to_string());
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    if cleaned.is_empty() {
        "file.bin".to_string()
    } else {
        cleaned.to_string()
    }
}

/// Best-effort MIME guess from file extension for list/download UX.
pub fn guess_mime_from_name(name: &str) -> Option<String> {
    let ext = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())?;
    let mime = match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" | "oga" => "audio/ogg",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "json" => "application/json",
        "txt" | "log" | "md" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "text/javascript",
        "csv" => "text/csv",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => return None,
    };
    Some(mime.to_string())
}

fn resolve_upload_display_name(path: &str, display_name: Option<&str>) -> String {
    if let Some(name) = display_name.map(str::trim).filter(|s| !s.is_empty()) {
        return sanitize_upload_filename(name);
    }
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|s| {
            !s.is_empty()
                && !s.starts_with("td-upload-")
                && !s.starts_with("td-api-")
                && !s.starts_with("td-wd-")
        })
        .map(|s| sanitize_upload_filename(&s))
        .unwrap_or_else(|| "file.bin".to_string())
}

async fn send_message_with_retry(
    client: &grammers_client::Client,
    peer: &grammers_client::types::Peer,
    message: InputMessage,
    net_config: &Arc<NetworkConfig>,
) -> Result<i32, String> {
    let max_retries = net_config.retry_attempts();
    let base_ms = net_config.retry_base_backoff_ms();
    let max_ms = net_config.retry_max_backoff_ms();
    let respect_flood = net_config.should_respect_flood_wait();
    let mut last_err = String::new();

    for attempt in 0..=max_retries {
        match client.send_message(peer, message.clone()).await {
            Ok(sent) => return Ok(sent.id()),
            Err(e) => {
                let err = map_error(e);
                if respect_flood {
                    if let Some(secs) = crate::vpn_optimizer::parse_flood_wait_secs(&err) {
                        tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                        last_err = err;
                        continue;
                    }
                }
                if classify_telegram_error_message(&err) == TelegramErrorClass::Fatal {
                    return Err(format!("Upload failed: {err}"));
                }
                last_err = err;
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms(
                        attempt, base_ms, max_ms,
                    )))
                    .await;
                }
            }
        }
    }
    Err(format!("Upload failed: {last_err}"))
}

/// Bot-mode upload with bounded retry. Mirrors the User-mode `send_message_with_retry`
/// policy (H2): transient network errors are retried with exponential backoff,
/// and FloodWait is respected (sleep then retry on the next available bot) up to
/// the retry budget. When retries are exhausted on a FloodWait, the error is
/// returned with a `FLOOD_WAIT:` prefix so the HTTP layer can map it to 503 +
/// `Retry-After` (H3) instead of a bare 500.
async fn bot_upload_with_retry(
    config: &ServerConfig,
    db: &DbConnection,
    data: &[u8],
    upload_name: &str,
    caption: Option<&str>,
    net_config: &Arc<NetworkConfig>,
    bot_pool: &crate::bot_pool::BotPool,
    selected: Option<&crate::telegram_transport::PreselectedBot>,
) -> Result<crate::telegram_transport::BotUploadResult, String> {
    let max_retries = net_config.retry_attempts();
    let base_ms = net_config.retry_base_backoff_ms();
    let max_ms = net_config.retry_max_backoff_ms();
    let respect_flood = net_config.should_respect_flood_wait();
    let mut last_err = String::new();

    for attempt in 0..=max_retries {
        match telegram_transport::bot_upload_bytes_with_pool_and_selection(
            config,
            db,
            data,
            upload_name,
            caption,
            bot_pool,
            selected,
        )
        .await
        {
            Ok(res) => return Ok(res),
            Err(e) => {
                // FloodWait: optionally respect it and retry; `acquire_bot_token`
                // already skips flooded bots, so a retry naturally lands on a
                // different (non-flooded) bot.
                if let Some(secs) = parse_bot_flood_wait_secs(&e) {
                    last_err = e;
                    if respect_flood {
                        let cap = secs.min(30) as u64; // bound the sleep per attempt
                        tokio::time::sleep(std::time::Duration::from_secs(cap)).await;
                        continue;
                    }
                    // Not respecting flood wait: surface for HTTP 503 mapping.
                    return Err(last_err);
                }
                last_err = e;
                if attempt < max_retries {
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms(
                        attempt, base_ms, max_ms,
                    )))
                    .await;
                }
            }
        }
    }
    Err(last_err)
}

/// Parse the `FLOOD_WAIT:{index}:{secs}` marker emitted by the bot transport.
fn parse_bot_flood_wait_secs(err: &str) -> Option<u64> {
    err.strip_prefix("FLOOD_WAIT:")?
        .rsplit(':')
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> DbConnection {
        let dir =
            std::env::temp_dir().join(format!("td-http-upload-test-{}", uuid::Uuid::new_v4()));
        crate::db::init_db_at(&dir).expect("db")
    }

    #[test]
    fn bot_receipt_projection_is_explicit_and_idempotent() {
        let db = temp_db();
        let receipt = crate::telegram_transport::TelegramUploadReceipt {
            message_id: 501,
            telegram_file_id: Some("tg-501".to_string()),
            file_name: "receipt.bin".to_string(),
            file_size: 64,
            mime_type: "application/octet-stream".to_string(),
            storage_peer_id: -1_000_000_000_501,
            storage_peer_kind: "channel".to_string(),
            bot_pool_index: Some(1),
            uploader_bot_id: Some("bot-stable".to_string()),
        };
        persist_uploaded_receipt_projection(&db, &receipt, None, "tenant:a").expect("first");
        persist_uploaded_receipt_projection(&db, &receipt, None, "tenant:a").expect("replay");
        let mapping = crate::db::get_bot_file_map(&db, 501)
            .expect("mapping query")
            .expect("mapping");
        assert_eq!(mapping.telegram_file_id, "tg-501");
        assert_eq!(mapping.bot_pool_index, 1);
        let asset = crate::db::get_file_asset(&db, 501)
            .expect("asset query")
            .expect("asset");
        assert_eq!(asset.owner_id, "tenant:a");
    }

    #[test]
    fn bot_receipt_projection_fails_closed_without_uploader_index() {
        let db = temp_db();
        let receipt = crate::telegram_transport::TelegramUploadReceipt {
            message_id: 502,
            telegram_file_id: Some("tg-502".to_string()),
            file_name: "receipt.bin".to_string(),
            file_size: 64,
            mime_type: "application/octet-stream".to_string(),
            storage_peer_id: -1_000_000_000_502,
            storage_peer_kind: "channel".to_string(),
            bot_pool_index: None,
            uploader_bot_id: Some("bot-stable".to_string()),
        };
        assert!(
            persist_uploaded_receipt_projection(&db, &receipt, None, "tenant:a")
                .unwrap_err()
                .contains("bot_pool_index")
        );
        assert!(crate::db::get_file_asset(&db, 502)
            .expect("asset query")
            .is_none());
    }

    #[test]
    fn parses_bot_flood_wait_marker() {
        assert_eq!(parse_bot_flood_wait_secs("FLOOD_WAIT:0:42"), Some(42));
        assert_eq!(parse_bot_flood_wait_secs("FLOOD_WAIT:3:7"), Some(7));
        assert_eq!(parse_bot_flood_wait_secs("FLOOD_WAIT:1:0"), Some(0));
        assert_eq!(parse_bot_flood_wait_secs("some other error"), None);
        assert_eq!(parse_bot_flood_wait_secs(""), None);
    }

    #[test]
    fn sanitize_keeps_extension_and_strips_path() {
        assert_eq!(sanitize_upload_filename(r"C:\Users\a\demo.mp4"), "demo.mp4");
        assert_eq!(sanitize_upload_filename("../evil/name.png"), "name.png");
        assert_eq!(sanitize_upload_filename("  clip.webm  "), "clip.webm");
    }

    #[test]
    fn sanitize_rejects_empty_and_controls() {
        assert_eq!(sanitize_upload_filename("..."), "file.bin");
        assert_eq!(sanitize_upload_filename(""), "file.bin");
        assert_eq!(sanitize_upload_filename("a\nb.mp4"), "ab.mp4");
    }

    #[test]
    fn guess_mime_from_common_extensions() {
        assert_eq!(
            guess_mime_from_name("movie.mp4").as_deref(),
            Some("video/mp4")
        );
        assert_eq!(
            guess_mime_from_name("photo.JPEG").as_deref(),
            Some("image/jpeg")
        );
        assert_eq!(guess_mime_from_name("noext"), None);
    }

    #[test]
    fn resolve_prefers_display_name_over_temp_path() {
        assert_eq!(
            resolve_upload_display_name(
                "/tmp/td-upload-5da3b9dd-b2fb-42d4-a263-7d016abfc2a8",
                Some("demo.mp4")
            ),
            "demo.mp4"
        );
        assert_eq!(
            resolve_upload_display_name(
                "/tmp/td-upload-5da3b9dd-b2fb-42d4-a263-7d016abfc2a8",
                None
            ),
            "file.bin"
        );
    }
}

pub async fn upload_file_path_with_receipt(
    path: String,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    _owner_id: &str,
    selected_bot: Option<&crate::telegram_transport::PreselectedBot>,
    display_name: Option<String>,
) -> Result<crate::telegram_transport::TelegramUploadReceipt, String> {
    let file_name = resolve_upload_display_name(&path, display_name.as_deref());
    let mime_type =
        guess_mime_from_name(&file_name).unwrap_or_else(|| "application/octet-stream".to_string());
    let mode = transport.effective_mode(config).await;
    if mode == TelegramTransportMode::Bot {
        // Use the shared bot pool + retry path (H2) instead of a throwaway
        // per-call pool, so FloodWait state is remembered across requests.
        let data = tokio::fs::read(&path)
            .await
            .map_err(|e| format!("read upload file: {e}"))?;
        let uploaded = bot_upload_with_retry(
            config,
            db,
            &data,
            &file_name,
            None,
            net_config,
            &state.bot_pool,
            selected_bot,
        )
        .await?;
        let mut receipt = crate::telegram_transport::TelegramUploadReceipt::from(&uploaded);
        // Prefer client-provided original name over temp path basename.
        receipt.file_name = file_name;
        if receipt.mime_type.is_empty() || receipt.mime_type == "application/octet-stream" {
            receipt.mime_type = mime_type;
        }
        return Ok(receipt);
    }

    let client_opt = { state.client.lock().await.clone() };
    let client = client_opt.ok_or_else(|| "Telegram client is not connected".to_string())?;

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| e.to_string())?;
    let file_size = meta.len() as usize;

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| e.to_string())?;
    let upload_limit = net_config.upload_limit_bytes_per_sec();
    let mut throttled = ThrottledReader::new(file, upload_limit);
    let uploaded_file = client
        .upload_stream(&mut throttled, file_size, file_name.clone())
        .await
        .map_err(map_error)?;

    let peer = resolve_peer_with_limit(
        &client,
        folder_id,
        &state.peer_cache,
        net_config.peer_cache_size(),
    )
    .await?;
    let message = InputMessage::new().text("").file(uploaded_file);
    let message_id = send_message_with_retry(&client, &peer, message, net_config).await?;
    let storage_peer_id = peer.id().bot_api_dialog_id();
    let storage_peer_kind = match &peer {
        Peer::User(_) => "user",
        Peer::Group(_) => "group",
        Peer::Channel(_) => "channel",
    };
    Ok(crate::telegram_transport::TelegramUploadReceipt {
        message_id,
        telegram_file_id: None,
        file_name,
        file_size: file_size as u64,
        mime_type,
        storage_peer_id,
        storage_peer_kind: storage_peer_kind.to_string(),
        bot_pool_index: None,
        uploader_bot_id: None,
    })
}

pub fn persist_uploaded_receipt_projection(
    db: &DbConnection,
    receipt: &crate::telegram_transport::TelegramUploadReceipt,
    folder_id: Option<i64>,
    owner_id: &str,
) -> Result<(), String> {
    let transport_mode = if receipt.telegram_file_id.is_some() {
        "bot"
    } else {
        "user"
    };
    crate::asset_locator::upsert_from_receipt(db, receipt, transport_mode, folder_id, owner_id)?;
    if let Some(telegram_file_id) = receipt.telegram_file_id.as_deref() {
        let bot_pool_index = receipt
            .bot_pool_index
            .ok_or_else(|| "Bot upload receipt is missing bot_pool_index".to_string())?;
        crate::db::upsert_bot_file_map(
            db,
            receipt.message_id,
            telegram_file_id,
            &receipt.file_name,
            receipt.file_size,
            None,
            bot_pool_index,
        )?;
    }
    crate::file_access::record_uploaded_file(
        db,
        receipt.message_id,
        folder_id,
        owner_id,
        &receipt.file_name,
        receipt.file_size as i64,
    )?;
    crate::metadata_cache::invalidate_files(db, folder_id);
    Ok(())
}

/// Compatibility wrapper for existing routes that only need the catalog tuple.
pub async fn upload_file_path(
    path: String,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    owner_id: &str,
) -> Result<(i32, String), String> {
    upload_file_path_named(
        path, folder_id, state, net_config, config, db, transport, owner_id, None,
    )
    .await
}

/// Like [`upload_file_path`], but keeps the original client filename (extension).
pub async fn upload_file_path_named(
    path: String,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
    owner_id: &str,
    display_name: Option<String>,
) -> Result<(i32, String), String> {
    let receipt = upload_file_path_with_receipt(
        path,
        folder_id,
        state,
        net_config,
        config,
        db,
        transport,
        owner_id,
        None,
        display_name,
    )
    .await?;
    persist_uploaded_receipt_projection(db, &receipt, folder_id, owner_id)?;
    Ok((receipt.message_id, receipt.file_name))
}

pub async fn compensate_uploaded_receipt(
    receipt: &crate::telegram_transport::TelegramUploadReceipt,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    _transport: &TransportHandle,
    expected_mode: &str,
) -> Result<(), String> {
    match expected_mode {
        "bot" => {
            telegram_transport::bot_delete_message(
                config,
                db,
                receipt.storage_peer_id,
                &receipt.storage_peer_kind,
                receipt.message_id,
                receipt.telegram_file_id.as_deref(),
                receipt.uploader_bot_id.as_deref(),
            )
            .await?;
        }
        "user" => {
            let client = state
                .client
                .lock()
                .await
                .clone()
                .ok_or_else(|| "Telegram client is not connected".to_string())?;
            let peer = resolve_peer_with_limit(
                &client,
                folder_id,
                &state.peer_cache,
                net_config.peer_cache_size(),
            )
            .await?;
            if peer.id().bot_api_dialog_id() != receipt.storage_peer_id {
                return Err("Telegram compensation peer mismatch".to_string());
            }
            if let Err(error) = client.delete_messages(&peer, &[receipt.message_id]).await {
                let error = map_error(error);
                let normalized = error.to_ascii_lowercase();
                if !normalized.contains("message_id_invalid")
                    && !normalized.contains("message id invalid")
                    && !normalized.contains("message to delete not found")
                {
                    return Err(error);
                }
            }
        }
        other => {
            return Err(format!("Unsupported Telegram compensation mode: {other}"));
        }
    }
    crate::file_access::purge_file_index_entry_strict(db, receipt.message_id, None)?;
    Ok(())
}

pub async fn upload_bytes_with_caption(
    data: Vec<u8>,
    upload_name: &str,
    caption: &str,
    folder_id: Option<i64>,
    owner_id: &str,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
) -> Result<i32, String> {
    let mode = transport.effective_mode(config).await;
    if mode == TelegramTransportMode::Bot {
        let uploaded = bot_upload_with_retry(
            config,
            db,
            &data,
            upload_name,
            Some(caption),
            net_config,
            &state.bot_pool,
            None,
        )
        .await?;
        crate::file_access::record_uploaded_file(
            db,
            uploaded.message_id,
            folder_id,
            owner_id,
            &uploaded.file_name,
            uploaded.file_size as i64,
        )?;
        crate::metadata_cache::invalidate_files(db, folder_id);
        return Ok(uploaded.message_id);
    }

    let tmp = std::env::temp_dir().join(format!("td-chunk-{}", uuid::Uuid::new_v4()));
    tokio::fs::write(&tmp, &data)
        .await
        .map_err(|e| e.to_string())?;

    let result = upload_file_path_with_caption(
        tmp.to_string_lossy().to_string(),
        upload_name.to_string(),
        caption.to_string(),
        folder_id,
        state,
        net_config,
    )
    .await;

    let _ = tokio::fs::remove_file(&tmp).await;
    let message_id = result?;
    crate::file_access::record_uploaded_file(
        db,
        message_id,
        folder_id,
        owner_id,
        upload_name,
        data.len() as i64,
    )?;
    crate::metadata_cache::invalidate_files(db, folder_id);
    Ok(message_id)
}

async fn upload_file_path_with_caption(
    path: String,
    upload_name: String,
    caption: String,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
) -> Result<i32, String> {
    let client_opt = { state.client.lock().await.clone() };
    let client = client_opt.ok_or_else(|| "Telegram client is not connected".to_string())?;

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| e.to_string())?;
    let file_size = meta.len() as usize;

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| e.to_string())?;
    let upload_limit = net_config.upload_limit_bytes_per_sec();
    let mut throttled = ThrottledReader::new(file, upload_limit);
    let uploaded = client
        .upload_stream(&mut throttled, file_size, upload_name)
        .await
        .map_err(map_error)?;

    let peer = resolve_peer_with_limit(
        &client,
        folder_id,
        &state.peer_cache,
        net_config.peer_cache_size(),
    )
    .await?;
    let message = InputMessage::new().text(caption).file(uploaded);
    send_message_with_retry(&client, &peer, message, net_config).await
}

pub async fn upload_text_file(
    content: &str,
    upload_name: &str,
    caption: &str,
    folder_id: Option<i64>,
    owner_id: &str,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
) -> Result<i32, String> {
    upload_bytes_with_caption(
        content.as_bytes().to_vec(),
        upload_name,
        caption,
        folder_id,
        owner_id,
        state,
        net_config,
        config,
        db,
        transport,
    )
    .await
}
