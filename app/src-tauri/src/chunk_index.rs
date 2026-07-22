//! Chunk index backend abstraction — supports SQLite (default) and Redis (multi-replica).
//!
//! ## Configuration
//!
//! ```env
//! CHUNK_INDEX_BACKEND=redis  # or sqlite (default)
//! CHUNK_INDEX_REDIS_URL=redis://redis:6379/0
//! CHUNK_INDEX_REDIS_PREFIX=td:chunk:
//! ```

use std::sync::Arc;

use crate::db::{DbConnection, UploadChunkRecord};
use crate::server_config::ServerConfig;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkIndexBackendType {
    Sqlite,
    Redis,
}

impl ChunkIndexBackendType {
    pub fn from_env() -> Self {
        let backend = std::env::var("CHUNK_INDEX_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        if backend == "redis" {
            Self::Redis
        } else {
            Self::Sqlite
        }
    }
}

// ---------------------------------------------------------------------------
// SQLite backend (existing DB) - wrapper only
// ---------------------------------------------------------------------------

/// SQLite chunk index - thin wrapper around existing db functions
pub struct SqliteChunkIndex {
    db: DbConnection,
}

impl SqliteChunkIndex {
    pub fn new(db: DbConnection) -> Arc<Self> {
        Arc::new(Self { db })
    }

    pub fn db(&self) -> &DbConnection {
        &self.db
    }
}

// ---------------------------------------------------------------------------
// Redis backend
// ---------------------------------------------------------------------------

pub struct RedisChunkIndex {
    client: redis::Client,
    prefix: String,
}

impl RedisChunkIndex {
    pub fn new(redis_url: &str, prefix: &str) -> Result<Arc<Self>, String> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| format!("Redis connect: {e}"))?;

        let mut conn = client.get_connection().map_err(|e| format!("Redis ping: {e}"))?;
        let _: String = redis::cmd("PING").query(&mut conn).map_err(|e| format!("Redis ping failed: {e}"))?;

        log::info!("RedisChunkIndex initialized: prefix={}", prefix);

        Ok(Arc::new(Self {
            client,
            prefix: prefix.to_string(),
        }))
    }

    fn key(&self, session_id: &str) -> String {
        format!("{}{}", self.prefix, session_id)
    }

    fn chunks_key(&self, session_id: &str) -> String {
        format!("{}{}:chunks", self.prefix, session_id)
    }

    pub async fn create_session(&self, session_id: &str, file_name: &str, total_chunks: i32) -> Result<(), String> {
        let key = self.key(session_id);
        let chunks_key = self.chunks_key(session_id);
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;

        redis::cmd("HMSET")
            .arg(&key)
            .arg("session_id").arg(session_id)
            .arg("filename").arg(file_name)
            .arg("total_chunks").arg(total_chunks)
            .arg("status").arg("active")
            .arg("created_at").arg(now)
            .query::<String>(&mut conn)
            .map_err(|e| format!("Redis HMSET: {e}"))?;

        redis::cmd("DEL").arg(&chunks_key).query::<String>(&mut conn).ok();
        redis::cmd("EXPIRE").arg(&key).arg(86400 * 7).query::<String>(&mut conn).ok();
        redis::cmd("EXPIRE").arg(&chunks_key).arg(86400 * 7).query::<String>(&mut conn).ok();

        Ok(())
    }

    pub async fn record_chunk(&self, session_id: &str, chunk_index: i32, file_id: &str, sha256: &str) -> Result<(), String> {
        let chunks_key = self.chunks_key(session_id);
        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;

        let chunk_data = format!("{}:{}:{}", chunk_index, file_id, sha256);
        redis::cmd("RPUSH").arg(&chunks_key).arg(&chunk_data).query::<i64>(&mut conn).map_err(|e| format!("Redis RPUSH: {e}"))?;

        Ok(())
    }

    pub async fn get_chunks(&self, session_id: &str) -> Result<Vec<UploadChunkRecord>, String> {
        let chunks_key = self.chunks_key(session_id);
        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;

        let raw_chunks: Vec<String> = redis::cmd("LRANGE")
            .arg(&chunks_key).arg(0).arg(-1)
            .query(&mut conn)
            .map_err(|e| format!("Redis LRANGE: {e}"))?;

        let chunks: Vec<UploadChunkRecord> = raw_chunks.iter().filter_map(|s| {
            let parts: Vec<&str> = s.splitn(3, ':').collect();
            if parts.len() == 3 {
                Some(UploadChunkRecord {
                    chunk_index: parts[0].parse().ok()?,
                    file_id: Some(parts[1].to_string()),
                    sha256: Some(parts[2].to_string()),
                    status: "uploaded".to_string(),
                })
            } else {
                None
            }
        }).collect();

        Ok(chunks)
    }

    pub async fn complete_session(&self, session_id: &str) -> Result<(), String> {
        let key = self.key(session_id);
        let mut conn = self.client.get_connection().map_err(|e| e.to_string())?;

        redis::cmd("HSET").arg(&key).arg("status").arg("complete").query::<i64>(&mut conn).map_err(|e| format!("Redis HSET: {e}"))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Chunk index backend enum
pub enum ChunkIndexBackendEnum {
    Sqlite(Arc<SqliteChunkIndex>),
    Redis(Arc<RedisChunkIndex>),
}

/// Build chunk index backend based on config
pub fn build_chunk_index_backend(config: &ServerConfig, db: &DbConnection) -> ChunkIndexBackendEnum {
    let backend_type = ChunkIndexBackendType::from_env();

    match backend_type {
        ChunkIndexBackendType::Redis => {
            let redis_url = std::env::var("CHUNK_INDEX_REDIS_URL")
                .ok()
                .or_else(|| config.redis_url.clone())
                .unwrap_or_else(|| "redis://127.0.0.1:6379/0".to_string());

            let prefix = std::env::var("CHUNK_INDEX_REDIS_PREFIX").unwrap_or_else(|_| "td:chunk:".to_string());

            match RedisChunkIndex::new(&redis_url, &prefix) {
                Ok(backend) => ChunkIndexBackendEnum::Redis(backend),
                Err(e) => {
                    log::warn!("Failed to init Redis chunk index, falling back to SQLite: {e}");
                    ChunkIndexBackendEnum::Sqlite(SqliteChunkIndex::new(db.clone()))
                }
            }
        }
        ChunkIndexBackendType::Sqlite => ChunkIndexBackendEnum::Sqlite(SqliteChunkIndex::new(db.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_type_from_env_defaults_to_sqlite() {
        assert_eq!(ChunkIndexBackendType::from_env(), ChunkIndexBackendType::Sqlite);
    }
}
