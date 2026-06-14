//! Bounded in-flight Telegram uploads (chunk + whole-file slots).
//! Client-side `CHUNK_CONCURRENT` / `FILES_CONCURRENT` only throttle browsers;
//! this gate protects the server under many simultaneous clients.
//!
//! Backends:
//! - **memory** — in-process `tokio::sync::Semaphore` (single replica)
//! - **redis** — shared counters in Redis (`REDIS_URL`) for multi-replica 7×24 deploys

use std::sync::Arc;
use std::time::Duration;

use actix_web::HttpResponse;
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(300);

const REDIS_CHUNK_IN_USE_KEY: &str = "td:upload:chunk:in_use";
const REDIS_FILE_IN_USE_KEY: &str = "td:upload:file:in_use";

const REDIS_ACQUIRE_LUA: &str = r#"
local in_use = tonumber(redis.call('GET', KEYS[1]) or '0')
local max = tonumber(ARGV[1])
if in_use >= max then
  return 0
end
redis.call('INCR', KEYS[1])
return 1
"#;

const REDIS_RELEASE_LUA: &str = r#"
local in_use = tonumber(redis.call('GET', KEYS[1]) or '0')
if in_use <= 0 then
  redis.call('SET', KEYS[1], 0)
  return 0
end
return redis.call('DECR', KEYS[1])
"#;

#[derive(Clone, Debug, Serialize)]
pub struct UploadQueueStatus {
    pub chunk_slots_total: u32,
    pub chunk_slots_available: u32,
    pub file_slots_total: u32,
    pub file_slots_available: u32,
}

enum GateBackend {
    Memory {
        chunk: Arc<Semaphore>,
        file: Arc<Semaphore>,
        chunk_total: u32,
        file_total: u32,
    },
    Redis(Arc<RedisGate>),
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

    fn try_acquire_key(&self, key: &str, max: u32) -> bool {
        let mut conn = match self.client.get_connection() {
            Ok(c) => c,
            Err(e) => {
                log::warn!("redis acquire connection failed: {e}");
                return false;
            }
        };
        let ok: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(key)
            .arg(max)
            .invoke(&mut conn)
            .unwrap_or(0);
        ok == 1
    }

    fn release_key(&self, key: &str) {
        if let Ok(mut conn) = self.client.get_connection() {
            let _: i32 = redis::Script::new(REDIS_RELEASE_LUA)
                .key(key)
                .invoke(&mut conn)
                .unwrap_or(0);
        }
    }

    fn in_use(&self, key: &str) -> u32 {
        let Ok(mut conn) = self.client.get_connection() else {
            return 0;
        };
        let v: Option<u32> = redis::cmd("GET").arg(key).query(&mut conn).unwrap_or(None);
        v.unwrap_or(0)
    }

    fn try_acquire_chunk(&self) -> Option<ChunkPermit> {
        if !self.try_acquire_key(REDIS_CHUNK_IN_USE_KEY, self.chunk_total) {
            return None;
        }
        Some(ChunkPermit {
            _inner: PermitInner::RedisChunk {
                release: RedisRelease {
                    client: self.client.clone(),
                    key: REDIS_CHUNK_IN_USE_KEY.to_string(),
                },
            },
        })
    }

