use crate::bandwidth::BandwidthManager;
use crate::commands::utils::{map_error, resolve_peer, resolve_peer_with_limit};
use crate::models::{FileMetadata, FolderMetadata};
use crate::vpn_optimizer::{backoff_ms, throttle_transfer_bytes, NetworkConfig, ThrottledReader};
use crate::TelegramState;
use grammers_client::types::{Media, Peer};
use grammers_client::InputMessage;
use grammers_tl_types as tl;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
#[cfg(not(feature = "headless-server"))]
use tauri::Manager;
use tauri::{Emitter, State};
use tokio::sync::oneshot;

static UPLOAD_CANCELLATIONS: OnceLock<Mutex<HashMap<String, oneshot::Sender<()>>>> =
    OnceLock::new();

fn get_upload_cancellations() -> &'static Mutex<HashMap<String, oneshot::Sender<()>>> {
    UPLOAD_CANCELLATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_upload_cancellations<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut HashMap<String, oneshot::Sender<()>>) -> T,
{
    let mut guard = get_upload_cancellations()
        .lock()
        .map_err(|e| format!("upload cancellation lock poisoned: {e}"))?;
    Ok(f(&mut guard))
}

fn require_client(
    client_opt: Option<grammers_client::Client>,
) -> Result<grammers_client::Client, String> {
    client_opt.ok_or_else(|| "Telegram client is not connected".to_string())
}

async fn list_document_files_in_folder(
    client: &grammers_client::Client,
    folder_id: Option<i64>,
    peer_cache: &std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<i64, Peer>>>,
) -> Result<Vec<FileMetadata>, String> {
    let peer = resolve_peer(client, folder_id, peer_cache).await?;
    let mut files = Vec::new();

    let mut msgs = client.iter_messages(&peer);
    while let Some(msg) = msgs.next().await.map_err(|e| e.to_string())? {
        if let Some(doc) = msg.media() {
            let (name, size, mime, ext) = match doc {
                Media::Document(d) => {
                    let n = d.name().to_string();
                    let s = d.size();
                    let m = d.mime_type().map(|s| s.to_string());
                    let e = std::path::Path::new(&n)
                        .extension()
                        .map(|os| os.to_str().unwrap_or("").to_string());
                    (n, s, m, e)
                }
                Media::Photo(_) => (
                    "Photo.jpg".to_string(),
                    0,
                    Some("image/jpeg".into()),
                    Some("jpg".into()),
                ),
                _ => ("Unknown".to_string(), 0, None, None),
            };
            files.push(FileMetadata {
                id: msg.id() as i64,
                folder_id,
                name,
                size: size as u64,
                mime_type: mime,
                file_ext: ext,
                created_at: msg.date().to_string(),
                icon_type: "file".into(),
            });
        }
    }

    Ok(files)
}

#[tauri::command]
pub async fn cmd_create_folder(
    name: String,
    state: State<'_, TelegramState>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<FolderMetadata, String> {
    let client_opt = { state.client.lock().await.clone() };

    let client = require_client(client_opt)?;
    log::info!("Creating Telegram Channel: {}", name);

    let result = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .invoke(&tl::functions::channels::CreateChannel {
                    broadcast: true,
                    megagroup: false,
                    title: format!("{} [TD]", name),
                    about: "Telegram Drive Storage Folder\n[telegram-drive-folder]".to_string(),
                    geo_point: None,
                    address: None,
                    for_import: false,
                    forum: false,
                    ttl_period: None,
                })
                .await
                .map_err(|e| map_error(e))
        },
        "create_channel",
    )
    .await?;

    let (chat_id, access_hash) = match result {
        tl::enums::Updates::Updates(u) => {
            let chat = u.chats.first().ok_or("No chat in updates")?;
            match chat {
                tl::enums::Chat::Channel(c) => (c.id, c.access_hash.unwrap_or(0)),
                _ => return Err("Created chat is not a channel".to_string()),
            }
        }
        _ => return Err("Unexpected response (not Updates::Updates)".to_string()),
    };

    // Explicitly Disable TTL
    let _input_channel = tl::enums::InputChannel::Channel(tl::types::InputChannel {
        channel_id: chat_id,
        access_hash,
    });

    let _ = client
        .invoke(&tl::functions::messages::SetHistoryTtl {
            peer: tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id: chat_id,
                access_hash,
            }),
            period: 0,
        })
        .await;

    let _ = crate::db::set_file_index_complete(&db_pool, false);

    Ok(FolderMetadata {
        id: chat_id,
        name,
        parent_id: None,
    })
}

