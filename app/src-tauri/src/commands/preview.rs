use tauri::State;
use tauri::Manager;
use grammers_client::types::Media;
use base64::{Engine as _, engine::general_purpose};
use crate::TelegramState;
use crate::bandwidth::BandwidthManager;
use crate::commands::utils::resolve_peer;
use crate::db::DbConnection;

const PREVIEW_CACHE_MAX_FILES: usize = 30;
const PREVIEW_CACHE_MAX_TOTAL_BYTES: u64 = 80 * 1024 * 1024;

fn preview_cache_path(
    cache_dir: &std::path::Path,
    folder_id: Option<i64>,
    message_id: i32,
    ext: &str,
) -> std::path::PathBuf {
    let folder_key = folder_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| "home".to_string());
    cache_dir.join(format!("{folder_key}_{message_id}.{ext}"))
}

#[cfg(not(feature = "headless-server"))]
async fn resolve_preview_extension(
    db_pool: &DbConnection,
    message_id: i32,
) -> String {
    if let Ok(Some(record)) = crate::db::get_file_asset(db_pool, message_id) {
        let ext = crate::local_api::extension_from_filename(&record.file_name);
        if ext != "bin" {
            return ext;
        }
    }
    if let Ok(Some(record)) = crate::db::get_bot_file_map(db_pool, message_id) {
        let ext = crate::local_api::extension_from_filename(&record.file_name);
        if ext != "bin" {
            return ext;
        }
    }
    "bin".to_string()
}

#[cfg(not(feature = "headless-server"))]
async fn preview_via_local_api(
    app_handle: &tauri::AppHandle,
    message_id: i32,
    folder_id: Option<i64>,
    cache_dir: std::path::PathBuf,
    db_pool: &DbConnection,
    bw_state: &BandwidthManager,
) -> Result<String, String> {
    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let bridge = crate::local_api::LocalApiBridge::from_data_dir(&data_dir);
    let ext = resolve_preview_extension(db_pool, message_id).await;
    let save_path = preview_cache_path(&cache_dir, folder_id, message_id, &ext);
    let save_path_str = save_path.to_string_lossy().to_string();

    let file_ready = if tokio::fs::metadata(&save_path).await.is_ok() {
        true
    } else {
        match crate::local_api::fetch_file_to_path(
            &bridge,
            message_id,
            folder_id,
            &save_path_str,
            bw_state,
        )
        .await
        {
            Ok(_) => {
                prune_preview_cache(cache_dir.clone()).await;
                true
            }
            Err(e) => {
                log::error!("Bot preview download error: {e}");
                false
            }
        }
    };

    if !file_ready {
        return Err("File not found or failed to download".to_string());
    }

    let lower_ext = ext.to_lowercase();
    if ["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg"].contains(&lower_ext.as_str()) {
        match tokio::fs::read(&save_path).await {
            Ok(bytes) => {
                let b64 = general_purpose::STANDARD.encode(&bytes);
                let mime = match lower_ext.as_str() {
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    "bmp" => "image/bmp",
                    "svg" => "image/svg+xml",
                    _ => "image/jpeg",
                };
                return Ok(format!("data:{mime};base64,{b64}"));
            }
            Err(e) => {
                log::error!("Failed to read preview file: {e}");
                return Ok(save_path_str);
            }
        }
    }
    Ok(save_path_str)
}

