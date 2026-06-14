//! Enforce file ownership before download / list.

use crate::db::{self, DbConnection};
use crate::server_config::ServerConfig;
use crate::telegram_transport::TelegramTransportMode;
use crate::tenant_auth::{CallerIdentity, OWNER_WEB};

/// When true, list/search/get must use the asset index exclusively (Bot / multi-tenant,
/// or User mode after an explicit full rebuild via Sync).
pub fn asset_index_authoritative(
    mode: TelegramTransportMode,
    config: &ServerConfig,
    db: &DbConnection,
) -> bool {
    if config.multi_tenant_enabled || mode == TelegramTransportMode::Bot {
        return true;
    }
    db::is_file_index_complete(db).unwrap_or(false)
}

pub fn record_uploaded_file(
    db: &DbConnection,
    message_id: i32,
    folder_id: Option<i64>,
    owner_id: &str,
    file_name: &str,
    file_size: i64,
) -> Result<(), String> {
    db::upsert_file_asset(db, message_id, folder_id, owner_id, file_name, file_size)
}

/// Best-effort batch index from desktop file list (lazy sync for User mode search).
pub fn index_file_metadata_list(
    db: &DbConnection,
    files: &[crate::models::FileMetadata],
    owner_id: &str,
) {
    for f in files {
        if f.icon_type == "folder" {
            continue;
        }
        if let Err(e) = record_uploaded_file(
            db,
            f.id as i32,
            f.folder_id,
            owner_id,
            &f.name,
            f.size as i64,
        ) {
            log::debug!("file_assets index skip {}: {e}", f.id);
        }
    }
}

/// After forward-move, Telegram assigns new message IDs — remap index rows.
pub fn remap_file_assets_after_move(
    db: &DbConnection,
    old_ids: &[i32],
    new_ids: &[i32],
    target_folder_id: Option<i64>,
) -> Result<(), String> {
    if old_ids.is_empty() {
        return Ok(());
    }
    if new_ids.len() != old_ids.len() {
        for old_id in old_ids {
            let _ = crate::db::delete_file_asset(db, *old_id, None);
        }
        return Err(format!(
            "forwarded message count {} != requested {}",
            new_ids.len(),
            old_ids.len()
        ));
    }
    for (old_id, new_id) in old_ids.iter().zip(new_ids.iter()) {
        let asset = crate::db::get_file_asset(db, *old_id)?;
        let _ = crate::db::delete_file_asset(db, *old_id, None);
        if let Some(a) = asset {
            record_uploaded_file(
                db,
                *new_id,
                target_folder_id,
                &a.owner_id,
                &a.file_name,
                a.file_size,
            )?;
        }
    }
    Ok(())
}

/// Returns Ok(()) if download is allowed; Err message for HTTP 403.
pub fn assert_download_allowed(
    db: &DbConnection,
    message_id: i32,
    caller: &CallerIdentity,
    multi_tenant: bool,
) -> Result<(), String> {
    if !multi_tenant {
        return Ok(());
    }
    if caller == &CallerIdentity::Admin {
        return Ok(());
    }
    let Some(asset) = db::get_file_asset(db, message_id)? else {
        return Err("File is not registered in the asset index".to_string());
    };
    if caller.can_access_owner(&asset.owner_id) {
        return Ok(());
    }
    Err("Access denied: file belongs to another tenant".to_string())
}

/// Presigned URL must match stored asset owner and folder.
pub fn assert_presigned_asset(
    db: &DbConnection,
    message_id: i32,
    folder_id: Option<i64>,
    owner_id: &str,
) -> Result<(), String> {
    let Some(asset) = db::get_file_asset(db, message_id)? else {
        return Err("Unknown file".to_string());
    };
    if asset.owner_id != owner_id {
        return Err("Owner mismatch".to_string());
    }
    if asset.folder_id != folder_id {
        return Err("Folder mismatch".to_string());
    }
    Ok(())
}

pub fn default_owner_if_missing(owner_id: &str) -> &str {
    if owner_id.is_empty() {
        OWNER_WEB
    } else {
        owner_id
    }
}

/// Share token download: when multi-tenant is on, file must still exist in the asset index.
pub fn assert_share_download_allowed(
    db: &DbConnection,
    message_id: i32,
    folder_id: Option<i64>,
    share_owner_id: Option<&str>,
    multi_tenant: bool,
) -> Result<(), String> {
    if !multi_tenant {
        return Ok(());
    }
    let Some(asset) = db::get_file_asset(db, message_id)? else {
        return Err("File is not registered in the asset index".to_string());
    };
    if let Some(so) = share_owner_id {
        if !so.is_empty() && asset.owner_id != so {
            return Err("Share no longer valid for this file".to_string());
        }
    }
    if asset.folder_id != folder_id {
        return Err("Folder mismatch".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn temp_db() -> db::DbConnection {
        let dir = std::env::temp_dir().join(format!("td-fa-test-{}", uuid::Uuid::new_v4()));
        db::init_db_at(&dir).expect("db")
    }

    #[test]
    fn share_download_requires_asset_when_multi_tenant() {
        let db = temp_db();
        db::upsert_file_asset(&db, 42, Some(1), "tenant:a", "a.txt", 10).expect("asset");
        assert!(assert_share_download_allowed(&db, 42, Some(1), Some("tenant:a"), true).is_ok());
        assert!(assert_share_download_allowed(&db, 99, Some(1), None, true).is_err());
    }

    #[test]
    fn asset_index_authoritative_user_requires_complete_flag() {
        use crate::telegram_transport::TelegramTransportMode;
        let db = temp_db();
        db::upsert_file_asset(&db, 1, None, "admin", "a.bin", 1).expect("asset");
        let mut cfg = (*crate::server_config::test_config()).clone();
        cfg.multi_tenant_enabled = false;
        assert!(!asset_index_authoritative(
            TelegramTransportMode::User,
            &cfg,
            &db
        ));
        db::set_file_index_complete(&db, true).expect("complete");
        assert!(asset_index_authoritative(
            TelegramTransportMode::User,
            &cfg,
            &db
        ));
        cfg.multi_tenant_enabled = true;
        assert!(asset_index_authoritative(
            TelegramTransportMode::User,
            &cfg,
            &db
        ));
    }

    #[test]
    fn remap_file_assets_after_move_updates_ids_and_folder() {
        let db = temp_db();
        db::upsert_file_asset(&db, 10, Some(100), "admin", "doc.pdf", 50).expect("upsert");
        remap_file_assets_after_move(&db, &[10], &[99], Some(200)).expect("remap");
        assert!(db::get_file_asset(&db, 10).expect("get").is_none());
        let moved = db::get_file_asset(&db, 99).expect("get").expect("row");
        assert_eq!(moved.folder_id, Some(200));
        assert_eq!(moved.file_name, "doc.pdf");
    }
}
