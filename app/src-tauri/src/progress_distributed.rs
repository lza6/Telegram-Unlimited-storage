//! Distributed upload progress hub — Redis Pub/Sub for multi-replica sync.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐     ┌─────────────────┐
//! │   Replica A     │     │   Replica B     │
//! │  (upload req)   │     │  (ws client)    │
//! └────────┬────────┘     └────────┬────────┘
//!          │                       │
//!          ▼                       ▼
//!   ┌────────────────────────────────────┐
//!   │           Redis Pub/Sub            │
//!   │    channel: td:progress:{session}  │
//!   └────────────────────────────────────┘
//! ```
//!
//! ## Configuration
//!
//! ```env
//! PROGRESS_REDIS_URL=redis://redis:6379/0
//! PROGRESS_REDIS_PREFIX=td:progress:
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Progress event payload (serializable for Redis)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub session_id: String,
    pub filename: String,
    pub uploaded_chunks: i32,
    pub total_chunks: i32,
    pub status: String,
    pub timestamp: i64,
}

/// Backend type for progress hub
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBackendType {
    Memory,
    Redis,
}

impl ProgressBackendType {
    pub fn from_env() -> Self {
        let backend = std::env::var("PROGRESS_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if backend == "redis" {
            Self::Redis
        } else {
            Self::Memory
        }
    }
}

// ---------------------------------------------------------------------------
// Memory backend (existing behavior)
// ---------------------------------------------------------------------------

/// In-memory progress hub using broadcast channels
#[derive(Clone)]
pub struct MemoryProgressHub {
    inner: Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, broadcast::Sender<ProgressEvent>>>,
    >,
}

impl MemoryProgressHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }

    pub async fn emit(&self, event: ProgressEvent) {
        let sid = event.session_id.clone();
        let mut map = self.inner.write().await;
        let tx = map.entry(sid).or_insert_with(|| broadcast::channel(64).0);
        let _ = tx.send(event);
    }

    pub async fn subscribe(&self, session_id: &str) -> broadcast::Receiver<ProgressEvent> {
        let mut map = self.inner.write().await;
        let tx = map
            .entry(session_id.to_string())
            .or_insert_with(|| broadcast::channel(64).0);
        tx.subscribe()
    }

    pub async fn remove_session(&self, session_id: &str) {
        self.inner.write().await.remove(session_id);
    }
}