    fn try_acquire_file(&self) -> Option<FilePermit> {
        if !self.try_acquire_key(REDIS_FILE_IN_USE_KEY, self.file_total) {
            return None;
        }
        Some(FilePermit {
            _inner: PermitInner::RedisFile {
                release: RedisRelease {
                    client: self.client.clone(),
                    key: REDIS_FILE_IN_USE_KEY.to_string(),
                },
            },
        })
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
}

impl RedisRelease {
    fn release_key(&self) {
        if let Ok(mut conn) = self.client.get_connection() {
            let _: i32 = redis::Script::new(REDIS_RELEASE_LUA)
                .key(&self.key)
                .invoke(&mut conn)
                .unwrap_or(0);
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

pub struct UploadGate {
    inner: GateBackend,
}

pub fn build_upload_gate(config: &crate::server_config::ServerConfig) -> UploadGate {
    if config.upload_queue_backend == "redis" {
        if let Some(url) = config.redis_url.as_deref() {
            match RedisGate::new(url, config.chunk_concurrent, config.files_concurrent) {
                Ok(gate) => {
                    return UploadGate {
                        inner: GateBackend::Redis(Arc::new(gate)),
                    };
                }
                Err(e) => {
                    log::error!("Redis UploadGate init failed ({e}); falling back to memory gate");
                }
            }
        } else {
            log::warn!("UPLOAD_QUEUE_BACKEND=redis but REDIS_URL unset — falling back to memory gate");
        }
    }
    UploadGate::new_memory(config.chunk_concurrent, config.files_concurrent)
}

impl UploadGate {
    pub fn new(chunk_concurrent: u32, files_concurrent: u32) -> Self {
        Self::new_memory(chunk_concurrent, files_concurrent)
    }

    pub fn new_memory(chunk_concurrent: u32, files_concurrent: u32) -> Self {
        let chunk_total = chunk_concurrent.max(1);
        let file_total = files_concurrent.max(1);
        Self {
            inner: GateBackend::Memory {
                chunk: Arc::new(Semaphore::new(chunk_total as usize)),
                file: Arc::new(Semaphore::new(file_total as usize)),
                chunk_total,
                file_total,
            },
        }
    }

    pub fn status(&self) -> UploadQueueStatus {
        match &self.inner {
            GateBackend::Memory {
                chunk,
                file,
                chunk_total,
                file_total,
            } => UploadQueueStatus {
                chunk_slots_total: *chunk_total,
                chunk_slots_available: chunk.available_permits() as u32,
                file_slots_total: *file_total,
                file_slots_available: file.available_permits() as u32,
            },
            GateBackend::Redis(g) => g.status(),
        }
    }

    /// Fast reject when saturated (load balancer / many clients).
    pub fn try_acquire_chunk(&self) -> Option<ChunkPermit> {
        match &self.inner {
            GateBackend::Memory { chunk, .. } => chunk
                .clone()
                .try_acquire_owned()
                .ok()
                .map(|p| ChunkPermit {
                    _inner: PermitInner::MemoryChunk(p),
                }),
            GateBackend::Redis(g) => g.try_acquire_chunk(),
        }
    }

    pub fn try_acquire_file(&self) -> Option<FilePermit> {
        match &self.inner {
            GateBackend::Memory { file, .. } => file
                .clone()
                .try_acquire_owned()
                .ok()
                .map(|p| FilePermit {
                    _inner: PermitInner::MemoryFile(p),
                }),
            GateBackend::Redis(g) => g.try_acquire_file(),
        }
    }

    /// Wait up to 5 minutes for a chunk slot (same file may retry many chunks).
    pub async fn acquire_chunk(&self) -> Option<ChunkPermit> {
        match &self.inner {
            GateBackend::Memory { chunk, .. } => {
                match tokio::time::timeout(ACQUIRE_TIMEOUT, chunk.clone().acquire_owned()).await {
                    Ok(Ok(p)) => Some(ChunkPermit {
                        _inner: PermitInner::MemoryChunk(p),
                    }),
                    _ => None,
                }
            }
            GateBackend::Redis(g) => {
                let deadline = tokio::time::Instant::now() + ACQUIRE_TIMEOUT;
                loop {
                    if let Some(p) = g.try_acquire_chunk() {
                        return Some(p);
                    }
                    if tokio::time::Instant::now() >= deadline {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    }

    pub async fn acquire_file(&self) -> Option<FilePermit> {
        match &self.inner {
            GateBackend::Memory { file, .. } => {
                match tokio::time::timeout(ACQUIRE_TIMEOUT, file.clone().acquire_owned()).await {
                    Ok(Ok(p)) => Some(FilePermit {
                        _inner: PermitInner::MemoryFile(p),
                    }),
                    _ => None,
                }
            }
            GateBackend::Redis(g) => {
                let deadline = tokio::time::Instant::now() + ACQUIRE_TIMEOUT;
                loop {
                    if let Some(p) = g.try_acquire_file() {
                        return Some(p);
                    }
                    if tokio::time::Instant::now() >= deadline {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
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
    }

    #[test]
    fn build_upload_gate_uses_config_limits() {
        let cfg = crate::server_config::test_config();
        let gate = build_upload_gate(&cfg);
        let s = gate.status();
        assert_eq!(s.chunk_slots_total, cfg.chunk_concurrent);
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
        let _: () = redis::cmd("DEL").arg(&test_key).query(&mut conn).ok().unwrap_or(());

        let ok: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(&test_key)
            .arg(2)
            .invoke(&mut conn)
            .unwrap();
        assert_eq!(ok, 1);

        let ok2: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(&test_key)
            .arg(2)
            .invoke(&mut conn)
            .unwrap();
        assert_eq!(ok2, 1);

        let fail: i32 = redis::Script::new(REDIS_ACQUIRE_LUA)
            .key(&test_key)
            .arg(2)
            .invoke(&mut conn)
            .unwrap();
        assert_eq!(fail, 0);

        let _: i32 = redis::Script::new(REDIS_RELEASE_LUA)
            .key(&test_key)
            .invoke(&mut conn)
            .unwrap();

        let _: () = redis::cmd("DEL").arg(&test_key).query(&mut conn).ok().unwrap_or(());
    }
}