#[cfg(not(feature = "headless-server"))]
async fn thumbnail_via_local_api(
    app_handle: &tauri::AppHandle,
    message_id: i32,
    folder_id: Option<i64>,
    cache_dir: std::path::PathBuf,
    db_pool: &DbConnection,
    bw_state: &BandwidthManager,
) -> Result<String, String> {
    let ext = resolve_preview_extension(db_pool, message_id).await;
    let lower = ext.to_lowercase();
    if !["jpg", "jpeg", "png", "gif", "webp"].contains(&lower.as_str()) {
        return Ok(String::new());
    }
    let thumb_ext = if lower == "jpeg" {
        "jpg".to_string()
    } else {
        lower.clone()
    };
    let save_path = cache_dir.join(format!("{message_id}.{thumb_ext}"));
    if tokio::fs::metadata(&save_path).await.is_ok() {
        if let Ok(bytes) = tokio::fs::read(&save_path).await {
            let mime = match thumb_ext.as_str() {
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "image/jpeg",
            };
            let b64 = general_purpose::STANDARD.encode(&bytes);
            return Ok(format!("data:{mime};base64,{b64}"));
        }
    }

    let data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let bridge = crate::local_api::LocalApiBridge::from_data_dir(&data_dir);
    let save_path_str = save_path.to_string_lossy().to_string();
    if crate::local_api::fetch_file_to_path(
        &bridge,
        message_id,
        folder_id,
        &save_path_str,
        bw_state,
    )
    .await
    .is_ok()
    {
        if let Ok(bytes) = tokio::fs::read(&save_path).await {
            bw_state.add_down(bytes.len() as u64).await;
            let mime = match thumb_ext.as_str() {
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "image/jpeg",
            };
            let b64 = general_purpose::STANDARD.encode(&bytes);
            return Ok(format!("data:{mime};base64,{b64}"));
        }
    }
    Ok(String::new())
}

async fn prune_preview_cache(cache_dir: std::path::PathBuf) {
    let _ = tokio::task::spawn_blocking(move || {
        let read_dir = match std::fs::read_dir(&cache_dir) {
            Ok(entries) => entries,
            Err(_) => return,
        };
        let mut files: Vec<(std::path::PathBuf, std::time::SystemTime, u64)> = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                files.push((path, modified, meta.len()));
            }
        }
        files.sort_by_key(|(_, modified, _)| *modified);
        let mut total_bytes: u64 = files.iter().map(|(_, _, len)| *len).sum();
        while files.len() > PREVIEW_CACHE_MAX_FILES || total_bytes > PREVIEW_CACHE_MAX_TOTAL_BYTES {
            if let Some((path, _, len)) = files.first().cloned() {
                let _ = std::fs::remove_file(&path);
                total_bytes = total_bytes.saturating_sub(len);
                files.remove(0);
            } else {
                break;
            }
        }
    }).await;
}

