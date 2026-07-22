//! Presigned download counter — enforces max download limits per signature.
//!
//! Presigned URLs are stateless (HMAC-signed), so to enforce a download limit
//! we store a counter keyed by the signature. Counters have TTL matching the
//! presigned URL expiry.
//!
//! Backends:
//! - Redis: `td:dlcount:{sig}` (preferred for multi-replica)
//! - In-memory: `HashMap<signature, (remaining, expires_at)>` with periodic cleanup

use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Counter backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadCounterBackend {
    Memory,
    Redis,
}

impl DownloadCounterBackend {
    pub fn from_env() -> Self {
        if std::env::var("REDIS_URL").is_ok() || std::env::var("PRESIGNED_REDIS_URL").is_ok() {
            Self::Redis
        } else {
            Self::Memory
        }
    }
}

// ---------------------------------------------------------------------------
// Memory backend
// ---------------------------------------------------------------------------

pub struct MemoryDownloadCounter {
    inner: parking_lot::Mutex<HashMap<String, (u32, i64)>>,
}

impl MemoryDownloadCounter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: parking_lot::Mutex::new(HashMap::new()),
        })
    }

    pub fn try_consume(
        &self,
        signature: &str,
        max_downloads: u32,
        expires_at: i64,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        if expires_at > 0 && now > expires_at {
            return Err("Presigned URL has expired".to_string());
        }

        let mut map = self.inner.lock();
        let entry = map
            .entry(signature.to_string())
            .or_insert((max_downloads, expires_at));

        // If TTL changed, update it
        entry.1 = expires_at;

        if entry.0 == 0 {
            return Err("Presigned URL download limit reached".to_string());
        }

        entry.0 -= 1;
        Ok(())
    }

    pub fn cleanup_expired(&self) {
        let now = chrono::Utc::now().timestamp();
        let mut map = self.inner.lock();
        map.retain(|_, (_, exp)| *exp == 0 || *exp >= now);
    }
}

// ---------------------------------------------------------------------------
// Redis backend
// ---------------------------------------------------------------------------

pub struct RedisDownloadCounter {
    client: redis::Client,
    prefix: String,
}

impl RedisDownloadCounter {
    pub fn new(redis_url: &str, prefix: &str) -> Result<Arc<Self>, String> {
        let client = redis::Client::open(redis_url).map_err(|e| format!("Redis connect: {e}"))?;

        // Verify connection
        let mut conn = client
            .get_connection()
            .map_err(|e| format!("Redis ping: {e}"))?;
        let _: String = redis::cmd("PING")
            .query(&mut conn)
            .map_err(|e| format!("Redis ping failed: {e}"))?;

        log::info!("RedisDownloadCounter initialized: prefix={}", prefix);

        Ok(Arc::new(Self {
            client,
            prefix: prefix.to_string(),
        }))
    }

    fn key(&self, signature: &str) -> String {
        format!("{}{}", self.prefix, signature)
    }

    pub async fn try_consume(
        &self,
        signature: &str,
        max_downloads: u32,
        expires_at: i64,
    ) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp();
        if expires_at > 0 && now > expires_at {
            return Err("Presigned URL has expired".to_string());
        }

        let key = self.key(signature);
        let client = self.client.clone();

