//! Bounded in-flight Telegram uploads (chunk + whole-file slots).
//! Client-side `CHUNK_CONCURRENT` / `FILES_CONCURRENT` only throttle browsers;
//! this gate protects the server under many simultaneous clients.
//!
//! Backends:
//! - **memory** — in-process `tokio::sync::Semaphore` (single replica)
//! - **redis** — shared counters in Redis (`REDIS_URL`) for multi-replica 7×24 deploys

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use actix_web::HttpResponse;
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Max time a request waits in the upload queue before returning 503.
/// Kept under Actix's `client_request_timeout` (default 120s) so the client
/// connection survives the wait and the upload still has headroom to run.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(90);

/// Consecutive Redis failures before marking the distributed backend degraded.
/// Redis mode remains fail-closed throughout the outage.
const REDIS_FAIL_THRESHOLD: u32 = 3;

const REDIS_CHUNK_IN_USE_KEY: &str = "td:upload:chunk:leases";
const REDIS_FILE_IN_USE_KEY: &str = "td:upload:file:leases";
/// Permit leases are reclaimed after a process crash. The hash is refreshed
/// on every acquire; normal uploads are much shorter than this window.
const REDIS_PERMIT_TTL_SECS: i64 = 86_400;

const REDIS_ACQUIRE_LUA: &str = r#"
local now = tonumber(redis.call('TIME')[1])
local max = tonumber(ARGV[1])
local token = ARGV[2]
local ttl = tonumber(ARGV[3])
for _, existing in ipairs(redis.call('HKEYS', KEYS[1])) do
  local expires = tonumber(redis.call('HGET', KEYS[1], existing) or '0')
  if expires <= now then
    redis.call('HDEL', KEYS[1], existing)
  end
end
if redis.call('HLEN', KEYS[1]) >= max then
  return 0
end
redis.call('HSET', KEYS[1], token, now + ttl)
redis.call('EXPIRE', KEYS[1], ttl + 60)
return 1
"#;

const REDIS_RELEASE_LUA: &str = r#"
return redis.call('HDEL', KEYS[1], ARGV[1])
"#;

#[derive(Clone, Debug, Serialize)]
pub struct UploadQueueStatus {
    pub chunk_slots_total: u32,
    pub chunk_slots_available: u32,
    pub file_slots_total: u32,
    pub file_slots_available: u32,
}

/// In-process memory backend — always present as the failover target.
struct MemoryBackend {
    chunk: Arc<Semaphore>,
    file: Arc<Semaphore>,
    chunk_total: u32,
    file_total: u32,
}

impl MemoryBackend {
    fn new(chunk_concurrent: u32, files_concurrent: u32) -> Self {
        let chunk_total = chunk_concurrent.max(1);
        let file_total = files_concurrent.max(1);
        Self {
            chunk: Arc::new(Semaphore::new(chunk_total as usize)),
            file: Arc::new(Semaphore::new(file_total as usize)),
            chunk_total,
            file_total,
        }
    }

    fn try_acquire_chunk(&self) -> Option<ChunkPermit> {
        self.chunk
            .clone()
            .try_acquire_owned()
            .ok()
            .map(|p| ChunkPermit {
                _inner: PermitInner::MemoryChunk(p),
            })
    }

    fn try_acquire_file(&self) -> Option<FilePermit> {
        self.file
            .clone()
            .try_acquire_owned()
            .ok()
            .map(|p| FilePermit {
                _inner: PermitInner::MemoryFile(p),
            })
    }
}

/// Outcome of an acquire attempt against Redis.
/// `Full` = slot saturated (legitimate reject); `Error` = Redis unreachable.
enum AcquireOutcome {
    Acquired(String),
    Full,
    Error,
}

struct RedisGate {
    client: redis::Client,
    chunk_total: u32,
    file_total: u32,
}