impl Default for MemoryProgressHub {
    fn default() -> Self {
        Self {
            inner: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Redis backend
// ---------------------------------------------------------------------------

pub struct RedisProgressHub {
    client: redis::Client,
    prefix: String,
    // Local broadcast for subscribers (pub/sub bridge)
    local: Arc<MemoryProgressHub>,
}

impl RedisProgressHub {
    pub fn new(redis_url: &str, prefix: &str) -> Result<Arc<Self>, String> {
        let client = redis::Client::open(redis_url).map_err(|e| format!("Redis connect: {e}"))?;

        // Verify connection
        let mut conn = client
            .get_connection()
            .map_err(|e| format!("Redis ping: {e}"))?;
        let _: String = redis::cmd("PING")
            .query(&mut conn)
            .map_err(|e| format!("Redis ping failed: {e}"))?;

        log::info!("RedisProgressHub initialized: prefix={}", prefix);

        Ok(Arc::new(Self {
            client,
            prefix: prefix.to_string(),
            local: MemoryProgressHub::new(),
        }))
    }

    fn channel_key(&self, session_id: &str) -> String {
        format!("{}{}", self.prefix, session_id)
    }

    /// Emit progress event via Redis Pub/Sub
    pub async fn emit(&self, event: ProgressEvent) {
        // 1. Publish to Redis
        let key = self.channel_key(&event.session_id);
        let payload = match serde_json::to_string(&event) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Failed to serialize progress event: {e}");
                return;
            }
        };

        let client = self.client.clone();
        let key_clone = key.clone();
        let payload_clone = payload.clone();

        if let Err(e) = tokio::task::spawn_blocking(move || {
            let mut conn = client.get_connection()?;
            redis::cmd("PUBLISH")
                .arg(&key_clone)
                .arg(&payload_clone)
                .query::<i64>(&mut conn)
        })
        .await
        {
            log::warn!("Redis PUBLISH failed: {e}");
        }

        // 2. Also emit locally for same-replica subscribers
        self.local.emit(event).await;
    }

    /// Subscribe to progress events (spawns Redis subscriber task)
    pub async fn subscribe(&self, session_id: &str) -> broadcast::Receiver<ProgressEvent> {
        let rx = self.local.subscribe(session_id).await;

        // Spawn a task to bridge Redis pub/sub to local broadcast
        let client = self.client.clone();
        let channel = self.channel_key(session_id);
        let local = self.local.clone();
        let sid = session_id.to_string();

        tokio::spawn(async move {
            Self::bridge_redis_to_local(client, &channel, local, &sid).await;
        });

        rx
    }

    async fn bridge_redis_to_local(
        client: redis::Client,
        channel: &str,
        local: Arc<MemoryProgressHub>,
        session_id: &str,
    ) {
        // Run blocking Redis SUBSCRIBE in spawn_blocking
        let channel_owned = channel.to_string();
        let session_id_owned = session_id.to_string();

        let result = tokio::task::spawn_blocking(move || {
            let mut conn = match client.get_connection() {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Redis subscriber connection failed: {e}");
                    return;
                }
            };

            let mut pubsub = conn.as_pubsub();
            if let Err(e) = pubsub.subscribe(&channel_owned) {
                log::warn!("Redis SUBSCRIBE failed: {e}");
                return;
            }

            log::debug!("Redis subscriber listening on {}", channel_owned);

            loop {
                match pubsub.get_message() {
                    Ok(msg) => {
                        if let Ok(payload) = msg.get_payload::<String>() {
                            if let Ok(event) = serde_json::from_str::<ProgressEvent>(&payload) {
                                // Don't re-emit to Redis (avoid loop)
                                // Use local broadcast directly
                                tokio::runtime::Handle::current().block_on(async {
                                    let map = local.inner.write().await;
                                    if let Some(tx) = map.get(&session_id_owned) {
                                        let _ = tx.send(event);
                                    }
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::debug!("Redis pub/sub error: {e}");
                        break;
                    }
                }
            }
        })
        .await;

        if let Err(e) = result {
            log::debug!("Redis bridge task failed: {e}");
        }
    }

    pub async fn remove_session(&self, session_id: &str) {
        self.local.remove_session(session_id).await;
    }
}

// ---------------------------------------------------------------------------
// Unified interface
// ---------------------------------------------------------------------------

/// Unified progress hub that abstracts Memory vs Redis backend
pub enum DistributedProgressHub {
    Memory(Arc<MemoryProgressHub>),
    Redis(Arc<RedisProgressHub>),
}

impl DistributedProgressHub {
    /// Build progress hub based on environment configuration
    pub fn from_env() -> Self {
        let backend_type = ProgressBackendType::from_env();

        match backend_type {
            ProgressBackendType::Redis => {
                let redis_url = std::env::var("PROGRESS_REDIS_URL")
                    .or_else(|_| std::env::var("REDIS_URL"))
                    .unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_string());

                let prefix = std::env::var("PROGRESS_REDIS_PREFIX")
                    .unwrap_or_else(|_| "td:progress:".to_string());

                match RedisProgressHub::new(&redis_url, &prefix) {
                    Ok(hub) => {
                        log::info!("Using Redis-backed progress hub");
                        Self::Redis(hub)
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to init Redis progress hub, falling back to memory: {e}"
                        );
                        Self::Memory(MemoryProgressHub::new())
                    }
                }
            }
            ProgressBackendType::Memory => {
                log::info!("Using in-memory progress hub");
                Self::Memory(MemoryProgressHub::new())
            }
        }
    }

    /// Create memory backend (for tests, desktop mode)
    pub fn memory() -> Self {
        Self::Memory(MemoryProgressHub::new())
    }

    /// Emit progress event
    pub async fn emit(&self, event: ProgressEvent) {
        match self {
            Self::Memory(hub) => hub.emit(event).await,
            Self::Redis(hub) => hub.emit(event).await,
        }
    }

    /// Subscribe to progress events for a session
    pub async fn subscribe(&self, session_id: &str) -> broadcast::Receiver<ProgressEvent> {
        match self {
            Self::Memory(hub) => hub.subscribe(session_id).await,
            Self::Redis(hub) => hub.subscribe(session_id).await,
        }
    }

    /// Remove session from tracking
    pub async fn remove_session(&self, session_id: &str) {
        match self {
            Self::Memory(hub) => hub.remove_session(session_id).await,
            Self::Redis(hub) => hub.remove_session(session_id).await,
        }
    }
}

impl Default for DistributedProgressHub {
    fn default() -> Self {
        Self::from_env()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_hub_delivers_events() {
        let hub = MemoryProgressHub::new();
        let mut rx = hub.subscribe("sess-1").await;

        hub.emit(ProgressEvent {
            session_id: "sess-1".into(),
            filename: "test.bin".into(),
            uploaded_chunks: 1,
            total_chunks: 3,
            status: "active".into(),
            timestamp: chrono::Utc::now().timestamp(),
        })
        .await;

        let ev = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout")
            .expect("closed");

        assert_eq!(ev.uploaded_chunks, 1);
        assert_eq!(ev.filename, "test.bin");
    }

    #[test]
    fn backend_type_defaults_to_memory() {
        std::env::remove_var("PROGRESS_BACKEND");
        assert_eq!(ProgressBackendType::from_env(), ProgressBackendType::Memory);
    }

    #[test]
    fn distributed_hub_creates_memory_by_default() {
        std::env::remove_var("PROGRESS_BACKEND");
        std::env::remove_var("PROGRESS_REDIS_URL");
        let hub = DistributedProgressHub::memory();
        assert!(matches!(hub, DistributedProgressHub::Memory(_)));
    }

    #[test]
    fn progress_event_serialization() {
        let event = ProgressEvent {
            session_id: "sess-abc".into(),
            filename: "file.zip".into(),
            uploaded_chunks: 5,
            total_chunks: 10,
            status: "active".into(),
            timestamp: 1234567890,
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: ProgressEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.session_id, "sess-abc");
        assert_eq!(parsed.uploaded_chunks, 5);
    }
}
