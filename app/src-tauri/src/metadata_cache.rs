//! Telegram 列表元数据本地缓存（SQLite JSON blob + TTL）。
//! 单实例 7×24 部署适用；多实例需外置存储（见 docs/planning/元数据缓存与企业部署.md）。

use serde::{Deserialize, Serialize};

use crate::db::DbConnection;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedFolder {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedFile {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub size: u64,
    pub mime_type: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLayer {
    Hit,
    Miss,
    Bypass,
}

pub fn init_schema(conn: &sqlite::Connection) -> Result<(), String> {
    conn.execute("PRAGMA journal_mode=WAL").map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS metadata_cache (
            cache_key TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            payload TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_metadata_cache_kind ON metadata_cache(kind, updated_at)",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn folders_cache_key() -> &'static str {
    "folders:all"
}

pub fn files_cache_key(folder_id: Option<i64>) -> String {
    match folder_id {
        Some(id) => format!("files:folder:{id}"),
        None => "files:all".to_string(),
    }
}

pub fn get_folders(db: &DbConnection, ttl_secs: u64) -> Option<Vec<CachedFolder>> {
    get_payload(db, folders_cache_key(), "folders", ttl_secs)
}

pub fn put_folders(db: &DbConnection, folders: &[CachedFolder]) -> Result<(), String> {
    put_payload(db, folders_cache_key(), "folders", folders)
}

pub fn get_files(
    db: &DbConnection,
    key: &str,
    ttl_secs: u64,
) -> Option<Vec<CachedFile>> {
    get_payload(db, key, "files", ttl_secs)
}

pub fn put_files(db: &DbConnection, key: &str, files: &[CachedFile]) -> Result<(), String> {
    put_payload(db, key, "files", files)
}

pub fn invalidate_files(db: &DbConnection, folder_id: Option<i64>) {
    let _ = delete_key(db, &files_cache_key(folder_id));
    if folder_id.is_some() {
        let _ = delete_key(db, "files:all");
    }
}

pub fn invalidate_folders(db: &DbConnection) {
    let _ = delete_key(db, folders_cache_key());
}

pub fn cleanup_stale(db: &DbConnection, max_age_secs: u64) -> Result<usize, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let cutoff = chrono::Utc::now().timestamp() - max_age_secs as i64;
    let mut stmt = conn
        .prepare("DELETE FROM metadata_cache WHERE updated_at < ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, cutoff)).map_err(|e| e.to_string())?;
    let mut n = 0usize;
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        n += 1;
    }
    Ok(n)
}

fn get_payload<T: for<'de> Deserialize<'de>>(
    db: &DbConnection,
    key: &str,
    kind: &str,
    ttl_secs: u64,
) -> Option<T> {
    if ttl_secs == 0 {
        return None;
    }
    let conn = db.lock().ok()?;
    let mut stmt = conn
        .prepare("SELECT payload, updated_at FROM metadata_cache WHERE cache_key = ? AND kind = ?")
        .ok()?;
    stmt.bind((1, key)).ok()?;
    stmt.bind((2, kind)).ok()?;
    let sqlite::State::Row = stmt.next().ok()? else {
        return None;
    };
    let payload: String = stmt.read::<String, _>("payload").ok()?;
    let updated_at: i64 = stmt.read::<i64, _>("updated_at").ok()?;
    let now = chrono::Utc::now().timestamp();
    if now - updated_at > ttl_secs as i64 {
        return None;
    }
    serde_json::from_str(&payload).ok()
}

fn put_payload<T: Serialize + ?Sized>(
    db: &DbConnection,
    key: &str,
    kind: &str,
    value: &T,
) -> Result<(), String> {
    let payload = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "INSERT INTO metadata_cache (cache_key, kind, payload, updated_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(cache_key) DO UPDATE SET
               kind = excluded.kind,
               payload = excluded.payload,
               updated_at = excluded.updated_at",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, key)).map_err(|e| e.to_string())?;
    stmt.bind((2, kind)).map_err(|e| e.to_string())?;
    stmt.bind((3, payload.as_str()))
        .map_err(|e| e.to_string())?;
    stmt.bind((4, now)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

fn delete_key(db: &DbConnection, key: &str) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("DELETE FROM metadata_cache WHERE cache_key = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, key)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_roundtrip_and_ttl() {
        let dir = std::env::temp_dir().join(format!("td-mc-{}", uuid::Uuid::new_v4()));
        let db = crate::db::init_db_at(&dir).expect("db");
        let rows = vec![CachedFolder {
            id: -100,
            name: "demo".to_string(),
        }];
        put_folders(&db, &rows).expect("put");
        assert_eq!(get_folders(&db, 300), Some(rows));
        assert!(get_folders(&db, 0).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
