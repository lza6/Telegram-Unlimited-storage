use rand::Rng;
use serde::Serialize;

use crate::db::DbConnection;

#[derive(Debug, Serialize, Clone)]
pub struct ShareInfo {
    pub id: String,
    pub file_name: String,
    pub file_size: i64,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub has_password: bool,
    pub link: String,
}

fn generate_share_token() -> String {
    let mut rng = rand::thread_rng();
    // 32 bytes → 64 hex chars (~256-bit), comparable to OSS presigned path entropy.
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Build a share link from base URL and token.
/// Falls back to localhost if base_url is empty.
fn build_share_link(base_url: &str, token: &str) -> String {
    let base = if base_url.is_empty() {
        format!("http://127.0.0.1:{}", crate::STREAM_PORT)
    } else {
        base_url.trim_end_matches('/').to_string()
    };
    format!("{base}/d/{token}")
}

pub fn create_share(
    db_pool: &DbConnection,
    base_url: &str,
    folder_id: Option<i64>,
    message_id: i32,
    file_name: String,
    file_size: i64,
    password: Option<String>,
    expiry_hours: Option<i64>,
    owner_id: Option<&str>,
) -> Result<ShareInfo, String> {
    let token = generate_share_token();
    let created_at = chrono::Utc::now().timestamp();
    let expires_at = expiry_hours.map(|hours| created_at + hours * 3600);

    let (password_hash, password_salt) = if let Some(ref pwd) = password {
        if pwd.is_empty() {
            (None, None)
        } else {
            let (hash, salt) = crate::password_kdf::hash_share_password(pwd);
            (Some(hash), salt)
        }
    } else {
        (None, None)
    };

    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "INSERT INTO shared_links (id, folder_id, message_id, file_name, file_size, password_hash, password_salt, expires_at, revoked, created_at, owner_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
        )
        .map_err(|e| e.to_string())?;

    stmt.bind((1, token.as_str())).map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.bind((3, message_id as i64))
        .map_err(|e| e.to_string())?;
    stmt.bind((4, file_name.as_str()))
        .map_err(|e| e.to_string())?;
    stmt.bind((5, file_size)).map_err(|e| e.to_string())?;
    stmt.bind((6, password_hash.as_deref()))
        .map_err(|e| e.to_string())?;
    stmt.bind((7, password_salt.as_deref()))
        .map_err(|e| e.to_string())?;
    stmt.bind((8, expires_at)).map_err(|e| e.to_string())?;
    stmt.bind((9, created_at)).map_err(|e| e.to_string())?;
    stmt.bind((10, owner_id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;

    let link = build_share_link(base_url, &token);

    Ok(ShareInfo {
        id: token,
        file_name,
        file_size,
        created_at,
        expires_at,
        has_password: password_hash.is_some(),
        link,
    })
}

pub fn list_shares(
    db_pool: &DbConnection,
    base_url: &str,
    owner_id: Option<&str>,
) -> Result<Vec<ShareInfo>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = if let Some(owner) = owner_id {
        let mut s = conn
            .prepare(
                "SELECT id, file_name, file_size, password_hash, expires_at, created_at
             FROM shared_links WHERE revoked = 0 AND owner_id = ? ORDER BY created_at DESC",
            )
            .map_err(|e| e.to_string())?;
        s.bind((1, owner)).map_err(|e| e.to_string())?;
        s
    } else {
        conn.prepare(
            "SELECT id, file_name, file_size, password_hash, expires_at, created_at
             FROM shared_links WHERE revoked = 0 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?
    };

    let mut shares = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let id = stmt.read::<String, _>("id").map_err(|e| e.to_string())?;
        shares.push(ShareInfo {
            id: id.clone(),
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|e| e.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|e| e.to_string())?,
            created_at: stmt
                .read::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
            expires_at: stmt.read::<Option<i64>, _>("expires_at").ok().flatten(),
            has_password: stmt
                .read::<Option<String>, _>("password_hash")
                .ok()
                .flatten()
                .is_some(),
            link: build_share_link(base_url, &id),
        });
    }
    Ok(shares)
}

pub fn revoke_share(db_pool: &DbConnection, id: &str) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("UPDATE shared_links SET revoked = 1 WHERE id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

/// Revoke only if share belongs to `owner_id` (multi-tenant isolation).
pub fn revoke_share_for_owner(
    db_pool: &DbConnection,
    id: &str,
    owner_id: &str,
) -> Result<bool, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut check = conn
        .prepare("SELECT owner_id FROM shared_links WHERE id = ? AND revoked = 0")
        .map_err(|e| e.to_string())?;
    check.bind((1, id)).map_err(|e| e.to_string())?;
    let owner = if let sqlite::State::Row = check.next().map_err(|e| e.to_string())? {
        check
            .read::<Option<String>, _>("owner_id")
            .ok()
            .flatten()
            .unwrap_or_default()
    } else {
        return Ok(false);
    };
    if owner != owner_id {
        return Err("Share not owned by this tenant".to_string());
    }
    revoke_share(db_pool, id)?;
    Ok(true)
}

/// Re-export cleanup function for convenience.
pub use crate::db::cleanup_expired_shares as cleanup_expired;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db_at;

    #[test]
    fn create_share_has_token_and_link() {
        let dir = std::env::temp_dir().join(format!("td-share-{}", uuid::Uuid::new_v4()));
        let db = init_db_at(&dir).expect("db");
        let info = create_share(
            &db,
            "http://test.local",
            Some(-100),
            42,
            "demo.bin".to_string(),
            1024,
            None,
            Some(24),
            Some("tenant:test"),
        )
        .expect("share");
        assert_eq!(info.file_name, "demo.bin");
        assert!(info.link.contains("/d/"));
        assert_eq!(info.id.len(), 64);
        let listed = list_shares(&db, "http://test.local", Some("tenant:test")).expect("list");
        assert_eq!(listed.len(), 1);
        revoke_share(&db, &info.id).expect("revoke");
        let _ = std::fs::remove_dir_all(dir);
    }
}