#[tauri::command]
pub async fn cmd_get_preview(
    message_id: i32,
    folder_id: Option<i64>,
    app_handle: tauri::AppHandle,
    state: State<'_, TelegramState>,
    bw_state: State<'_, BandwidthManager>,
    #[allow(unused_variables)]
    db_pool: State<'_, DbConnection>,
) -> Result<String, String> {
    let cache_dir = app_handle
        .path()
        .app_cache_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("previews");
    if tokio::fs::metadata(&cache_dir).await.is_err() {
        let _ = tokio::fs::create_dir_all(&cache_dir).await;
    }
    prune_preview_cache(cache_dir.clone()).await;
    log::info!("Using preview cache dir: {:?}", cache_dir);
    log::info!("Preview Request: msg_id={}", message_id);

    #[cfg(not(feature = "headless-server"))]
    {
        if crate::local_api::desktop_uses_asset_index(&app_handle, &db_pool).await? {
            return preview_via_local_api(
                &app_handle,
                message_id,
                folder_id,
                cache_dir,
                &db_pool,
                &bw_state,
            )
            .await;
        }
    }

    let client_opt = { state.client.lock().await.clone() };
    let Some(client) = client_opt else {
        return Ok("".to_string());
    };

    let peer = resolve_peer(&client, folder_id, &state.peer_cache).await?;
    let messages = client.get_messages_by_id(&peer, &[message_id])
        .await.map_err(|e| e.to_string())?;
    let target_message = messages.into_iter().flatten().next();

    if let Some(msg) = target_message {
        if let Some(media) = msg.media() {
            let ext = match &media {
                Media::Document(d) => {
                    let mut e = std::path::Path::new(d.name())
                        .extension()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if e.is_empty() {
                        if let Some(mime) = d.mime_type() {
                            e = match mime {
                                "image/jpeg" => "jpg".to_string(),
                                "image/png" => "png".to_string(),
                                "video/mp4" => "mp4".to_string(),
                                _ => "bin".to_string(),
                            };
                        } else {
                            e = "bin".to_string();
                        }
                    }
                    e
                },
                Media::Photo(_) => "jpg".to_string(),
                _ => "bin".to_string(),
            };
            let folder_key = folder_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "home".to_string());
            let save_path = cache_dir.join(format!("{}_{}.{}", folder_key, message_id, ext));
            let save_path_str = save_path.to_string_lossy().to_string();

            let file_ready = if tokio::fs::metadata(&save_path).await.is_ok() {
                log::info!("File ({}) exists in cache.", message_id);
                true
            } else {
                let size = match &media {
                    Media::Document(d) => d.size() as u64,
                    Media::Photo(_) => 1024 * 1024,
                    _ => 0,
                };
                log::info!("Downloading preview... Size: {}", size);
                if let Err(e) = bw_state.can_transfer(size).await {
                    log::warn!("Bandwidth limit hit for preview: {}", e);
                    false
                } else {
                    match client.download_media(&media, &save_path_str).await {
                        Ok(_) => {
                            log::info!("Preview download complete.");
                            bw_state.add_down(size).await;
                            prune_preview_cache(cache_dir.clone()).await;
                            true
                        },
                        Err(e) => {
                            log::error!("Preview Download Error: {}", e);
                            false
                        }
                    }
                }
            };
            if file_ready {
                let lower_ext = ext.to_lowercase();
                if ["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg"].contains(&lower_ext.as_str()) {
                    log::info!("Converting image to Base64...");
                    match tokio::fs::read(&save_path).await {
                        Ok(bytes) => {
                            let b64 = general_purpose::STANDARD.encode(&bytes);
                            let mime = match lower_ext.as_str() {
                                "png" => "image/png",
                                "gif" => "image/gif",
                                "webp" => "image/webp",
                                "bmp" => "image/bmp",
                                "svg" => "image/svg+xml",
                                _ => "image/jpeg",
                            };
                            return Ok(format!("data:{};base64,{}", mime, b64));
                        },
                        Err(e) => {
                            log::error!("Failed to read file for base64: {}", e);
                            return Ok(save_path_str);
                        }
                    }
                }
                log::info!("Returning path preview: {}", save_path_str);
                return Ok(save_path_str);
            }
        }
    }
    Err("File not found or failed to download".to_string())
}

#[tauri::command]
pub async fn cmd_clean_cache(
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let cache_dir = app_handle
        .path()
        .app_cache_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("previews");
    let thumb_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("thumbnails");

    let _ = tokio::task::spawn_blocking(move || {
        if cache_dir.exists() {
            let _ = std::fs::remove_dir_all(cache_dir);
        }
        if thumb_dir.exists() {
            let _ = std::fs::remove_dir_all(thumb_dir);
        }
    }).await;
    Ok(())
}

