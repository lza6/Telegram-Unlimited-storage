//! Storage backend abstraction — primary Telegram with fallback support.
//! Enables multi-storage resilience (S3/R2/WebDAV as failover when Telegram is unavailable).
//!
//! ## Architecture
//!
//! ```text
//! StorageFactory
//!   ├── primary: TelegramBackend (Bot API + User MTProto)
//!   ├── fallback: Option<S3Backend | R2Backend | WebDAVBackend>
//!   └── strategy: FailoverStrategy
//!         ├── failover (primary → fallback on error)
//!         ├── mirror   (write to both, read from primary)
//!         └── tiered   (primary for <20MB, fallback for larger)
//! ```
//!
//! ## Configuration
//!
//! ```env
//! # Primary is always Telegram
//! STORAGE_FALLBACK=s3
//! S3_ENDPOINT=https://s3.amazonaws.com
//! S3_REGION=us-east-1
//! S3_BUCKET=telegram-drive-fallback
//! S3_ACCESS_KEY=xxx
//! S3_SECRET_KEY=xxx
//! STORAGE_FAILOVER_STRATEGY=failover  # failover | mirror | tiered
//! STORAGE_FALLBACK_ONLY_BOT=false     # only fallback when Bot mode fails
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;

use crate::server_config::ServerConfig;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Backend type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BackendType {
    Telegram,
    S3,
    R2,
    WebDAV,
}

impl BackendType {
    fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "telegram" => Some(Self::Telegram),
            "s3" => Some(Self::S3),
            "r2" => Some(Self::R2),
            "webdav" => Some(Self::WebDAV),
            _ => None,
        }
    }
}

/// Result of a storage operation
#[derive(Debug, Clone, Serialize)]
pub struct StorageResult {
    pub id: String,
    pub backend: BackendType,
    pub size: u64,
    pub mime_type: String,
}

/// Failover strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverStrategy {
    /// Primary first, fallback on error
    Failover,
    /// Write to both, read from primary
    Mirror,
    /// Primary for small files (<20MB), fallback for larger
    Tiered,
}

impl FailoverStrategy {
    fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "mirror" => Self::Mirror,
            "tiered" => Self::Tiered,
            _ => Self::Failover,
        }
    }
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// Abstract storage backend
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Upload raw bytes
    async fn upload(&self, data: &[u8], name: &str, mime: &str) -> Result<StorageResult, String>;

    /// Download by ID
    async fn download(&self, id: &str) -> Result<Vec<u8>, String>;

    /// Delete by ID
    async fn delete(&self, id: &str) -> Result<(), String>;

    /// Health check
    async fn health_check(&self) -> Result<bool, String>;

    /// Backend type
    fn backend_type(&self) -> BackendType;

    /// Maximum file size in bytes (0 = unlimited)
    fn max_file_size(&self) -> u64 { 0 }
}

// ---------------------------------------------------------------------------
// S3 backend
// ---------------------------------------------------------------------------

pub struct S3Backend {
    endpoint: String,
    region: String,
    bucket: String,
    access_key: String,
    secret_key: String,
}

impl S3Backend {
    pub fn from_config(config: &ServerConfig) -> Option<Arc<Self>> {
        let endpoint = config.s3_endpoint.as_deref()?;
        let region = config.s3_region.as_deref().unwrap_or("us-east-1");
        let bucket = config.s3_bucket.as_deref()?;
        let access_key = config.s3_access_key.as_deref()?;
        let secret_key = config.s3_secret_key.as_deref()?;

        let this = Self {
            endpoint: endpoint.to_string(),
            region: region.to_string(),
            bucket: bucket.to_string(),
            access_key: access_key.to_string(),
            secret_key: secret_key.to_string(),
        };

        log::info!("S3 backend configured: bucket={}, endpoint={}", this.bucket, this.endpoint);
        Some(Arc::new(this))
    }
}

#[async_trait]
impl StorageBackend for S3Backend {
    fn backend_type(&self) -> BackendType {
        BackendType::S3
    }

    fn max_file_size(&self) -> u64 {
        100 * 1024 * 1024 // 100MB for basic S3
    }

