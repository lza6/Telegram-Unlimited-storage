//! Upload/download URL policy — HMAC presigned (preferred) or share token fallback.

use actix_web::HttpRequest;

use crate::asset_locator::{AssetLocatorRecord, LocatorResolution};
use crate::postgres_control_plane::PostgresControlPlane;
use crate::postgres_download_accounting::DownloadAccountingContext;

use crate::admin_routes::{check_access_pwd, legacy_download_url};
use crate::commands::api_settings;
use crate::db::DbConnection;
use crate::file_access;
use crate::presigned_url::{self, PresignedParams};
use crate::server_config::ServerConfig;
use crate::sharing_core;

#[derive(Debug, Clone)]
pub struct SecureDownloadLink {
    pub download_url: String,
    pub share_id: String,
    pub expires_at: Option<i64>,
    /// Telegram message id (catalog only — not authorization).
    pub file_id: String,
    pub owner_id: String,
    pub link_kind: &'static str,
}

/// Register ownership and issue download URL for uploader.
pub fn issue_upload_download_link(
    db: &DbConnection,
    config: &ServerConfig,
    base_url: &str,
    folder_id: Option<i64>,
    message_id: i32,
    file_name: String,
    file_size: i64,
    owner_id: &str,
    merged_manifest: bool,
) -> Result<SecureDownloadLink, String> {
    let owner = file_access::default_owner_if_missing(owner_id);
    let file_id = message_id.to_string();
    file_access::record_uploaded_file(db, message_id, folder_id, owner, &file_name, file_size)?;

    let now = chrono::Utc::now().timestamp();
    let ttl_secs = config.upload_link_ttl_secs;
    // 0 = permanent presigned URL (exp=0, signature still required; rotate secret to revoke)
    let expires_at = if ttl_secs == 0 {
        0
    } else {
        now + ttl_secs.max(60) as i64
    };

    if let Some(secret) = config.download_signing_secret.as_deref() {
        let params = PresignedParams {
            message_id,
            folder_id,
            expires_at,
            owner_id: owner.to_string(),
            max_downloads: config.presigned_max_downloads,
        };
        let url = presigned_url::build_presigned_url(base_url, &params, secret)?;
        return Ok(SecureDownloadLink {
            download_url: url,
            share_id: String::new(),
            expires_at: Some(expires_at).filter(|&e| e > 0),
            file_id,
            owner_id: owner.to_string(),
            link_kind: "presigned",
        });
    }

    if config.upload_share_ttl_hours > 0 {
        let hours = config.upload_share_ttl_hours;
        let share = sharing_core::create_share(
            db,
            base_url,
            folder_id,
            message_id,
            file_name,
            file_size,
            None,
            Some(hours),
            Some(owner),
        )?;
        return Ok(SecureDownloadLink {
            download_url: share.link,
            share_id: share.id,
            expires_at: share.expires_at,
            file_id,
            owner_id: owner.to_string(),
            link_kind: "share",
        });
    }

    if config.public_file_id_download {
        let name_for_url = if merged_manifest {
            "fileAll.txt".to_string()
        } else {
            file_name
        };
        return Ok(SecureDownloadLink {
            download_url: legacy_download_url(base_url, message_id, &name_for_url, merged_manifest),
            share_id: String::new(),
            expires_at: None,
            file_id,
            owner_id: owner.to_string(),
            link_kind: "legacy",
        });
    }

    Err("No download link mode: set DOWNLOAD_SIGNING_SECRET or UPLOAD_SHARE_TTL_HOURS".to_string())
}

/// Whether `GET /d?file_id=` is allowed for this request.
pub fn raw_file_id_download_allowed(req: &HttpRequest, config: &ServerConfig) -> bool {
    if config.public_file_id_download {
        return true;
    }
    if check_access_pwd(req, config) {
        return true;
    }
    api_key_from_request(req, config).is_some()
}

fn api_key_from_request(req: &HttpRequest, config: &ServerConfig) -> Option<()> {
    let hash = config.api_key_hash.as_ref()?;
    let key = req.headers().get("X-API-Key")?.to_str().ok()?;
    if api_settings::verify_and_upgrade_key(key, hash, &config.data_dir) {
        Some(())
    } else {
        None
    }
}

#[derive(Clone)]
pub struct PreparedCanonicalDownload {
    pub locator: AssetLocatorRecord,
    pub accounting: Option<(PostgresControlPlane, DownloadAccountingContext)>,
    pub scheduler: Option<crate::http_download::SchedulerStreamContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadPreflightError {
    Forbidden(String),
    Ambiguous,
    Locator(String),
    ControlPlane(String),
}

fn request_id_for_download(req: &HttpRequest, namespace: &str) -> String {
    let request_id = req
        .headers()
        .get("X-Request-ID")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty() && value.len() <= 200)
        .unwrap_or("");
    if request_id.is_empty() {
        format!("{namespace}:{}", uuid::Uuid::new_v4())
    } else {
        format!("{namespace}:{request_id}")
    }
}