/// Get a small thumbnail for inline display in file cards.
/// Returns base64 data URL for images, empty string for non-image files.
/// Uses same cache as cmd_get_preview for consistency.
#[tauri::command]
pub async fn cmd_get_thumbnail(
    message_id: i32,
    folder_id: Option<i64>,
    app_handle: tauri::AppHandle,
    state: State<'_, TelegramState>,
    bw_state: State<'_, BandwidthManager>,
    #[allow(unused_variables)]
    db_pool: State<'_, DbConnection>,
) -> Result<String, String> {
    // Check if thumbnail already in cache
    let cache_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("thumbnails");
    if tokio::fs::metadata(&cache_dir).await.is_err() {
        let _ = tokio::fs::create_dir_all(&cache_dir).await;
    }

    // Check for any cached thumbnail for this message by checking predicted paths
    let supported_exts = ["jpg", "png", "gif", "webp"];
    for ext in &supported_exts {
        let path = cache_dir.join(format!("{}.{}", message_id, ext));
        if tokio::fs::metadata(&path).await.is_ok() {
            if let Ok(bytes) = tokio::fs::read(&path).await {
                let mime = match *ext {
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    _ => "image/jpeg",
                };
                let b64 = general_purpose::STANDARD.encode(&bytes);
                return Ok(format!("data:{};base64,{}", mime, b64));
            }
        }
    }

    #[cfg(not(feature = "headless-server"))]
    {
        if crate::local_api::desktop_uses_asset_index(&app_handle, &db_pool).await? {
            return thumbnail_via_local_api(
                &app_handle,
                message_id,
                folder_id,
                cache_dir,
                &db_pool,
                &bw_state,
            )
            .await;
        }
    }

    // No cache, need to fetch from Telegram
    let client_opt = { state.client.lock().await.clone() };
    let Some(client) = client_opt else {
        return Ok("".to_string());
    };

    let peer = resolve_peer(&client, folder_id, &state.peer_cache).await?;
    let messages = client.get_messages_by_id(&peer, &[message_id])
        .await.map_err(|e| e.to_string())?;
    if let Some(m) = messages.into_iter().flatten().next() {
        if let Some(media) = m.media() {
            // Only get thumbnails for photos and documents with photo thumbnails
            let (is_image, ext) = match &media {
                Media::Photo(_) => (true, "jpg".to_string()),
                Media::Document(d) => {
                    let mime = d.mime_type().unwrap_or("");
                    if mime.starts_with("image/") {
                        let e = match mime {
                            "image/png" => "png",
                            "image/gif" => "gif",
                            "image/webp" => "webp",
                            _ => "jpg",
                        };
                        (true, e.to_string())
                    } else {
                        // Not an image, return empty - FileCard will show icon
                        return Ok("".to_string());
                    }
                },
                _ => return Ok("".to_string()),
            };

            if is_image {
                // Get photo thumbnail (smallest size for speed)
                let save_path = cache_dir.join(format!("{}.{}", message_id, ext));
                let save_path_str = save_path.to_string_lossy().to_string();

                let thumbs = match &media {
                    Media::Photo(p) => p.thumbs(),
                    Media::Document(d) => d.thumbs(),
                    _ => vec![],
                };

                let download_success = if let Some(thumb) = thumbs.iter().filter(|t| t.size() > 0).min_by_key(|t| t.size()) {
                    client.download_media(thumb, &save_path_str).await.is_ok()
                } else {
                    client.download_media(&media, &save_path_str).await.is_ok()
                };

                if download_success {
                    // Track bandwidth for thumbnail download
                    if let Ok(meta) = tokio::fs::metadata(&save_path).await {
                        bw_state.add_down(meta.len()).await;
                    }
                    if let Ok(bytes) = tokio::fs::read(&save_path).await {
                        let mime = match ext.as_str() {
                            "png" => "image/png",
                            "gif" => "image/gif",
                            "webp" => "image/webp",
                            _ => "image/jpeg",
                        };
                        let b64 = general_purpose::STANDARD.encode(&bytes);
                        return Ok(format!("data:{};base64,{}", mime, b64));
                    }
                }
            }
        }
    }

    Ok("".to_string())
}

#[tauri::command]
pub async fn cmd_delete_image_thumbnail(
    message_id: i32,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let cache_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("thumbnails");
        
    let _ = tokio::task::spawn_blocking(move || {
        let supported_exts = ["jpg", "png", "gif", "webp"];
        for ext in &supported_exts {
            let path = cache_dir.join(format!("{}.{}", message_id, ext));
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }).await;
    Ok(())
}