impl RedisGate {
    fn new(redis_url: &str, chunk_total: u32, file_total: u32) -> Result<Self, String> {
        let client = redis::Client::open(redis_url).map_err(|e| e.to_string())?;
        let mut conn = client
            .get_connection()
            .map_err(|e| format!("redis connect: {e}"))?;
        let _: String = redis::cmd("PING")
            .query(&mut conn)
            .map_err(|e| format!("redis ping: {e}"))?;
        let _: i32 = redis::cmd("EVAL")
            .arg("return 1")
            .arg(0)
            .query(&mut conn)
            .map_err(|e| format!("redis eval permission: {e}"))?;
        log::info!(
            "UploadGate: redis backend active (chunk_max={}, file_max={}, keys {}/{})",
            chunk_total,
            file_total,
            REDIS_CHUNK_IN_USE_KEY,
            REDIS_FILE_IN_USE_KEY
        );
        Ok(Self {
            client,
            chunk_total: chunk_total.max(1),
            file_total: file_total.max(1),
        })
    }

    fn try_acquire_key(&self, key: &str, max: u32) -> AcquireOutcome {
        let mut conn = match self.client.get_connection() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("redis acquire connection failed: {e}");
                return AcquireOutcome::Error;
            }
        };
        let token = uuid::Uuid::new_v4().to_string();
        let ok: i32 = match redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(key)
            .arg(max)
            .arg(&token)
            .arg(REDIS_PERMIT_TTL_SECS)
            .invoke(&mut conn)
        {
            Ok(value) => value,
            Err(e) => {
                log::warn!("redis acquire script failed for {key}: {e}");
                return AcquireOutcome::Error;
            }
        };
        if ok == 1 {
            AcquireOutcome::Acquired(token)
        } else {
            AcquireOutcome::Full
        }
    }

    fn in_use(&self, key: &str) -> u32 {
        let Ok(mut conn) = self.client.get_connection() else {
            return 0;
        };
        redis::cmd("HLEN").arg(key).query(&mut conn).unwrap_or(0)
    }

    /// Lightweight liveness probe (PING). Used to detect Redis recovery and
    /// re-enable the Redis backend after a failover to memory.
    fn ping(&self) -> bool {
        self.client
            .get_connection()
            .and_then(|mut c| {
                let pong: String = redis::cmd("PING").query(&mut c)?;
                Ok(pong)
            })
            .is_ok()
    }

    /// `Err` = Redis unreachable (caller may failover); `Ok(None)` = saturated.
    fn try_acquire_chunk(&self) -> Result<Option<ChunkPermit>, ()> {
        match self.try_acquire_key(REDIS_CHUNK_IN_USE_KEY, self.chunk_total) {
            AcquireOutcome::Acquired(token) => Ok(Some(ChunkPermit {
                _inner: PermitInner::RedisChunk {
                    release: RedisRelease {
                        client: self.client.clone(),
                        key: REDIS_CHUNK_IN_USE_KEY.to_string(),
                        token,
                    },
                },
            })),
            AcquireOutcome::Full => Ok(None),
            AcquireOutcome::Error => Err(()),
        }
    }

    fn try_acquire_file(&self) -> Result<Option<FilePermit>, ()> {
        match self.try_acquire_key(REDIS_FILE_IN_USE_KEY, self.file_total) {
            AcquireOutcome::Acquired(token) => Ok(Some(FilePermit {
                _inner: PermitInner::RedisFile {
                    release: RedisRelease {
                        client: self.client.clone(),
                        key: REDIS_FILE_IN_USE_KEY.to_string(),
                        token,
                    },
                },
            })),
            AcquireOutcome::Full => Ok(None),
            AcquireOutcome::Error => Err(()),
        }
    }

    fn status(&self) -> UploadQueueStatus {
        let chunk_in_use = self.in_use(REDIS_CHUNK_IN_USE_KEY);
        let file_in_use = self.in_use(REDIS_FILE_IN_USE_KEY);
        UploadQueueStatus {
            chunk_slots_total: self.chunk_total,
            chunk_slots_available: self.chunk_total.saturating_sub(chunk_in_use),
            file_slots_total: self.file_total,
            file_slots_available: self.file_total.saturating_sub(file_in_use),
        }
    }
}

struct RedisRelease {
    client: redis::Client,
    key: String,
    token: String,
}

impl RedisRelease {
    fn release_key(&self) {
        if let Ok(mut conn) = self.client.get_connection() {
            if let Err(e) = redis::Script::new(REDIS_RELEASE_LUA)
                .key(&self.key)
                .arg(&self.token)
                .invoke::<i32>(&mut conn)
            {
                log::warn!("redis permit release failed for {}: {e}", self.key);
            }
        } else {
            log::warn!("redis permit release connection failed for {}", self.key);
        }
    }
}