#[tauri::command]
pub async fn cmd_delete_folder(
    folder_id: i64,
    state: State<'_, TelegramState>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<bool, String> {
    let client_opt = { state.client.lock().await.clone() };

    let client = require_client(client_opt)?;
    log::info!("Deleting folder/channel: {}", folder_id);

    let peer = crate::vpn_optimizer::with_retry(
        &net_config,
        || async { resolve_peer(&client, Some(folder_id), &state.peer_cache).await },
        "resolve_peer(delete_folder)",
    )
    .await?;

    let input_channel = match peer {
        Peer::Channel(c) => {
            let chan = &c.raw;
            tl::enums::InputChannel::Channel(tl::types::InputChannel {
                channel_id: chan.id,
                access_hash: chan.access_hash.ok_or("No access hash for channel")?,
            })
        }
        _ => return Err("Only channels (folders) can be deleted.".to_string()),
    };

    crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .invoke(&tl::functions::channels::DeleteChannel {
                    channel: input_channel.clone(),
                })
                .await
                .map_err(|e| format!("Failed to delete channel: {}", e))
        },
        "delete_channel",
    )
    .await?;

    let _ = crate::db::delete_file_assets_in_folder(
        &db_pool,
        Some(folder_id),
        crate::tenant_auth::OWNER_WEB,
    );
    let _ = crate::db::set_file_index_complete(&db_pool, false);

    Ok(true)
}

#[derive(Clone, serde::Serialize)]
struct ProgressPayload {
    id: String,
    percent: u8,
    uploaded_bytes: u64,
    total_bytes: u64,
    speed_bytes_per_sec: u64,
}

/// Async reader wrapper that tracks bytes read for progress reporting.
/// Wraps a tokio File and counts how many bytes have been consumed.
struct ProgressReader {
    inner: tokio::io::BufReader<tokio::fs::File>,
    bytes_read: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl ProgressReader {
    async fn new(
        path: &str,
    ) -> Result<(Self, u64, std::sync::Arc<std::sync::atomic::AtomicU64>), String> {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|e| e.to_string())?;
        let metadata = file.metadata().await.map_err(|e| e.to_string())?;
        let size = metadata.len();
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let reader = Self {
            inner: tokio::io::BufReader::new(file),
            bytes_read: counter.clone(),
        };
        Ok((reader, size, counter))
    }
}

impl tokio::io::AsyncRead for ProgressReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if let std::task::Poll::Ready(Ok(())) = &result {
            let after = buf.filled().len();
            let delta = (after - before) as u64;
            self.bytes_read
                .fetch_add(delta, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }
}

/// Delete a partial file with retries (best-effort cleanup)
fn cleanup_partial_file(path: &str) {
    let path = path.to_string();
    std::thread::spawn(move || {
        for attempt in 0..5 {
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    log::info!("Cleaned up partial file: {}", path);
                    return;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
                Err(e) => {
                    log::warn!(
                        "Cleanup attempt {}/5 failed for {}: {}",
                        attempt + 1,
                        path,
                        e
                    );
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
    });
}

#[tauri::command]
pub async fn cmd_cancel_transfer(
    transfer_id: String,
    state: State<'_, TelegramState>,
) -> Result<bool, String> {
    log::info!("Cancelling transfer: {}", transfer_id);
    state
        .cancelled_transfers
        .write()
        .await
        .insert(transfer_id.clone());
    if let Ok(Some(tx)) = with_upload_cancellations(|m| m.remove(&transfer_id)) {
        let _ = tx.send(());
    }
    Ok(true)
}

#[tauri::command]
pub async fn cmd_upload_file(
    path: String,
    folder_id: Option<i64>,
    transfer_id: Option<String>,
    app_handle: tauri::AppHandle,
    state: State<'_, TelegramState>,
    bw_state: State<'_, BandwidthManager>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<String, String> {
    let size = tokio::fs::metadata(&path)
        .await
        .map_err(|e| e.to_string())?
        .len();
    bw_state.can_transfer(size).await?;

    let tid = transfer_id.unwrap_or_default();

    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    // Emit start progress
    if !tid.is_empty() {
        let _ = app_handle.emit(
            "upload-progress",
            ProgressPayload {
                id: tid.clone(),
                percent: 0,
                uploaded_bytes: 0,
                total_bytes: size,
                speed_bytes_per_sec: 0,
            },
        );
    }

    // Create progress-tracking reader
    let (reader, file_size, bytes_counter) = ProgressReader::new(&path).await?;
    let upload_limit = net_config.upload_limit_bytes_per_sec();
    let mut throttled = ThrottledReader::new(reader, upload_limit);
    let file_name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let file_name_for_index = file_name.clone();

    // Spawn a progress reporter task that emits events every 250ms
    let cancelled = state.cancelled_transfers.clone();
    let progress_tid = tid.clone();
    let progress_handle = app_handle.clone();
    let progress_counter = bytes_counter.clone();
    let progress_task = if !tid.is_empty() {
        Some(tokio::spawn(async move {
            let mut last_bytes: u64 = 0;
            let mut last_time = std::time::Instant::now();
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                let current = progress_counter.load(std::sync::atomic::Ordering::Relaxed);
                let now = std::time::Instant::now();
                let dt = now.duration_since(last_time).as_secs_f64();
                let speed = if dt > 0.0 {
                    ((current - last_bytes) as f64 / dt) as u64
                } else {
                    0
                };
                let percent = if file_size > 0 {
                    ((current as f64 / file_size as f64) * 100.0).min(99.0) as u8
                } else {
                    0
                };

                let _ = progress_handle.emit(
                    "upload-progress",
                    ProgressPayload {
                        id: progress_tid.clone(),
                        percent,
                        uploaded_bytes: current,
                        total_bytes: file_size,
                        speed_bytes_per_sec: speed,
                    },
                );

                last_bytes = current;
                last_time = now;

                if current >= file_size {
                    break;
                }
                // Check cancellation
                if cancelled.read().await.contains(&progress_tid) {
                    break;
                }
            }
        }))
    } else {
        None
    };

    // Check cancellation before starting
    if state.cancelled_transfers.read().await.contains(&tid) {
        state.cancelled_transfers.write().await.remove(&tid);
        if let Some(t) = progress_task {
            t.abort();
        }
        return Err("Transfer cancelled".to_string());
    }

    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    if !tid.is_empty() {
        with_upload_cancellations(|m| m.insert(tid.clone(), cancel_tx))?;
    }

    let client_clone = client.clone();
    let mut upload_task = tokio::spawn(async move {
        client_clone
            .upload_stream(&mut throttled, file_size as usize, file_name)
            .await
    });

    let upload_result = {
        tokio::select! {
            res = &mut upload_task => {
                if !tid.is_empty() {
                    let _ = with_upload_cancellations(|m| m.remove(&tid));
                }
                res.map_err(|e| format!("Task join error: {}", e))?
            }
            _ = cancel_rx => {
                log::info!("Aborting upload task for transfer ID: {}", tid);
                upload_task.abort();
                state.cancelled_transfers.write().await.remove(&tid);
                if let Some(t) = progress_task { t.abort(); }
                return Err("Transfer cancelled".to_string());
            }
        }
    };

    // Stop progress reporter
    if let Some(t) = progress_task {
        t.abort();
    }

    let uploaded_file = upload_result.map_err(map_error)?;
    let message = InputMessage::new().text("").file(uploaded_file);

    let peer = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            resolve_peer_with_limit(
                &client,
                folder_id,
                &state.peer_cache,
                net_config.peer_cache_size(),
            )
            .await
        },
        "resolve_peer(upload)",
    )
    .await?;

    // VPN-aware retry logic for send_message via unified wrapper
    let sent = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .send_message(&peer, message.clone())
                .await
                .map_err(|e| map_error(e))
        },
        "send_message(upload)",
    )
    .await?;

    let message_id = sent.id();
    if let Err(e) = crate::file_access::record_uploaded_file(
        &db_pool,
        message_id,
        folder_id,
        crate::tenant_auth::OWNER_WEB,
        &file_name_for_index,
        size as i64,
    ) {
        log::warn!("file_assets index after upload failed: {e}");
    }

    crate::metadata_cache::invalidate_files(&db_pool, folder_id);

    bw_state.add_up(size).await;
    if !tid.is_empty() {
        let _ = app_handle.emit(
            "upload-progress",
            ProgressPayload {
                id: tid,
                percent: 100,
                uploaded_bytes: size,
                total_bytes: size,
                speed_bytes_per_sec: 0,
            },
        );
    }
    Ok("File uploaded successfully".to_string())
}