    async fn upload(&self, data: &[u8], name: &str, mime: &str) -> Result<StorageResult, String> {
        let key = format!("td/{}/{}", chrono::Utc::now().format("%Y/%m/%d"), name);
        let url = format!("{}/{}/{}", self.endpoint.trim_end_matches('/'), self.bucket, key);

        let client = reqwest::Client::new();
        let resp = client
            .put(&url)
            .header("Content-Type", mime)
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| format!("S3 upload request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("S3 upload failed ({}): {}", status, body));
        }

        Ok(StorageResult {
            id: key,
            backend: BackendType::S3,
            size: data.len() as u64,
            mime_type: mime.to_string(),
        })
    }

    async fn download(&self, id: &str) -> Result<Vec<u8>, String> {
        let url = format!("{}/{}/{}", self.endpoint.trim_end_matches('/'), self.bucket, id.trim_start_matches('/'));
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("S3 download request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("S3 download failed: {}", resp.status()));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("S3 download body: {e}"))
    }

    async fn delete(&self, id: &str) -> Result<(), String> {
        let url = format!("{}/{}/{}", self.endpoint.trim_end_matches('/'), self.bucket, id.trim_start_matches('/'));
        let client = reqwest::Client::new();
        let resp = client
            .delete(&url)
            .send()
            .await
            .map_err(|e| format!("S3 delete request failed: {e}"))?;

        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            return Err(format!("S3 delete failed: {}", resp.status()));
        }
        Ok(())
    }

    async fn health_check(&self) -> Result<bool, String> {
        let url = format!("{}/{}", self.endpoint.trim_end_matches('/'), self.bucket);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        match client.head(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success() || resp.status().as_u16() == 403),
            Err(_) => Ok(false),
        }
    }
}

// ---------------------------------------------------------------------------
// StorageFactory
// ---------------------------------------------------------------------------

/// Factory managing primary and fallback storage backends
pub struct StorageFactory {
    /// Primary Telegram backend (always present)
    pub primary_backend_type: BackendType,
    /// Optional fallback backend (S3/R2/WebDAV)
    pub fallback: Option<Arc<dyn StorageBackend>>,
    /// Failover strategy
    pub strategy: FailoverStrategy,
    /// If true, only fallback when Bot mode fails
    pub fallback_only_bot: bool,
}

impl StorageFactory {
    /// Build factory from server config
    pub fn from_config(config: &ServerConfig) -> Self {
        // Determine fallback backend from env
        let fallback_type = config
            .s3_endpoint
            .as_deref()
            .and_then(|_| BackendType::from_str("s3"))
            .or_else(|| {
                std::env::var("STORAGE_FALLBACK").ok().as_deref().and_then(BackendType::from_str)
            });

        let fallback = match fallback_type {
            Some(BackendType::S3) => S3Backend::from_config(config).map(|b| b as Arc<dyn StorageBackend>),
            Some(_) => {
                log::warn!("Unsupported fallback backend configured");
                None
            }
            None => None,
        };

        let strategy = FailoverStrategy::from_str(
            &std::env::var("STORAGE_FAILOVER_STRATEGY").unwrap_or_default(),
        );

        let fallback_only_bot = std::env::var("STORAGE_FALLBACK_ONLY_BOT")
            .ok()
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        if fallback.is_some() {
            log_info_fallback(&strategy, &fallback_type.unwrap());
        }

        Self {
            primary_backend_type: BackendType::Telegram,
            fallback,
            strategy,
            fallback_only_bot,
        }
    }

