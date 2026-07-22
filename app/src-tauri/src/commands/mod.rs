use crate::bot_pool::BotPool;
use grammers_client::types::{LoginToken, PasswordToken, Peer};
use grammers_client::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

const USER_PROBE_SUCCESS_TTL: Duration = Duration::from_secs(30);
const USER_PROBE_FAILURE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
struct UserProbeSnapshot {
    checked_at: Instant,
    user: Option<String>,
}

/// Process-local snapshot for User-mode Telegram readiness.
///
/// The probe lock provides single-flight behavior so an unthrottled readiness
/// endpoint cannot fan out one `get_me()` request per HTTP request.
#[derive(Default)]
pub struct UserProbeCache {
    state: RwLock<Option<UserProbeSnapshot>>,
    probe_lock: Mutex<()>,
}

impl UserProbeCache {
    pub async fn cached(&self) -> Option<Option<String>> {
        let state = self.state.read().await;
        let snapshot = state.as_ref()?;
        let ttl = if snapshot.user.is_some() {
            USER_PROBE_SUCCESS_TTL
        } else {
            USER_PROBE_FAILURE_TTL
        };
        if snapshot.checked_at.elapsed() <= ttl {
            Some(snapshot.user.clone())
        } else {
            None
        }
    }

    pub async fn record(&self, user: Option<String>) {
        *self.state.write().await = Some(UserProbeSnapshot {
            checked_at: Instant::now(),
            user,
        });
    }

    pub async fn acquire_probe_lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.probe_lock.lock().await
    }
}

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
    /// Cached, single-flight User-mode `get_me()` readiness probe.
    pub user_probe_cache: Arc<UserProbeCache>,
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

#[cfg(test)]
mod user_probe_tests {
    use super::*;

    #[tokio::test]
    async fn user_probe_cache_records_success_and_failure() {
        let cache = UserProbeCache::default();
        assert_eq!(cache.cached().await, None);
        cache.record(Some("Ada".to_string())).await;
        assert_eq!(cache.cached().await, Some(Some("Ada".to_string())));
        cache.record(None).await;
        assert_eq!(cache.cached().await, Some(None));
    }
}

pub mod api_settings;
pub mod auth;
pub mod fs;
pub mod network;
#[cfg(feature = "desktop")]
pub mod preview;
#[cfg(feature = "desktop")]
pub mod settings;
#[cfg(feature = "desktop")]
pub mod sharing;
#[cfg(feature = "desktop")]
pub mod streaming;
pub mod utils;

pub use api_settings::*;
pub use auth::*;
pub use fs::*;
pub use network::*;
#[cfg(feature = "desktop")]
pub use preview::*;
#[cfg(feature = "desktop")]
pub use settings::*;
#[cfg(feature = "desktop")]
pub use sharing::*;
#[cfg(feature = "desktop")]
pub use streaming::*;
pub use utils::*;