#[tauri::command]
pub async fn cmd_delete_file(
    message_id: i32,
    folder_id: Option<i64>,
    #[allow(unused_variables)] app: tauri::AppHandle,
    state: State<'_, TelegramState>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<bool, String> {
    #[cfg(not(feature = "headless-server"))]
    {
        if crate::local_api::desktop_uses_asset_index(&app, &db_pool).await? {
            return Ok(crate::db::delete_file_asset(&db_pool, message_id, None).unwrap_or(false));
        }
    }

    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    let peer = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            resolve_peer_with_limit(
                &client,
                folder_id,
                &state.peer_cache,
                net_config.peer_cache_size(),
            )
            .await
        },
        "resolve_peer(delete_file)",
    )
    .await?;

    crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .delete_messages(&peer, &[message_id])
                .await
                .map_err(|e| e.to_string())
        },
        "delete_messages",
    )
    .await?;

    let _ = crate::db::delete_file_asset(&db_pool, message_id, None);

    Ok(true)
}

#[tauri::command]
pub async fn cmd_download_file(
    message_id: i32,
    save_path: String,
    folder_id: Option<i64>,
    transfer_id: Option<String>,
    app_handle: tauri::AppHandle,
    state: State<'_, TelegramState>,
    bw_state: State<'_, BandwidthManager>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    #[allow(unused_variables)] db_pool: State<'_, crate::db::DbConnection>,
) -> Result<String, String> {
    let tid = transfer_id.unwrap_or_default();

    #[cfg(not(feature = "headless-server"))]
    {
        if crate::local_api::desktop_uses_asset_index(&app_handle, &db_pool).await? {
            return download_file_via_local_api(
                &app_handle,
                message_id,
                folder_id,
                &save_path,
                &tid,
                &state,
                &bw_state,
                &net_config,
            )
            .await;
        }
    }

    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    let peer = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            resolve_peer_with_limit(
                &client,
                folder_id,
                &state.peer_cache,
                net_config.peer_cache_size(),
            )
            .await
        },
        "resolve_peer(download)",
    )
    .await?;

    // Use get_messages_by_id for efficient message lookup (same as server.rs)
    let messages = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .get_messages_by_id(&peer, &[message_id])
                .await
                .map_err(|e| e.to_string())
        },
        "get_messages_by_id",
    )
    .await?;

    let msg = messages
        .into_iter()
        .flatten()
        .next()
        .ok_or_else(|| "Message not found".to_string())?;

    let media = msg
        .media()
        .ok_or_else(|| "No media in message".to_string())?;

    let total_size = match &media {
        Media::Document(d) => d.size() as u64,
        Media::Photo(_) => 1024 * 1024,
        _ => 0,
    };

    bw_state.can_transfer(total_size).await?;

    // Emit start
    if !tid.is_empty() {
        let _ = app_handle.emit(
            "download-progress",
            ProgressPayload {
                id: tid.clone(),
                percent: 0,
                uploaded_bytes: 0,
                total_bytes: total_size,
                speed_bytes_per_sec: 0,
            },
        );
    }

    // Stream download with per-chunk progress
    let chunk_size = net_config.chunk_size_bytes() as i32;
    let mut download_iter = client.iter_download(&media).chunk_size(chunk_size);
    let mut file = tokio::fs::File::create(&save_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut last_emit_time = std::time::Instant::now();
    let mut last_emit_bytes: u64 = 0;
    let mut chunk_retry_budget = net_config.retry_attempts();
    let mut throttle_bytes = 0u64;
    let mut throttle_start = std::time::Instant::now();

    while let Some(chunk) = download_iter.next().await.transpose() {
        // Check cancellation
        if state.cancelled_transfers.read().await.contains(&tid) {
            state.cancelled_transfers.write().await.remove(&tid);
            drop(file);
            cleanup_partial_file(&save_path);
            return Err("Transfer cancelled".to_string());
        }

        let bytes = match chunk {
            Ok(b) => {
                chunk_retry_budget = net_config.retry_attempts(); // reset on success
                b
            }
            Err(e) => {
                let err = map_error(&e);
                if chunk_retry_budget > 0 {
                    chunk_retry_budget -= 1;
                    log::warn!(
                        "Download chunk error (retries left: {}): {}",
                        chunk_retry_budget,
                        err
                    );
                    let delay = backoff_ms(
                        0,
                        net_config.retry_base_backoff_ms(),
                        net_config.retry_max_backoff_ms(),
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    continue;
                }
                return Err(format!("Download chunk error: {}", err));
            }
        };
        tokio::io::AsyncWriteExt::write_all(&mut file, &bytes)
            .await
            .map_err(|e| e.to_string())?;
        downloaded += bytes.len() as u64;

        // Time-based progress emission (every 250ms)
        if !tid.is_empty() {
            let now = std::time::Instant::now();
            let dt = now.duration_since(last_emit_time).as_secs_f64();
            if dt >= 0.25 || downloaded >= total_size {
                let speed = if dt > 0.0 {
                    ((downloaded - last_emit_bytes) as f64 / dt) as u64
                } else {
                    0
                };
                let percent = if total_size > 0 {
                    ((downloaded as f64 / total_size as f64) * 100.0).min(100.0) as u8
                } else {
                    0
                };
                let _ = app_handle.emit(
                    "download-progress",
                    ProgressPayload {
                        id: tid.clone(),
                        percent,
                        uploaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: speed,
                    },
                );
                last_emit_time = now;
                last_emit_bytes = downloaded;
            }
        }

        // Bandwidth throttle
        let dl_limit = net_config.download_limit_bytes_per_sec();
        if dl_limit > 0 {
            throttle_transfer_bytes(
                bytes.len() as u64,
                dl_limit,
                &mut throttle_bytes,
                &mut throttle_start,
            )
            .await;
        }
    }

    bw_state.add_down(total_size).await;

    // Emit completion
    if !tid.is_empty() {
        let _ = app_handle.emit(
            "download-progress",
            ProgressPayload {
                id: tid,
                percent: 100,
                uploaded_bytes: downloaded,
                total_bytes: total_size,
                speed_bytes_per_sec: 0,
            },
        );
    }

    Ok("Download successful".to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFilesResult {
    pub moved: usize,
    pub old_message_ids: Vec<i32>,
    pub new_message_ids: Vec<i32>,
    pub target_folder_id: Option<i64>,
}

#[tauri::command]
pub async fn cmd_move_files(
    message_ids: Vec<i32>,
    source_folder_id: Option<i64>,
    target_folder_id: Option<i64>,
    state: State<'_, TelegramState>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<MoveFilesResult, String> {
    if source_folder_id == target_folder_id {
        return Ok(MoveFilesResult {
            moved: 0,
            old_message_ids: message_ids,
            new_message_ids: vec![],
            target_folder_id,
        });
    }
    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    let source_peer = crate::vpn_optimizer::with_retry(
        &net_config,
        || async { resolve_peer(&client, source_folder_id, &state.peer_cache).await },
        "resolve_peer(move_source)",
    )
    .await?;
    let target_peer = crate::vpn_optimizer::with_retry(
        &net_config,
        || async { resolve_peer(&client, target_folder_id, &state.peer_cache).await },
        "resolve_peer(move_target)",
    )
    .await?;

    let forwarded = crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .forward_messages(&target_peer, &message_ids, &source_peer)
                .await
                .map_err(|e| format!("Forward failed: {}", e))
        },
        "forward_messages",
    )
    .await?;

    let new_ids: Vec<i32> = forwarded
        .iter()
        .filter_map(|m| m.as_ref().map(|msg| msg.id()))
        .collect();
    if new_ids.len() != message_ids.len() {
        return Err(format!(
            "Forward returned {} message(s), expected {} — originals not deleted",
            new_ids.len(),
            message_ids.len()
        ));
    }

    crate::vpn_optimizer::with_retry(
        &net_config,
        || async {
            client
                .delete_messages(&source_peer, &message_ids)
                .await
                .map_err(|e| format!("Delete original failed: {}", e))
        },
        "delete_messages(move)",
    )
    .await?;
    crate::file_access::remap_file_assets_after_move(
        &db_pool,
        &message_ids,
        &new_ids,
        target_folder_id,
    )
    .map_err(|e| format!("Index remap after move failed: {e}"))?;

    Ok(MoveFilesResult {
        moved: new_ids.len(),
        old_message_ids: message_ids,
        new_message_ids: new_ids,
        target_folder_id,
    })
}

#[cfg(not(feature = "headless-server"))]
fn asset_record_to_file_metadata(r: crate::db::FileAssetRecord) -> FileMetadata {
    let ext = std::path::Path::new(&r.file_name)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase());
    FileMetadata {
        id: r.message_id as i64,
        folder_id: r.folder_id,
        name: r.file_name,
        size: r.file_size.max(0) as u64,
        mime_type: None,
        file_ext: ext,
        created_at: chrono::DateTime::from_timestamp(r.created_at, 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| r.created_at.to_string()),
        icon_type: "file".to_string(),
    }
}

