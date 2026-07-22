use grammers_client::InputMessage;
use std::sync::Arc;

use crate::commands::utils::{map_error, resolve_peer_with_limit};
use crate::commands::TelegramState;
use crate::db::DbConnection;
use crate::server_config::ServerConfig;
use crate::telegram_error::{classify_telegram_error_message, TelegramErrorClass};
use crate::telegram_transport::{self, TelegramTransportMode, TransportHandle};
use crate::vpn_optimizer::{backoff_ms, NetworkConfig, ThrottledReader};

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

pub async fn upload_file_path(
    path: String,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
) -> Result<(i32, String), String> {
    let mode = transport.effective_mode(config).await;
    if mode == TelegramTransportMode::Bot {
        let uploaded = telegram_transport::bot_upload_file_path(config, db, &path, None).await?;
        crate::metadata_cache::invalidate_files(db, folder_id);
        return Ok((uploaded.message_id, uploaded.file_name));
    }

    let client_opt = { state.client.lock().await.clone() };
    let client = client_opt.ok_or_else(|| "Telegram client is not connected".to_string())?;

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| e.to_string())?;
    let file_size = meta.len() as usize;
    let file_name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());

    let mut file = tokio::fs::File::open(&path)
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
    let _ = crate::file_access::record_uploaded_file(
        db,
        message_id,
        folder_id,
        crate::tenant_auth::OWNER_WEB,
        &file_name,
        file_size as i64,
    );
    crate::metadata_cache::invalidate_files(db, folder_id);
    Ok((message_id, file_name))
}

pub async fn upload_bytes_with_caption(
    data: Vec<u8>,
    upload_name: &str,
    caption: &str,
    folder_id: Option<i64>,
    state: &TelegramState,
    net_config: &Arc<NetworkConfig>,
    config: &ServerConfig,
    db: &DbConnection,
    transport: &TransportHandle,
) -> Result<i32, String> {
    let mode = transport.effective_mode(config).await;
    if mode == TelegramTransportMode::Bot {
        let uploaded = telegram_transport::bot_upload_bytes_with_pool(
            config,
            db,
            &data,
            upload_name,
            Some(caption),
            &state.bot_pool,
        )
        .await?;
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
    if let Ok(message_id) = result {
        let _ = crate::file_access::record_uploaded_file(
            db,
            message_id,
            folder_id,
            crate::tenant_auth::OWNER_WEB,
            upload_name,
            data.len() as i64,
        );
        crate::metadata_cache::invalidate_files(db, folder_id);
    }
    result
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

    let mut file = tokio::fs::File::open(&path)
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
        state,
        net_config,
        config,
        db,
        transport,
    )
    .await
}