enum PermitInner {
    MemoryChunk(OwnedSemaphorePermit),
    MemoryFile(OwnedSemaphorePermit),
    RedisChunk { release: RedisRelease },
    RedisFile { release: RedisRelease },
}

impl Drop for PermitInner {
    fn drop(&mut self) {
        match self {
            PermitInner::RedisChunk { release } | PermitInner::RedisFile { release } => {
                release.release_key();
            }
            _ => {}
        }
    }
}

pub struct ChunkPermit {
    _inner: PermitInner,
}

pub struct FilePermit {
    _inner: PermitInner,
}

/// Upload gate with an explicit backend contract.
///
/// When Redis is configured and healthy, slots are tracked in Redis so that
/// multiple replicas share a global cap. If Redis becomes unreachable, Redis
/// mode rejects new work until the backend recovers; it never silently falls
/// back to process-local permits. Memory mode is an explicit single-node
/// deployment choice. A periodic liveness probe re-enables Redis once it
/// recovers.
pub struct UploadGate {
    memory: MemoryBackend,
    redis: Option<Arc<RedisGate>>,
    /// Whether Redis is currently considered usable.
    redis_healthy: AtomicBool,
    /// Consecutive Redis failures; reset on success or successful ping.
    redis_fail_count: AtomicU32,
    /// Redis mode is an explicit distributed contract. It must never silently
    /// fall back to process-local permits, because that would over-admit when
    /// more than one API replica is running.
    redis_required: bool,
}

pub fn build_upload_gate(config: &crate::server_config::ServerConfig) -> UploadGate {
    let mem = MemoryBackend::new(config.chunk_concurrent, config.files_concurrent);
    if config.upload_queue_backend == "redis" {
        if let Some(url) = config.redis_url.as_deref() {
            match RedisGate::new(url, config.chunk_concurrent, config.files_concurrent) {
                Ok(gate) => {
                    return UploadGate {
                        memory: mem,
                        redis: Some(Arc::new(gate)),
                        redis_healthy: AtomicBool::new(true),
                        redis_fail_count: AtomicU32::new(0),
                        redis_required: true,
                    };
                }
                Err(e) => {
                    log::error!(
                        "Redis UploadGate init failed ({e}); refusing uploads until Redis recovers"
                    );
                    return UploadGate {
                        memory: mem,
                        redis: None,
                        redis_healthy: AtomicBool::new(false),
                        redis_fail_count: AtomicU32::new(REDIS_FAIL_THRESHOLD),
                        redis_required: true,
                    };
                }
            }
        } else {
            log::error!("UPLOAD_QUEUE_BACKEND=redis but REDIS_URL unset — refusing uploads");
            return UploadGate {
                memory: mem,
                redis: None,
                redis_healthy: AtomicBool::new(false),
                redis_fail_count: AtomicU32::new(REDIS_FAIL_THRESHOLD),
                redis_required: true,
            };
        }
    }
    UploadGate::new_memory(config.chunk_concurrent, config.files_concurrent)
}

impl UploadGate {
    pub fn new(chunk_concurrent: u32, files_concurrent: u32) -> Self {
        Self::new_memory(chunk_concurrent, files_concurrent)
    }

    pub fn new_memory(chunk_concurrent: u32, files_concurrent: u32) -> Self {
        UploadGate {
            memory: MemoryBackend::new(chunk_concurrent, files_concurrent),
            redis: None,
            redis_healthy: AtomicBool::new(false),
            redis_fail_count: AtomicU32::new(0),
            redis_required: false,
        }
    }

    /// True when the distributed backend is currently usable.
    fn redis_active(&self) -> bool {
        self.redis.is_some() && self.redis_healthy.load(Ordering::Relaxed)
    }

    /// Whether the configured queue backend can currently admit work.
    /// Explicit memory mode is ready locally; Redis mode is ready only while
    /// a Redis client exists and the liveness state is healthy.
    pub fn ready(&self) -> bool {
        if self.redis_required {
            self.redis_active()
        } else {
            true
        }
    }

    /// Mark a Redis failure and expose a degraded state for readiness metrics.
    fn note_redis_failure(&self) {
        let count = self.redis_fail_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= REDIS_FAIL_THRESHOLD && self.redis_healthy.load(Ordering::Relaxed) {
            self.redis_healthy.store(false, Ordering::Relaxed);
            log::warn!(
                "UploadGate: Redis unreachable after {count} consecutive failures — \
                 refusing new uploads until Redis recovers."
            );
        }
    }