/// Resolve the canonical locator and, in PostgreSQL mode, authorize the exact
/// tenant/peer asset while creating a fenced download-accounting job.
/// `Ok(None)` is only a legacy SQLite-mode fallback. PostgreSQL mode fails
/// closed when the canonical locator or control-plane asset is unavailable.
pub async fn prepare_canonical_download(
    req: &HttpRequest,
    db: &DbConnection,
    message_id: i32,
    legacy_folder_id: Option<i64>,
    expected_owner: Option<&str>,
    request_namespace: &str,
) -> Result<Option<PreparedCanonicalDownload>, DownloadPreflightError> {
    let control_plane =
        PostgresControlPlane::from_env().map_err(DownloadPreflightError::ControlPlane)?;
    let locator =
        match crate::asset_locator::resolve(db, message_id, legacy_folder_id, expected_owner) {
            Ok(LocatorResolution::Found(locator)) => locator,
            Ok(LocatorResolution::Ambiguous { .. }) => {
                return Err(DownloadPreflightError::Ambiguous)
            }
            Ok(LocatorResolution::NotFound) if control_plane.enabled() => {
                return Err(DownloadPreflightError::ControlPlane(
                    "DOWNLOAD_ASSET_LOCATOR_REQUIRED".to_string(),
                ))
            }
            Ok(LocatorResolution::NotFound) => return Ok(None),
            Err(error) => return Err(DownloadPreflightError::Locator(error)),
        };
    if let Some(owner) = expected_owner {
        if locator.owner_id != owner {
            return Err(DownloadPreflightError::Forbidden(
                "DOWNLOAD_ASSET_OWNER_MISMATCH".to_string(),
            ));
        }
    }
    let request_id = request_id_for_download(req, request_namespace);
    let accounting = control_plane
        .begin_download(
            &locator.owner_id,
            &locator,
            &request_id,
            locator.file_size.max(0) as u64,
        )
        .await
        .map_err(DownloadPreflightError::ControlPlane)?
        .map(|context| (control_plane.clone(), context));
    let scheduler = if let Some((scheduler_cp, context)) = accounting.as_ref() {
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let credentials = crate::postgres_upload_saga::SagaNodeCredentials::from_env(
            std::path::Path::new(&data_dir),
        )
        .map_err(DownloadPreflightError::ControlPlane)?;
        let bot = if locator.transport_mode == "bot" {
            Some(locator.uploader_bot_id.as_deref().ok_or_else(|| {
                DownloadPreflightError::ControlPlane(
                    "DOWNLOAD_SCHEDULER_BOT_ID_REQUIRED".to_string(),
                )
            })?)
        } else {
            None
        };
        let resources = crate::durable_scheduler::SchedulerResourceSet::transfer(
            &locator.transport_mode,
            "download",
            &context.tenant_id,
            "download",
            bot,
            Some((
                &locator.transport_mode,
                &locator.storage_peer_kind,
                locator.storage_peer_id,
            )),
        )
        .map_err(DownloadPreflightError::ControlPlane)?;
        let lease = scheduler_cp
            .acquire_scheduler_lease(&credentials, &context.job_id, &resources, 300)
            .await
            .map_err(DownloadPreflightError::ControlPlane)?
            .ok_or_else(|| {
                DownloadPreflightError::ControlPlane("DOWNLOAD_SCHEDULER_REQUIRED".to_string())
            })?;
        Some(crate::http_download::SchedulerStreamContext {
            guard: crate::durable_scheduler::SchedulerLeaseGuard::start_download(
                scheduler_cp.clone(),
                credentials,
                lease,
            ),
        })
    } else {
        None
    };
    Ok(Some(PreparedCanonicalDownload {
        locator,
        accounting,
        scheduler,
    }))
}

#[cfg(test)]
mod tests {
    use super::request_id_for_download;

    #[test]
    fn enterprise_defaults_use_presigned_when_secret_set() {
        let mut config = (*crate::server_config::test_config()).clone();
        config.public_file_id_download = false;
        config.download_signing_secret = Some("x".repeat(32));
        config.multi_tenant_enabled = true;
        assert!(config.download_signing_secret.is_some());
    }

    #[test]
    fn download_request_id_is_namespaced_and_replay_stable() {
        let req = actix_web::test::TestRequest::default()
            .insert_header(("X-Request-ID", "retry-42"))
            .to_http_request();
        assert_eq!(
            request_id_for_download(&req, "share:abc"),
            "share:abc:retry-42"
        );
    }

    #[test]
    fn invalid_download_request_id_is_not_reused() {
        let req = actix_web::test::TestRequest::default()
            .insert_header(("X-Request-ID", "x".repeat(201)))
            .to_http_request();
        let id = request_id_for_download(&req, "webdav:def");
        assert!(id.starts_with("webdav:def:"));
        assert!(!id.ends_with(&"x".repeat(201)));
    }
}