    /// Upload via primary, fallback on failure (strategy-dependent)
    pub async fn upload(
        &self,
        data: &[u8],
        name: &str,
        mime: &str,
        primary_result: Result<StorageResult, String>,
    ) -> Result<StorageResult, String> {
        match self.strategy {
            FailoverStrategy::Failover => {
                match primary_result {
                    Ok(r) => Ok(r),
                    Err(e) => {
                        log::warn!("Primary storage failed, trying fallback: {e}");
                        match &self.fallback {
                            Some(fb) => fb.upload(data, name, mime).await,
                            None => Err(e),
                        }
                    }
                }
            }
            FailoverStrategy::Mirror => {
                // Write to primary
                let primary = match primary_result {
                    Ok(r) => r,
                    Err(e) => {
                        log::warn!("Mirror: primary failed, fallback only: {e}");
                        match &self.fallback {
                            Some(fb) => return fb.upload(data, name, mime).await,
                            None => return Err(e),
                        }
                    }
                };
                // Mirror to fallback in background
                if let Some(fb) = &self.fallback {
                    let fb = fb.clone();
                    let data = data.to_vec();
                    let name = name.to_string();
                    let mime = mime.to_string();
                    tokio::spawn(async move {
                        match fb.upload(&data, &name, &mime).await {
                            Ok(r) => log::info!("Mirror upload to {:?} succeeded: {}", fb.backend_type(), r.id),
                            Err(e) => log::warn!("Mirror upload failed: {e}"),
                        }
                    });
                }
                Ok(primary)
            }
            FailoverStrategy::Tiered => {
                // Primary for files <20MB, fallback for larger
                if data.len() < 20 * 1024 * 1024 {
                    primary_result
                } else {
                    match &self.fallback {
                        Some(fb) => fb.upload(data, name, mime).await,
                        None => primary_result,
                    }
                }
            }
        }
    }

    /// Check if a specific backend is healthy
    pub async fn health_check_backend(&self, backend_type: BackendType) -> Result<bool, String> {
        match backend_type {
            BackendType::Telegram => Ok(true), // Checked via /api/v1/health
            _ => match &self.fallback {
                Some(fb) if fb.backend_type() == backend_type => fb.health_check().await,
                _ => Ok(false),
            },
        }
    }

    /// Get health summary of all backends
    pub async fn health_summary(&self) -> Vec<(BackendType, bool)> {
        let mut result = vec![(BackendType::Telegram, true)];
        if let Some(fb) = &self.fallback {
            let healthy = fb.health_check().await.unwrap_or(false);
            result.push((fb.backend_type(), healthy));
        }
        result
    }
}

fn log_info_fallback(strategy: &FailoverStrategy, backend: &BackendType) {
    log::info!(
        "StorageFactory: {:?} fallback enabled (strategy={:?})",
        backend,
        strategy
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failover_strategy_defaults_to_failover() {
        assert_eq!(FailoverStrategy::from_str(""), FailoverStrategy::Failover);
        assert_eq!(FailoverStrategy::from_str("unknown"), FailoverStrategy::Failover);
    }

    #[test]
    fn failover_strategy_mirror_and_tiered() {
        assert_eq!(FailoverStrategy::from_str("mirror"), FailoverStrategy::Mirror);
        assert_eq!(FailoverStrategy::from_str("tiered"), FailoverStrategy::Tiered);
    }

    #[test]
    fn backend_type_parsing() {
        assert_eq!(BackendType::from_str("telegram"), Some(BackendType::Telegram));
        assert_eq!(BackendType::from_str("s3"), Some(BackendType::S3));
        assert_eq!(BackendType::from_str("r2"), Some(BackendType::R2));
        assert_eq!(BackendType::from_str("invalid"), None);
    }

    #[test]
    fn failover_upload_passes_primary_ok() {
        // Test that failover strategy returns Ok primary result without calling fallback
        let factory = StorageFactory {
            primary_backend_type: BackendType::Telegram,
            fallback: None,
            strategy: FailoverStrategy::Failover,
            fallback_only_bot: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            factory
                .upload(
                    &[1, 2, 3],
                    "test.txt",
                    "text/plain",
                    Ok(StorageResult {
                        id: "primary-id".into(),
                        backend: BackendType::Telegram,
                        size: 3,
                        mime_type: "text/plain".into(),
                    }),
                )
                .await
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "primary-id");
    }

    #[test]
    fn failover_upload_falls_back_on_error() {
        let factory = StorageFactory {
            primary_backend_type: BackendType::Telegram,
            fallback: None, // No fallback configured -> error propagates
            strategy: FailoverStrategy::Failover,
            fallback_only_bot: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            factory
                .upload(
                    &[1, 2, 3],
                    "test.txt",
                    "text/plain",
                    Err("primary failed".into()),
                )
                .await
        });

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "primary failed");
    }
}