    /// Mark a Redis success; clears failure count and re-enables the backend.
    fn note_redis_success(&self) {
        let was_down = !self.redis_healthy.load(Ordering::Relaxed);
        self.redis_fail_count.store(0, Ordering::Relaxed);
        if was_down {
            self.redis_healthy.store(true, Ordering::Relaxed);
            log::info!("UploadGate: Redis recovered — switching back to redis backend");
        }
    }

    async fn redis_acquire_chunk_async(gate: Arc<RedisGate>) -> Result<Option<ChunkPermit>, ()> {
        tokio::task::spawn_blocking(move || gate.try_acquire_chunk())
            .await
            .unwrap_or(Err(()))
    }

    async fn redis_acquire_file_async(gate: Arc<RedisGate>) -> Result<Option<FilePermit>, ()> {
        tokio::task::spawn_blocking(move || gate.try_acquire_file())
            .await
            .unwrap_or(Err(()))
    }

    async fn probe_redis_recovery_async(gate: Arc<RedisGate>) -> bool {
        tokio::task::spawn_blocking(move || gate.ping())
            .await
            .unwrap_or(false)
    }

    pub fn status(&self) -> UploadQueueStatus {
        if self.redis_active() {
            if let Some(g) = &self.redis {
                return g.status();
            }
        }
        if self.redis_required {
            return UploadQueueStatus {
                chunk_slots_total: self.memory.chunk_total,
                chunk_slots_available: 0,
                file_slots_total: self.memory.file_total,
                file_slots_available: 0,
            };
        }
        UploadQueueStatus {
            chunk_slots_total: self.memory.chunk_total,
            chunk_slots_available: self.memory.chunk.available_permits() as u32,
            file_slots_total: self.memory.file_total,
            file_slots_available: self.memory.file.available_permits() as u32,
        }
    }

    /// Non-blocking health/readiness snapshot.
    ///
    /// The legacy Redis backend can only discover the distributed in-use count
    /// through synchronous network I/O. Public health endpoints must never do
    /// that work on an Actix worker, so Redis availability is reported
    /// conservatively until the async lease backend maintains an in-memory
    /// snapshot. Memory mode remains exact.
    pub fn status_snapshot(&self) -> UploadQueueStatus {
        if let Some(redis) = &self.redis {
            return UploadQueueStatus {
                chunk_slots_total: redis.chunk_total,
                chunk_slots_available: 0,
                file_slots_total: redis.file_total,
                file_slots_available: 0,
            };
        }
        if self.redis_required {
            return UploadQueueStatus {
                chunk_slots_total: self.memory.chunk_total,
                chunk_slots_available: 0,
                file_slots_total: self.memory.file_total,
                file_slots_available: 0,
            };
        }
        UploadQueueStatus {
            chunk_slots_total: self.memory.chunk_total,
            chunk_slots_available: self.memory.chunk.available_permits() as u32,
            file_slots_total: self.memory.file_total,
            file_slots_available: self.memory.file.available_permits() as u32,
        }
    }

    /// Fast reject when saturated (load balancer / many clients).
    pub fn try_acquire_chunk(&self) -> Option<ChunkPermit> {
        if self.redis_active() {
            if let Some(g) = &self.redis {
                return match g.try_acquire_chunk() {
                    Ok(opt) => {
                        self.note_redis_success();
                        opt
                    }
                    Err(()) => {
                        self.note_redis_failure();
                        None
                    }
                };
            }
        }
        if self.redis_required {
            return None;
        }
        self.memory.try_acquire_chunk()
    }

    pub fn try_acquire_file(&self) -> Option<FilePermit> {
        if self.redis_active() {
            if let Some(g) = &self.redis {
                return match g.try_acquire_file() {
                    Ok(opt) => {
                        self.note_redis_success();
                        opt
                    }
                    Err(()) => {
                        self.note_redis_failure();
                        None
                    }
                };
            }
        }
        if self.redis_required {
            return None;
        }
        self.memory.try_acquire_file()
    }