#[cfg(not(feature = "headless-server"))]
fn search_files_from_asset_index(
    db_pool: &crate::db::DbConnection,
    query: &str,
    owner_id: Option<&str>,
    limit: usize,
) -> Result<Vec<FileMetadata>, String> {
    let records = crate::db::search_file_assets(db_pool, query, owner_id, None, false, limit)?;
    Ok(records
        .into_iter()
        .map(asset_record_to_file_metadata)
        .collect())
}

#[cfg(not(feature = "headless-server"))]
fn list_files_from_asset_index(
    db_pool: &crate::db::DbConnection,
    folder_id: Option<i64>,
) -> Result<Vec<FileMetadata>, String> {
    let has_folder = folder_id.is_some();
    let records = crate::db::list_file_assets_scoped(
        db_pool,
        Some(crate::tenant_auth::OWNER_WEB),
        folder_id,
        has_folder,
        None,
        50_000,
        0,
    )?;
    Ok(records
        .into_iter()
        .map(asset_record_to_file_metadata)
        .collect())
}

#[cfg(not(feature = "headless-server"))]
async fn download_file_via_local_api(
    app_handle: &tauri::AppHandle,
    message_id: i32,
    folder_id: Option<i64>,
    save_path: &str,
    tid: &str,
    state: &TelegramState,
    bw_state: &BandwidthManager,
    net_config: &std::sync::Arc<NetworkConfig>,
) -> Result<String, String> {
    use futures_util::StreamExt;

    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let settings = crate::commands::api_settings::load_settings_at(&data_dir);
    let pwd = crate::commands::api_settings::load_local_access_pwd(&data_dir);
    if pwd.is_empty() {
        return Err("Local API access password is not configured".to_string());
    }

    let url = crate::local_api::build_download_url(settings.port, message_id, folder_id);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(&url)
        .header("X-Access-Pwd", &pwd)
        .send()
        .await
        .map_err(|e| format!("API download request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("API download failed ({status}): {body}"));
    }

    let total_size = response.content_length().unwrap_or(0);
    if total_size > 0 {
        bw_state.can_transfer(total_size).await?;
    }

    if !tid.is_empty() {
        let _ = app_handle.emit(
            "download-progress",
            ProgressPayload {
                id: tid.to_string(),
                percent: 0,
                uploaded_bytes: 0,
                total_bytes: total_size,
                speed_bytes_per_sec: 0,
            },
        );
    }

    let mut file = tokio::fs::File::create(save_path)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut last_emit_time = std::time::Instant::now();
    let mut last_emit_bytes: u64 = 0;
    let mut throttle_bytes = 0u64;
    let mut throttle_start = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        if !tid.is_empty() && state.cancelled_transfers.read().await.contains(tid) {
            state.cancelled_transfers.write().await.remove(tid);
            drop(file);
            cleanup_partial_file(save_path);
            return Err("Transfer cancelled".to_string());
        }

        let bytes = chunk_result.map_err(|e| format!("API download stream error: {e}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &bytes)
            .await
            .map_err(|e| e.to_string())?;
        downloaded += bytes.len() as u64;

        if !tid.is_empty() {
            let now = std::time::Instant::now();
            let dt = now.duration_since(last_emit_time).as_secs_f64();
            if dt >= 0.25 || (total_size > 0 && downloaded >= total_size) {
                let speed = if dt > 0.0 {
                    ((downloaded - last_emit_bytes) as f64 / dt) as u64
                } else {
                    0
                };
                let percent = if total_size > 0 {
                    ((downloaded as f64 / total_size as f64) * 100.0).min(100.0) as u8
                } else {
                    0
                };
                let _ = app_handle.emit(
                    "download-progress",
                    ProgressPayload {
                        id: tid.to_string(),
                        percent,
                        uploaded_bytes: downloaded,
                        total_bytes: total_size,
                        speed_bytes_per_sec: speed,
                    },
                );
                last_emit_time = now;
                last_emit_bytes = downloaded;
            }
        }

        let dl_limit = net_config.download_limit_bytes_per_sec();
        if dl_limit > 0 {
            throttle_transfer_bytes(
                bytes.len() as u64,
                dl_limit,
                &mut throttle_bytes,
                &mut throttle_start,
            )
            .await;
        }
    }

    let counted = if total_size > 0 {
        total_size
    } else {
        downloaded
    };
    bw_state.add_down(counted).await;

    if !tid.is_empty() {
        let _ = app_handle.emit(
            "download-progress",
            ProgressPayload {
                id: tid.to_string(),
                percent: 100,
                uploaded_bytes: downloaded,
                total_bytes: if total_size > 0 {
                    total_size
                } else {
                    downloaded
                },
                speed_bytes_per_sec: 0,
            },
        );
    }

    Ok("Download successful".to_string())
}

#[tauri::command]
pub async fn cmd_get_files(
    folder_id: Option<i64>,
    #[allow(unused_variables)] app: tauri::AppHandle,
    state: State<'_, TelegramState>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<Vec<FileMetadata>, String> {
    #[cfg(not(feature = "headless-server"))]
    {
        if crate::local_api::desktop_uses_asset_index(&app, &db_pool).await? {
            return list_files_from_asset_index(&db_pool, folder_id);
        }
    }

    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    let files = list_document_files_in_folder(&client, folder_id, &state.peer_cache).await?;

    crate::file_access::index_file_metadata_list(&db_pool, &files, crate::tenant_auth::OWNER_WEB);

    Ok(files)
}

#[derive(serde::Serialize)]
pub struct RebuildIndexResult {
    pub folders_scanned: usize,
    pub files_indexed: usize,
}

/// Scan all given folders (None = Saved Messages) and rebuild the local file_assets index.
pub async fn rebuild_file_index_for_folders(
    client: &grammers_client::Client,
    peer_cache: &std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<i64, Peer>>>,
    db_pool: &crate::db::DbConnection,
    folder_ids: Vec<Option<i64>>,
    owner_id: &str,
) -> Result<RebuildIndexResult, String> {
    let targets: Vec<Option<i64>> = if folder_ids.is_empty() {
        vec![None]
    } else {
        folder_ids
    };

    crate::db::delete_all_file_assets_for_owner(db_pool, owner_id)?;

    let mut files_indexed = 0usize;
    for folder_id in &targets {
        let files = list_document_files_in_folder(client, *folder_id, peer_cache).await?;
        files_indexed += files.len();
        crate::file_access::index_file_metadata_list(db_pool, &files, owner_id);
    }

    crate::db::set_file_index_complete(db_pool, true)?;

    Ok(RebuildIndexResult {
        folders_scanned: targets.len(),
        files_indexed,
    })
}

#[tauri::command]
pub async fn cmd_rebuild_file_index(
    folder_ids: Vec<Option<i64>>,
    state: State<'_, TelegramState>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<RebuildIndexResult, String> {
    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    rebuild_file_index_for_folders(
        &client,
        &state.peer_cache,
        &db_pool,
        folder_ids,
        crate::tenant_auth::OWNER_WEB,
    )
    .await
}

#[tauri::command]
pub fn cmd_invalidate_file_index(
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<bool, String> {
    crate::db::set_file_index_complete(&db_pool, false)?;
    Ok(true)
}

#[tauri::command]
pub async fn cmd_search_global(
    query: String,
    #[allow(unused_variables)] app: tauri::AppHandle,
    state: State<'_, TelegramState>,
    db_pool: State<'_, crate::db::DbConnection>,
) -> Result<Vec<FileMetadata>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    #[cfg(not(feature = "headless-server"))]
    {
        if crate::local_api::desktop_uses_asset_index(&app, &db_pool).await? {
            return search_files_from_asset_index(
                &db_pool,
                q,
                Some(crate::tenant_auth::OWNER_WEB),
                50,
            );
        }
    }

    if crate::db::is_file_index_complete(&db_pool)? {
        #[cfg(not(feature = "headless-server"))]
        {
            return search_files_from_asset_index(&db_pool, q, None, 50);
        }
        #[cfg(feature = "headless-server")]
        {
            let records = crate::db::search_file_assets(&db_pool, q, None, None, false, 50)?;
            return Ok(records
                .into_iter()
                .map(|r| {
                    let ext = std::path::Path::new(&r.file_name)
                        .extension()
                        .map(|os| os.to_str().unwrap_or("").to_string());
                    FileMetadata {
                        id: r.message_id as i64,
                        folder_id: r.folder_id,
                        name: r.file_name,
                        size: r.file_size as u64,
                        mime_type: None,
                        file_ext: ext,
                        created_at: r.created_at.to_string(),
                        icon_type: "file".into(),
                    }
                })
                .collect());
        }
    }

    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;
    let mut files = Vec::new();

    log::info!("Searching global for: {}", query);

    let result = client
        .invoke(&tl::functions::messages::SearchGlobal {
            q: query,
            filter: tl::enums::MessagesFilter::InputMessagesFilterDocument,
            min_date: 0,
            max_date: 0,
            offset_rate: 0,
            offset_peer: tl::enums::InputPeer::Empty,
            offset_id: 0,
            limit: 50,
            folder_id: None,
            broadcasts_only: false,
            groups_only: false,
            users_only: false,
        })
        .await
        .map_err(map_error)?;

    if let tl::enums::messages::Messages::Messages(msgs) = result {
        for msg in msgs.messages {
            if let tl::enums::Message::Message(m) = msg {
                if let Some(tl::enums::MessageMedia::Document(d)) = m.media {
                    let doc = match d.document {
                        Some(tl::enums::Document::Document(doc)) => doc,
                        None => return Err("Search result missing document payload".to_string()),
                        Some(_) => {
                            return Err("Search result has unsupported document type".to_string())
                        }
                    };
                    let name = doc
                        .attributes
                        .iter()
                        .find_map(|a| match a {
                            tl::enums::DocumentAttribute::Filename(f) => Some(f.file_name.clone()),
                            _ => None,
                        })
                        .unwrap_or("Unknown".to_string());
                    let size = doc.size as u64;
                    let mime = doc.mime_type.clone();
                    let ext = std::path::Path::new(&name)
                        .extension()
                        .map(|os| os.to_str().unwrap_or("").to_string());
                    let folder_id =
                        crate::commands::utils::telegram_peer_id_to_folder_id(&m.peer_id);
                    files.push(FileMetadata {
                        id: m.id as i64,
                        folder_id,
                        name,
                        size,
                        mime_type: Some(mime),
                        file_ext: ext,
                        created_at: m.date.to_string(),
                        icon_type: "file".into(),
                    });
                }
            }
        }
    } else if let tl::enums::messages::Messages::Slice(msgs) = result {
        for msg in msgs.messages {
            if let tl::enums::Message::Message(m) = msg {
                if let Some(tl::enums::MessageMedia::Document(d)) = m.media {
                    let doc = match d.document {
                        Some(tl::enums::Document::Document(doc)) => doc,
                        None => return Err("Search result missing document payload".to_string()),
                        Some(_) => {
                            return Err("Search result has unsupported document type".to_string())
                        }
                    };
                    let name = doc
                        .attributes
                        .iter()
                        .find_map(|a| match a {
                            tl::enums::DocumentAttribute::Filename(f) => Some(f.file_name.clone()),
                            _ => None,
                        })
                        .unwrap_or("Unknown".to_string());
                    let size = doc.size as u64;
                    let mime = doc.mime_type.clone();
                    let ext = std::path::Path::new(&name)
                        .extension()
                        .map(|os| os.to_str().unwrap_or("").to_string());
                    let folder_id =
                        crate::commands::utils::telegram_peer_id_to_folder_id(&m.peer_id);
                    files.push(FileMetadata {
                        id: m.id as i64,
                        folder_id,
                        name,
                        size,
                        mime_type: Some(mime),
                        file_ext: ext,
                        created_at: m.date.to_string(),
                        icon_type: "file".into(),
                    });
                }
            }
        }
    }

    Ok(files)
}

#[tauri::command]
pub async fn cmd_scan_folders(
    state: State<'_, TelegramState>,
    net_config: State<'_, std::sync::Arc<NetworkConfig>>,
) -> Result<Vec<FolderMetadata>, String> {
    let client_opt = { state.client.lock().await.clone() };
    let client = require_client(client_opt)?;

    let mut folders = Vec::new();
    let mut dialogs = client.iter_dialogs();
    let mut discovered = HashMap::new();

    log::info!("Starting Folder Scan...");

    while let Some(dialog) = dialogs.next().await.map_err(|e| e.to_string())? {
        // Populate peer cache for every dialog we encounter (free priming)
        match &dialog.peer {
            Peer::Channel(c) => {
                let id = c.raw.id;
                discovered.insert(id, dialog.peer.clone());

                let name = c.raw.title.clone();
                let access_hash = c.raw.access_hash.unwrap_or(0);

                log::debug!("[SCAN] Processing Channel: '{}' (ID: {})", name, id);

                // Strategy 1: Title
                if name.to_lowercase().contains("[td]") {
                    log::info!(" -> MATCH via Title: {}", name);
                    let display_name = name
                        .replace(" [TD]", "")
                        .replace(" [td]", "")
                        .replace("[TD]", "")
                        .replace("[td]", "")
                        .trim()
                        .to_string();
                    folders.push(FolderMetadata {
                        id,
                        name: display_name,
                        parent_id: None,
                    });
                    continue;
                }

                // Strategy 2: About (Only if we are the creator to avoid rate limits on third-party channels)
                if c.raw.creator {
                    let input_chan = tl::enums::InputChannel::Channel(tl::types::InputChannel {
                        channel_id: c.raw.id,
                        access_hash,
                    });

                    match client
                        .invoke(&tl::functions::channels::GetFullChannel {
                            channel: input_chan,
                        })
                        .await
                    {
                        Ok(tl::enums::messages::ChatFull::Full(f)) => {
                            if let tl::enums::ChatFull::Full(cf) = f.full_chat {
                                if cf.about.contains("[telegram-drive-folder]") {
                                    log::info!(" -> MATCH via About: {}", name);
                                    folders.push(FolderMetadata {
                                        id,
                                        name: name.clone(),
                                        parent_id: None,
                                    });
                                }
                            }
                        }
                        Err(e) => log::warn!(" -> Failed to get full info: {}", e),
                    }
                }
            }
            Peer::User(u) => {
                discovered.insert(u.raw.id(), dialog.peer.clone());
                log::debug!("[SCAN] Cached User Peer: {}", u.raw.id());
            }
            peer => {
                log::debug!("[SCAN] Skipped Peer: {:?}", peer);
            }
        }
    }

    {
        let mut cache = state.peer_cache.write().await;
        cache.extend(discovered);
        crate::commands::utils::trim_peer_cache(&mut cache, net_config.peer_cache_size());
    }

    let cache_len = state.peer_cache.read().await.len();
    log::info!(
        "Scan complete. Found {} folders. Peer cache size: {}.",
        folders.len(),
        cache_len
    );
    Ok(folders)
}

/// Zip a folder's contents into a temp file and return the path.
/// The resulting zip preserves the relative directory structure.
#[tauri::command]
pub async fn cmd_zip_folder(folder_path: String) -> Result<String, String> {
    let src = std::path::Path::new(&folder_path)
        .canonicalize()
        .map_err(|e| format!("Invalid folder path: {}", e))?;
    if !src.is_dir() {
        return Err(format!("'{}' is not a directory", folder_path));
    }

    let folder_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "folder".to_string());

    let zip_path = std::env::temp_dir().join(format!("{}.zip", folder_name));
    let src_owned = src.clone();
    let out_path = zip_path.clone();

    // Run blocking I/O on a dedicated thread so we don't stall the async runtime
    let (zip_path_str, zip_size) = tokio::task::spawn_blocking(move || {
        let file = std::fs::File::create(&out_path)
            .map_err(|e| format!("Failed to create zip file: {}", e))?;
        let mut zip_writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for entry in walkdir::WalkDir::new(&src_owned)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            let relative = path.strip_prefix(&src_owned).unwrap_or(path);

            if path.is_file() {
                let name = relative.to_string_lossy().to_string();
                zip_writer
                    .start_file(&name, options)
                    .map_err(|e| format!("Failed to add '{}': {}", name, e))?;
                let mut f = std::fs::File::open(path)
                    .map_err(|e| format!("Failed to open '{}': {}", name, e))?;
                std::io::copy(&mut f, &mut zip_writer)
                    .map_err(|e| format!("Failed to write '{}': {}", name, e))?;
            } else if path.is_dir() && path != src_owned {
                let dir_name = format!("{}/", relative.to_string_lossy());
                zip_writer
                    .add_directory(&dir_name, options)
                    .map_err(|e| format!("Failed to add dir '{}': {}", dir_name, e))?;
            }
        }

        zip_writer
            .finish()
            .map_err(|e| format!("Failed to finalize zip: {}", e))?;
        let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
        Ok::<(String, u64), String>((out_path.to_string_lossy().to_string(), size))
    })
    .await
    .map_err(|e| format!("Zip task panicked: {}", e))?
    .map_err(|e: String| e)?;

    log::info!(
        "Zipped '{}' -> '{}' ({} bytes)",
        folder_name,
        zip_path_str,
        zip_size
    );

    Ok(zip_path_str)
}

