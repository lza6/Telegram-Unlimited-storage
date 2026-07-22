use grammers_client::Client;
use grammers_client::types::Peer;
use tauri::State;
use crate::bandwidth::BandwidthManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Resolve a folder_id to a Telegram Peer, using the cache for O(1) lookups.
///
/// - `folder_id == None` → returns the user's own peer (Saved Messages)
/// - Cache hit → returns immediately without any network call
/// - Cache miss → scans all dialogs, populates the cache, and returns
const DEFAULT_PEER_CACHE_MAX: usize = 500;

pub fn trim_peer_cache(cache: &mut HashMap<i64, Peer>, max: usize) {
    if cache.len() <= max {
        return;
    }
    let excess: Vec<i64> = cache.keys().copied().take(cache.len() - max).collect();
    for k in excess {
        cache.remove(&k);
    }
}

pub async fn resolve_peer(
    client: &Client,
    folder_id: Option<i64>,
    peer_cache: &Arc<RwLock<HashMap<i64, Peer>>>,
) -> Result<Peer, String> {
    resolve_peer_with_limit(client, folder_id, peer_cache, DEFAULT_PEER_CACHE_MAX).await
}

pub async fn resolve_peer_with_limit(
    client: &Client,
    folder_id: Option<i64>,
    peer_cache: &Arc<RwLock<HashMap<i64, Peer>>>,
    max_cache: usize,
) -> Result<Peer, String> {
    if let Some(fid) = folder_id {
        // Fast path: check cache
        {
            let cache = peer_cache.read().await;
            if let Some(peer) = cache.get(&fid) {
                return Ok(peer.clone());
            }
        }

        // Slow path: scan dialogs and populate cache
        log::debug!("Peer cache miss for folder_id={}, scanning dialogs...", fid);
        let mut found: Option<Peer> = None;
        let mut dialogs = client.iter_dialogs();
        let mut discovered = HashMap::new();
        while let Some(dialog) = dialogs.next().await.map_err(|e| e.to_string())? {
            let peer_id = match &dialog.peer {
                Peer::Channel(c) => Some(c.raw.id),
                Peer::User(u) => Some(u.raw.id()),
                _ => None,
            };
            if let Some(id) = peer_id {
                discovered.insert(id, dialog.peer.clone());
                if id == fid {
                    found = Some(dialog.peer.clone());
                    // Don't break — keep scanning to warm the cache
                }
            }
        }

        {
            let mut cache = peer_cache.write().await;
            cache.extend(discovered);
            trim_peer_cache(&mut cache, max_cache.max(100));
        }

        found.ok_or_else(|| format!("Folder/Chat {} not found", fid))
    } else {
        match client.get_me().await {
            Ok(me) => Ok(Peer::User(me)),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// Clear the peer cache (called on logout)
pub async fn clear_peer_cache(peer_cache: &Arc<RwLock<HashMap<i64, Peer>>>) {
    peer_cache.write().await.clear();
}

#[tauri::command]
pub fn cmd_log(message: String) {
    log::info!("[FRONTEND] {}", message);
}

#[tauri::command]
pub async fn cmd_get_bandwidth(bw_state: State<'_, BandwidthManager>) -> Result<crate::bandwidth::BandwidthStats, String> {
    Ok(bw_state.get_stats().await)
}

/// RAII guard that deletes a temp file on drop unless `keep()` is called.
pub struct TempFileGuard {
    path: Option<std::path::PathBuf>,
}

impl TempFileGuard {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path: Some(path) }
    }
    pub fn path(&self) -> &std::path::PathBuf {
        self.path.as_ref().unwrap()
    }
    pub fn keep(mut self) {
        self.path = None;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if let Some(ref p) = self.path {
            let _ = std::fs::remove_file(p);
        }
    }
}

pub fn map_error(e: impl std::fmt::Display) -> String {
    let err_str = e.to_string();
    if err_str.contains("FLOOD_WAIT") {
        // Expected format: ... (value: 1234)
        if let Some(start) = err_str.find("(value: ") {
             let rest = &err_str[start + 8..];
             if let Some(end) = rest.find(')') {
                 if let Ok(seconds) = rest[..end].parse::<i64>() {
                     return format!("FLOOD_WAIT_{}", seconds);
                 }
             }
        }
        // Fallback if parsing fails but we know it's a flood wait
        return "FLOOD_WAIT_60".to_string();
    }
    err_str
}

/// Map Telegram message peer to TD folder_id (`None` = Saved Messages).
pub fn telegram_peer_id_to_folder_id(peer: &grammers_tl_types::enums::Peer) -> Option<i64> {
    match peer {
        grammers_tl_types::enums::Peer::Channel(c) => Some(c.channel_id),
        grammers_tl_types::enums::Peer::User(_) => None,
        grammers_tl_types::enums::Peer::Chat(_) => None,
    }
}

#[cfg(test)]
mod peer_map_tests {
    use super::*;
    use grammers_tl_types as tl;

    #[test]
    fn user_peer_maps_to_saved_messages() {
        let peer = tl::enums::Peer::User(tl::types::PeerUser { user_id: 999 });
        assert_eq!(telegram_peer_id_to_folder_id(&peer), None);
    }

    #[test]
    fn channel_peer_maps_to_folder_id() {
        let peer = tl::enums::Peer::Channel(tl::types::PeerChannel { channel_id: 42 });
        assert_eq!(telegram_peer_id_to_folder_id(&peer), Some(42));
    }

    #[test]
    fn chat_peer_maps_to_saved_messages() {
        let peer = tl::enums::Peer::Chat(tl::types::PeerChat { chat_id: 100 });
        assert_eq!(telegram_peer_id_to_folder_id(&peer), None);
    }
}