    /// Wait up to `ACQUIRE_TIMEOUT` for a chunk slot (same file may retry many chunks).
    /// Prefers the Redis backend (global cap across replicas); Redis errors are
    /// retried until the bounded wait expires and never fall back to memory.
    pub async fn acquire_chunk(&self) -> Option<ChunkPermit> {
        if self.redis_required && self.redis.is_none() {
            return None;
        }
        if self.redis.is_some() {
            let deadline = tokio::time::Instant::now() + ACQUIRE_TIMEOUT;
            let mut probe_tick = 0u32;
            loop {
                if self.redis_active() {
                    if let Some(g) = &self.redis {
                        match Self::redis_acquire_chunk_async(g.clone()).await {
                            Ok(Some(p)) => {
                                self.note_redis_success();
                                return Some(p);
                            }
                            Ok(None) => {} // saturated — keep waiting
                            Err(()) => self.note_redis_failure(),
                        }
                    }
                }
                if !self.redis_active() {
                    // While degraded, periodically probe Redis for recovery
                    // (every ~5s of 200ms ticks) before retrying admission.
                    probe_tick = probe_tick.saturating_add(1);
                    if probe_tick % 25 == 0 {
                        let recovered = match self.redis.clone() {
                            Some(gate) => Self::probe_redis_recovery_async(gate).await,
                            None => false,
                        };
                        if recovered {
                            self.redis_healthy.store(true, Ordering::Relaxed);
                            self.redis_fail_count.store(0, Ordering::Relaxed);
                            log::info!("UploadGate: Redis liveness probe succeeded — re-enabling redis backend");
                        }
                        if recovered {
                            continue; // retry Redis acquire this iteration
                        }
                    }
                    if self.redis_required {
                        if tokio::time::Instant::now() >= deadline {
                            return None;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                    // Block on memory backend only in explicit memory mode.
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    return match tokio::time::timeout(
                        remaining,
                        self.memory.chunk.clone().acquire_owned(),
                    )
                    .await
                    {
                        Ok(Ok(p)) => Some(ChunkPermit {
                            _inner: PermitInner::MemoryChunk(p),
                        }),
                        _ => None,
                    };
                }
                if tokio::time::Instant::now() >= deadline {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        match tokio::time::timeout(ACQUIRE_TIMEOUT, self.memory.chunk.clone().acquire_owned()).await
        {
            Ok(Ok(p)) => Some(ChunkPermit {
                _inner: PermitInner::MemoryChunk(p),
            }),
            _ => None,
        }
    }

    pub async fn acquire_file(&self) -> Option<FilePermit> {
        if self.redis_required && self.redis.is_none() {
            return None;
        }
        if self.redis.is_some() {
            let deadline = tokio::time::Instant::now() + ACQUIRE_TIMEOUT;
            let mut probe_tick = 0u32;
            loop {
                if self.redis_active() {
                    if let Some(g) = &self.redis {
                        match Self::redis_acquire_file_async(g.clone()).await {
                            Ok(Some(p)) => {
                                self.note_redis_success();
                                return Some(p);
                            }
                            Ok(None) => {}
                            Err(()) => self.note_redis_failure(),
                        }
                    }
                }
                if !self.redis_active() {
                    probe_tick = probe_tick.saturating_add(1);
                    if probe_tick % 25 == 0 {
                        let recovered = match self.redis.clone() {
                            Some(gate) => Self::probe_redis_recovery_async(gate).await,
                            None => false,
                        };
                        if recovered {
                            self.redis_healthy.store(true, Ordering::Relaxed);
                            self.redis_fail_count.store(0, Ordering::Relaxed);
                        }
                        if recovered {
                            continue;
                        }
                    }
                    if self.redis_required {
                        if tokio::time::Instant::now() >= deadline {
                            return None;
                        }
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    return match tokio::time::timeout(
                        remaining,
                        self.memory.file.clone().acquire_owned(),
                    )
                    .await
                    {
                        Ok(Ok(p)) => Some(FilePermit {
                            _inner: PermitInner::MemoryFile(p),
                        }),
                        _ => None,
                    };
                }
                if tokio::time::Instant::now() >= deadline {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        match tokio::time::timeout(ACQUIRE_TIMEOUT, self.memory.file.clone().acquire_owned()).await
        {
            Ok(Ok(p)) => Some(FilePermit {
                _inner: PermitInner::MemoryFile(p),
            }),
            _ => None,
        }
    }
}

pub fn response_upload_busy(retry_after_secs: u32) -> HttpResponse {
    HttpResponse::ServiceUnavailable()
        .insert_header(("Retry-After", retry_after_secs.to_string()))
        .json(serde_json::json!({
            "error": {
                "code": "UPLOAD_QUEUE_FULL",
                "message": "Upload capacity saturated; retry after backoff"
            }
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_limits_chunk_slots() {
        let gate = UploadGate::new_memory(2, 1);
        let _a = gate.try_acquire_chunk().expect("first");
        let _b = gate.try_acquire_chunk().expect("second");
        assert!(gate.try_acquire_chunk().is_none());
        let s = gate.status();
        assert_eq!(s.chunk_slots_available, 0);
        assert_eq!(gate.status_snapshot().chunk_slots_available, 0);
    }

    #[test]
    fn build_upload_gate_uses_config_limits() {
        let cfg = crate::server_config::test_config();
        let gate = build_upload_gate(&cfg);
        let s = gate.status();
        assert_eq!(s.chunk_slots_total, cfg.chunk_concurrent);
    }

    #[test]
    fn build_falls_back_to_memory_when_redis_unreachable_at_startup() {
        // Point at a port where nothing listens: RedisGate::new fails closed.
        let cfg = crate::server_config::ServerConfig {
            upload_queue_backend: "redis".to_string(),
            redis_url: Some("redis://127.0.0.1:9/0".to_string()),
            ..(*crate::server_config::test_config()).clone()
        };
        let gate = build_upload_gate(&cfg);
        assert!(
            gate.redis.is_none(),
            "must not fall back to memory when redis is unreachable at startup"
        );
        assert!(!gate.redis_active());
        assert!(gate.redis_required);
        assert!(!gate.ready());
        assert!(gate.try_acquire_chunk().is_none());
        assert_eq!(gate.status_snapshot().chunk_slots_available, 0);
    }

    #[tokio::test]
    async fn redis_required_startup_failure_rejects_async_acquire() {
        let cfg = crate::server_config::ServerConfig {
            upload_queue_backend: "redis".to_string(),
            redis_url: Some("redis://127.0.0.1:9/0".to_string()),
            ..(*crate::server_config::test_config()).clone()
        };
        let gate = build_upload_gate(&cfg);
        assert!(gate.acquire_chunk().await.is_none());
        assert!(gate.acquire_file().await.is_none());
    }

    #[test]
    fn memory_only_gate_never_uses_redis() {
        let gate = UploadGate::new_memory(3, 2);
        assert!(gate.redis.is_none());
        assert!(!gate.redis_active());
        let s = gate.status();
        assert_eq!(s.chunk_slots_total, 3);
        assert_eq!(s.file_slots_total, 2);
    }

    #[test]
    fn redis_lua_acquire_release_roundtrip() {
        let client = match redis::Client::open("redis://127.0.0.1:6379/15") {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut conn = match client.get_connection() {
            Ok(c) => c,
            Err(_) => return,
        };
        let test_key = format!("td:test:gate:{}", uuid::Uuid::new_v4());
        let token1 = uuid::Uuid::new_v4().to_string();
        let token2 = uuid::Uuid::new_v4().to_string();
        let _: () = redis::cmd("DEL")
            .arg(&test_key)
            .query(&mut conn)
            .ok()
            .unwrap_or(());

        let ok: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(&test_key)
            .arg(2)
            .arg(&token1)
            .arg(REDIS_PERMIT_TTL_SECS)
            .invoke(&mut conn)
            .unwrap();
        assert_eq!(ok, 1);

        let ok2: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(&test_key)
            .arg(2)
            .arg(&token2)
            .arg(REDIS_PERMIT_TTL_SECS)
            .invoke(&mut conn)
            .unwrap();
        assert_eq!(ok2, 1);

        let fail: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(&test_key)
            .arg(2)
            .arg(uuid::Uuid::new_v4().to_string())
            .arg(REDIS_PERMIT_TTL_SECS)
            .invoke(&mut conn)
            .unwrap();
        assert_eq!(fail, 0);

        let _: i32 = redis::Script::new(REDIS_RELEASE_LUA)
            .key(&test_key)
            .arg(&token1)
            .invoke(&mut conn)
            .unwrap();

        let _: () = redis::cmd("DEL")
            .arg(&test_key)
            .query(&mut conn)
            .ok()
            .unwrap_or(());
    }
}