        // Use Redis DECR on a counter initialized to max_downloads.
        // First call sets the key with TTL if not exists.
        let result = tokio::task::spawn_blocking(move || {
            let mut conn = client.get_connection().map_err(|e| e.to_string())?;

            // Lua script: initialize if not exists, decrement, return new value
            let script = redis::Script::new(
                r#"
                local key = KEYS[1]
                local max_val = tonumber(ARGV[1])
                local ttl = tonumber(ARGV[2])
                local exists = redis.call("EXISTS", key)
                if exists == 0 then
                    redis.call("SET", key, max_val, "EX", ttl)
                end
                return redis.call("DECR", key)
                "#,
            );

            let ttl = if expires_at > 0 {
                (expires_at - now).max(1) as usize
            } else {
                86400 * 7 // 7 days default for non-expiring URLs
            };

            script
                .key(&key)
                .arg(max_downloads.to_string())
                .arg(ttl.to_string())
                .invoke::<i64>(&mut conn)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| format!("Redis counter task failed: {e}"))?;

        match result {
            Ok(remaining) if remaining >= 0 => Ok(()),
            Ok(_) => Err("Presigned URL download limit reached".to_string()),
            Err(e) => Err(format!("Redis counter error: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Unified counter
// ---------------------------------------------------------------------------

pub enum DownloadCounter {
    Memory(Arc<MemoryDownloadCounter>),
    Redis(Arc<RedisDownloadCounter>),
}

impl DownloadCounter {
    pub fn from_env() -> Self {
        match DownloadCounterBackend::from_env() {
            DownloadCounterBackend::Redis => {
                let redis_url = std::env::var("PRESIGNED_REDIS_URL")
                    .or_else(|_| std::env::var("REDIS_URL"))
                    .unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_string());

                let prefix = std::env::var("PRESIGNED_REDIS_PREFIX")
                    .unwrap_or_else(|_| "td:dlcount:".to_string());

                match RedisDownloadCounter::new(&redis_url, &prefix) {
                    Ok(counter) => {
                        log::info!("Using Redis-backed presigned download counter");
                        Self::Redis(counter)
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to init Redis download counter, falling back to memory: {e}"
                        );
                        Self::Memory(MemoryDownloadCounter::new())
                    }
                }
            }
            DownloadCounterBackend::Memory => {
                log::info!("Using in-memory presigned download counter");
                Self::Memory(MemoryDownloadCounter::new())
            }
        }
    }

    pub fn memory() -> Self {
        Self::Memory(MemoryDownloadCounter::new())
    }

    pub async fn try_consume(
        &self,
        signature: &str,
        max_downloads: u32,
        expires_at: i64,
    ) -> Result<(), String> {
        match self {
            Self::Memory(c) => c.try_consume(signature, max_downloads, expires_at),
            Self::Redis(c) => c.try_consume(signature, max_downloads, expires_at).await,
        }
    }

    /// Start periodic cleanup task for in-memory backend
    pub fn spawn_cleanup_task(
        &self,
        interval_secs: u64,
        running: Arc<std::sync::atomic::AtomicBool>,
    ) {
        if let Self::Memory(c) = self {
            let counter = c.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
                    if !running.load(std::sync::atomic::Ordering::SeqCst) {
                        return;
                    }
                    counter.cleanup_expired();
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_counter_enforces_limit() {
        let counter = MemoryDownloadCounter::new();
        let sig = "sig-1";
        let exp = chrono::Utc::now().timestamp() + 3600;

        assert!(counter.try_consume(sig, 2, exp).is_ok());
        assert!(counter.try_consume(sig, 2, exp).is_ok());
        assert!(counter.try_consume(sig, 2, exp).is_err());
    }

    #[test]
    fn memory_counter_rejects_expired() {
        let counter = MemoryDownloadCounter::new();
        let sig = "sig-expired";
        let exp = chrono::Utc::now().timestamp() - 1;

        assert!(counter.try_consume(sig, 1, exp).is_err());
    }

    #[test]
    fn memory_counter_cleanup_removes_expired() {
        let counter = MemoryDownloadCounter::new();
        let now = chrono::Utc::now().timestamp();

        counter.try_consume("live", 5, now + 3600).unwrap();
        // Insert an entry that will be expired after cleanup
        counter.try_consume("dead", 5, now + 2).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(3));

        counter.cleanup_expired();

        // live entry still works
        assert!(counter.try_consume("live", 5, now + 3600).is_ok());
        // dead entry was removed and re-checking expired timestamp fails
        assert!(counter.try_consume("dead", 5, now + 2).is_err());
    }
}