/// Delete a temporary zip file created by cmd_zip_folder.
#[tauri::command]
pub async fn cmd_delete_temp_zip(path: String) -> Result<(), String> {
    let path_clone = path.clone();
    tokio::task::spawn_blocking(move || {
        let p = std::path::Path::new(&path_clone);
        if !p.exists() {
            return Ok(());
        }
        let canonical_p = p
            .canonicalize()
            .map_err(|e| format!("Invalid path: {}", e))?;
        let tmp = std::env::temp_dir()
            .canonicalize()
            .map_err(|e| format!("Could not resolve temp directory: {}", e))?;
        if !canonical_p.starts_with(&tmp) {
            return Err("Refusing to delete file outside temp directory".to_string());
        }
        std::fs::remove_file(&canonical_p).map_err(|e| e.to_string())?;
        log::info!("Cleaned up temp zip: {}", path_clone);
        Ok(())
    })
    .await
    .map_err(|e| format!("Task panicked: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_client_err_when_disconnected() {
        match require_client(None) {
            Err(err) => assert!(err.contains("not connected")),
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn move_files_result_serializes_camel_case() {
        let result = MoveFilesResult {
            moved: 2,
            old_message_ids: vec![10, 11],
            new_message_ids: vec![20, 21],
            target_folder_id: Some(99),
        };
        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["moved"], 2);
        assert_eq!(json["oldMessageIds"], serde_json::json!([10, 11]));
        assert_eq!(json["newMessageIds"], serde_json::json!([20, 21]));
        assert_eq!(json["targetFolderId"], 99);
    }
}
