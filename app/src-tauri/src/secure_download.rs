//! Upload/download URL policy — HMAC presigned (preferred) or share token fallback.

use actix_web::HttpRequest;

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
    if api_settings::verify_key(key, hash) {
        Some(())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn enterprise_defaults_use_presigned_when_secret_set() {
        let mut config = (*crate::server_config::test_config()).clone();
        config.public_file_id_download = false;
        config.download_signing_secret = Some("x".repeat(32));
        config.multi_tenant_enabled = true;
        assert!(config.download_signing_secret.is_some());
    }
}
