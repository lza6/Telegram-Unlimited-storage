use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;
use grammers_client::{Client};
use grammers_client::types::{LoginToken, PasswordToken, Peer};
use crate::bot_pool::BotPool;

/// Tracks the lifecycle of the Telegram connection
///
/// IMPORTANT: The `runner_shutdown` field is critical for preventing stack overflow.
/// When reconnecting, we MUST shutdown the old runner before spawning a new one.
/// Without this, runner tasks accumulate and exhaust the thread stack.
#[derive(Clone)]
pub struct TelegramState {
    pub client: Arc<Mutex<Option<Client>>>,
    pub login_token: Arc<Mutex<Option<LoginToken>>>,
    pub password_token: Arc<Mutex<Option<PasswordToken>>>,
    pub api_id: Arc<Mutex<Option<i32>>>,
    /// Send to this channel to request runner shutdown.
    /// Uses std::sync::Mutex (not tokio) so it can be locked from synchronous
    /// contexts like the RunEvent::Exit handler.
    pub runner_shutdown: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    pub runner_count: Arc<std::sync::atomic::AtomicU32>,
    /// Cache of folder_id → Peer to avoid O(N) dialog scanning on every operation.
    /// Populated lazily on first resolve_peer call, eagerly during cmd_scan_folders.
    /// Cleared on logout.
    pub peer_cache: Arc<tokio::sync::RwLock<HashMap<i64, Peer>>>,
    /// Set of transfer IDs that have been cancelled. Checked cooperatively
    /// in upload/download chunk loops. Cleared on logout.
    pub cancelled_transfers: Arc<tokio::sync::RwLock<HashSet<String>>>,
    /// Bot token pool with FloodWait awareness (shared across all requests).
    /// Initialized from TG_BOT_TOKENS environment variable.
    pub bot_pool: Arc<BotPool>,
}

/// Signal the grammers network runner to exit. Safe from sync or async contexts.
pub fn signal_runner_shutdown(
    runner_shutdown: &Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
) -> bool {
    let mut guard = match runner_shutdown.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(tx) = guard.take() {
        let _ = tx.send(());
        true
    } else {
        false
    }
}

#[cfg(test)]
mod signal_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::oneshot;

    #[test]
    fn signal_runner_shutdown_sends_once() {
        let shutdown = Arc::new(std::sync::Mutex::new(None));
        let (tx, mut rx) = oneshot::channel();
        *shutdown.lock().unwrap() = Some(tx);
        assert!(signal_runner_shutdown(&shutdown));
        assert!(!signal_runner_shutdown(&shutdown));
        rx.try_recv().expect("shutdown signal");
    }
}

pub mod auth;
pub mod fs;
pub mod preview;
pub mod utils;
pub mod network;
pub mod streaming;
pub mod api_settings;
pub mod settings;
pub mod sharing;

pub use auth::*;
pub use fs::*;
pub use preview::*;
pub use utils::*;
pub use network::*;
pub use streaming::*;
pub use api_settings::*;
pub use settings::*;
pub use sharing::*;

