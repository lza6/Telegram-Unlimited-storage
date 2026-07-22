use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub type DbConnection = Arc<Mutex<sqlite::Connection>>;

pub fn init_db_at(data_dir: &Path) -> Result<DbConnection, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let db_path = data_dir.join("shares.db");
    open_db(db_path)
}

pub fn init_db(app: &AppHandle) -> Result<DbConnection, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    init_db_at(&dir)
}

fn open_db(db_path: std::path::PathBuf) -> Result<DbConnection, String> {
    let conn = sqlite::open(db_path).map_err(|e| e.to_string())?;

    // Shared links table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS shared_links (
            id TEXT PRIMARY KEY,
            folder_id INTEGER,
            message_id INTEGER NOT NULL,
            file_name TEXT NOT NULL,
            file_size INTEGER NOT NULL DEFAULT 0,
            password_hash TEXT,
            password_salt TEXT,
            expires_at INTEGER,
            revoked INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;

    // Indexes for shares
    conn.execute("CREATE INDEX IF NOT EXISTS idx_shares_expires ON shared_links(expires_at)")
        .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_shares_revoked ON shared_links(revoked, created_at)",
    )
    .map_err(|e| e.to_string())?;
    let _ = conn.execute("ALTER TABLE shared_links ADD COLUMN owner_id TEXT");
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_shares_owner ON shared_links(owner_id, created_at DESC)",
    );

    // Upload sessions for resumable chunk uploads
    conn.execute(
        "CREATE TABLE IF NOT EXISTS upload_sessions (
            session_id TEXT PRIMARY KEY,
            filename TEXT NOT NULL,
            total_chunks INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            manifest_file_id TEXT,
            created_at INTEGER NOT NULL,
            expires_at INTEGER NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS upload_chunks (
            session_id TEXT NOT NULL,
            chunk_index INTEGER NOT NULL,
            file_id TEXT,
            sha256 TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at INTEGER NOT NULL,
            PRIMARY KEY (session_id, chunk_index),
            FOREIGN KEY (session_id) REFERENCES upload_sessions(session_id)
        )",
    )
    .map_err(|e| e.to_string())?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_upload_session ON upload_chunks(session_id)")
        .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS bot_file_map (
            message_id INTEGER PRIMARY KEY,
            telegram_file_id TEXT NOT NULL,
            file_name TEXT NOT NULL DEFAULT '',
            file_size INTEGER NOT NULL DEFAULT 0,
            caption TEXT,
            created_at INTEGER NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_bot_file_created ON bot_file_map(created_at DESC)",
    )
    .map_err(|e| e.to_string())?;

    let _ = conn
        .execute("ALTER TABLE bot_file_map ADD COLUMN bot_pool_index INTEGER NOT NULL DEFAULT 0");

    crate::metadata_cache::init_schema(&conn)?;

    init_tenant_tables(&conn)?;
    init_file_asset_tables(&conn)?;
    init_app_meta_table(&conn)?;

    log::info!("SQLite database initialized successfully using sqlite crate.");
    Ok(Arc::new(Mutex::new(conn)))
}

// ── Tenants (API keys → tenant_id) ───────────────────────────────────────

pub fn init_tenant_tables(conn: &sqlite::Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tenants (
            tenant_id TEXT PRIMARY KEY,
            api_key_hash TEXT NOT NULL,
            display_name TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn count_tenants(db_pool: &DbConnection) -> Result<usize, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT COUNT(*) AS c FROM tenants WHERE enabled = 1")
        .map_err(|e| e.to_string())?;
    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        Ok(stmt.read::<i64, _>("c").map_err(|e| e.to_string())? as usize)
    } else {
        Ok(0)
    }
}

pub fn upsert_tenant(
    db_pool: &DbConnection,
    tenant_id: &str,
    api_key_hash: String,
    display_name: Option<&str>,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let mut stmt = conn
        .prepare(
            "INSERT INTO tenants (tenant_id, api_key_hash, display_name, enabled, created_at)
             VALUES (?, ?, ?, 1, ?)
             ON CONFLICT(tenant_id) DO UPDATE SET
               api_key_hash = excluded.api_key_hash,
               display_name = COALESCE(excluded.display_name, tenants.display_name)",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, tenant_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, api_key_hash.as_str()))
        .map_err(|e| e.to_string())?;
    stmt.bind((3, display_name)).map_err(|e| e.to_string())?;
    stmt.bind((4, now)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn find_tenant_id_by_api_key(
    db_pool: &DbConnection,
    plaintext_key: &str,
) -> Result<Option<String>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT tenant_id, api_key_hash FROM tenants WHERE enabled = 1")
        .map_err(|e| e.to_string())?;
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let id = stmt
            .read::<String, _>("tenant_id")
            .map_err(|e| e.to_string())?;
        let hash = stmt
            .read::<String, _>("api_key_hash")
            .map_err(|e| e.to_string())?;
        if crate::commands::api_settings::verify_key(plaintext_key, &hash) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

// ── File assets (ownership index) ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileAssetRecord {
    pub message_id: i32,
    pub folder_id: Option<i64>,
    pub owner_id: String,
    pub file_name: String,
    pub file_size: i64,
    pub created_at: i64,
}

pub fn init_file_asset_tables(conn: &sqlite::Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS file_assets (
            message_id INTEGER PRIMARY KEY,
            folder_id INTEGER,
            owner_id TEXT NOT NULL,
            file_name TEXT NOT NULL,
            file_size INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_assets_owner ON file_assets(owner_id, created_at DESC)",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_file_asset(
    db_pool: &DbConnection,
    message_id: i32,
    folder_id: Option<i64>,
    owner_id: &str,
    file_name: &str,
    file_size: i64,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let mut stmt = conn
        .prepare(
            "INSERT INTO file_assets (message_id, folder_id, owner_id, file_name, file_size, created_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(message_id) DO UPDATE SET
               folder_id = excluded.folder_id,
               owner_id = excluded.owner_id,
               file_name = excluded.file_name,
               file_size = excluded.file_size",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, message_id as i64))
        .map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.bind((3, owner_id)).map_err(|e| e.to_string())?;
    stmt.bind((4, file_name)).map_err(|e| e.to_string())?;
    stmt.bind((5, file_size)).map_err(|e| e.to_string())?;
    stmt.bind((6, now)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_file_asset(
    db_pool: &DbConnection,
    message_id: i32,
) -> Result<Option<FileAssetRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE message_id = ?",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, message_id as i64))
        .map_err(|e| e.to_string())?;
    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        Ok(Some(FileAssetRecord {
            message_id: stmt
                .read::<i64, _>("message_id")
                .map_err(|e| e.to_string())? as i32,
            folder_id: stmt.read::<Option<i64>, _>("folder_id").ok().flatten(),
            owner_id: stmt
                .read::<String, _>("owner_id")
                .map_err(|e| e.to_string())?,
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|e| e.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|e| e.to_string())?,
            created_at: stmt
                .read::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
        }))
    } else {
        Ok(None)
    }
}

pub fn list_file_assets_by_owner(
    db_pool: &DbConnection,
    owner_id: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<FileAssetRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE owner_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, owner_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, limit as i64)).map_err(|e| e.to_string())?;
    stmt.bind((3, offset as i64)).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        out.push(FileAssetRecord {
            message_id: stmt
                .read::<i64, _>("message_id")
                .map_err(|e| e.to_string())? as i32,
            folder_id: stmt.read::<Option<i64>, _>("folder_id").ok().flatten(),
            owner_id: stmt
                .read::<String, _>("owner_id")
                .map_err(|e| e.to_string())?,
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|e| e.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|e| e.to_string())?,
            created_at: stmt
                .read::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

pub fn list_all_file_assets(
    db_pool: &DbConnection,
    limit: usize,
    offset: usize,
) -> Result<Vec<FileAssetRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, limit as i64)).map_err(|e| e.to_string())?;
    stmt.bind((2, offset as i64)).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        out.push(FileAssetRecord {
            message_id: stmt
                .read::<i64, _>("message_id")
                .map_err(|e| e.to_string())? as i32,
            folder_id: stmt.read::<Option<i64>, _>("folder_id").ok().flatten(),
            owner_id: stmt
                .read::<String, _>("owner_id")
                .map_err(|e| e.to_string())?,
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|e| e.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|e| e.to_string())?,
            created_at: stmt
                .read::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
        });
    }
    Ok(out)
}

fn read_file_asset_row(stmt: &mut sqlite::Statement<'_>) -> Result<FileAssetRecord, String> {
    Ok(FileAssetRecord {
        message_id: stmt
            .read::<i64, _>("message_id")
            .map_err(|e| e.to_string())? as i32,
        folder_id: stmt.read::<Option<i64>, _>("folder_id").ok().flatten(),
        owner_id: stmt
            .read::<String, _>("owner_id")
            .map_err(|e| e.to_string())?,
        file_name: stmt
            .read::<String, _>("file_name")
            .map_err(|e| e.to_string())?,
        file_size: stmt
            .read::<i64, _>("file_size")
            .map_err(|e| e.to_string())?,
        created_at: stmt
            .read::<i64, _>("created_at")
            .map_err(|e| e.to_string())?,
    })
}

/// List file_assets with optional owner + folder scope + name filter (SQL-level pagination).
pub fn list_file_assets_scoped(
    db_pool: &DbConnection,
    owner_id: Option<&str>,
    folder_id: Option<i64>,
    has_folder_scope: bool,
    name_contains: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<FileAssetRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut sql = String::from(
        "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at FROM file_assets WHERE 1=1",
    );
    if owner_id.is_some() {
        sql.push_str(" AND owner_id = ?");
    }
    if has_folder_scope {
        if folder_id.is_some() {
            sql.push_str(" AND folder_id = ?");
        } else {
            sql.push_str(" AND folder_id IS NULL");
        }
    }
    if name_contains.is_some() {
        sql.push_str(" AND file_name LIKE ? COLLATE NOCASE");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

    let pattern = name_contains.map(|q| format!("%{}%", q.replace('%', "").replace('_', "")));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut idx = 1usize;
    if let Some(owner) = owner_id {
        stmt.bind((idx, owner)).map_err(|e| e.to_string())?;
        idx += 1;
        if has_folder_scope {
            if let Some(fid) = folder_id {
                stmt.bind((idx, fid)).map_err(|e| e.to_string())?;
                idx += 1;
            }
        }
    } else if has_folder_scope {
        if let Some(fid) = folder_id {
            stmt.bind((idx, fid)).map_err(|e| e.to_string())?;
            idx += 1;
        }
    }
    if let Some(ref pat) = pattern {
        stmt.bind((idx, pat.as_str())).map_err(|e| e.to_string())?;
        idx += 1;
    }
    stmt.bind((idx, limit as i64)).map_err(|e| e.to_string())?;
    idx += 1;
    stmt.bind((idx, offset as i64)).map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        out.push(read_file_asset_row(&mut stmt)?);
    }
    Ok(out)
}

pub fn count_file_assets_scoped(
    db_pool: &DbConnection,
    owner_id: Option<&str>,
    folder_id: Option<i64>,
    has_folder_scope: bool,
    name_contains: Option<&str>,
) -> Result<usize, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut sql = String::from("SELECT COUNT(*) FROM file_assets WHERE 1=1");
    if owner_id.is_some() {
        sql.push_str(" AND owner_id = ?");
    }
    if has_folder_scope {
        if folder_id.is_some() {
            sql.push_str(" AND folder_id = ?");
        } else {
            sql.push_str(" AND folder_id IS NULL");
        }
    }
    if name_contains.is_some() {
        sql.push_str(" AND file_name LIKE ? COLLATE NOCASE");
    }

    let pattern = name_contains.map(|q| format!("%{}%", q.replace('%', "").replace('_', "")));

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let mut idx = 1usize;
    if let Some(owner) = owner_id {
        stmt.bind((idx, owner)).map_err(|e| e.to_string())?;
        idx += 1;
        if has_folder_scope {
            if let Some(fid) = folder_id {
                stmt.bind((idx, fid)).map_err(|e| e.to_string())?;
                idx += 1;
            }
        }
    } else if has_folder_scope {
        if let Some(fid) = folder_id {
            stmt.bind((idx, fid)).map_err(|e| e.to_string())?;
            idx += 1;
        }
    }
    if let Some(ref pat) = pattern {
        stmt.bind((idx, pat.as_str())).map_err(|e| e.to_string())?;
    }

    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let n: i64 = stmt.read(0).map_err(|e| e.to_string())?;
        return Ok(n as usize);
    }
    Ok(0)
}

/// Total rows in file_assets (enables User-mode API search/list via local index).
pub fn count_all_file_assets(db_pool: &DbConnection) -> Result<usize, String> {
    count_file_assets_scoped(db_pool, None, None, false, None)
}

// ── App metadata (index completion flags, etc.) ───────────────────────────

const META_FILE_INDEX_COMPLETE: &str = "file_index_complete";

pub fn init_app_meta_table(conn: &sqlite::Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn set_app_meta(db_pool: &DbConnection, key: &str, value: &str) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "INSERT INTO app_meta (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, key)).map_err(|e| e.to_string())?;
    stmt.bind((2, value)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

fn get_app_meta(db_pool: &DbConnection, key: &str) -> Result<Option<String>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM app_meta WHERE key = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, key)).map_err(|e| e.to_string())?;
    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        Ok(Some(stmt.read::<String, _>(0).map_err(|e| e.to_string())?))
    } else {
        Ok(None)
    }
}

/// User-mode search/get may trust DB only after an explicit full rebuild (Sync).
pub fn is_file_index_complete(db_pool: &DbConnection) -> Result<bool, String> {
    Ok(get_app_meta(db_pool, META_FILE_INDEX_COMPLETE)?
        .map(|v| v == "1")
        .unwrap_or(false))
}

pub fn set_file_index_complete(db_pool: &DbConnection, complete: bool) -> Result<(), String> {
    set_app_meta(
        db_pool,
        META_FILE_INDEX_COMPLETE,
        if complete { "1" } else { "0" },
    )
}

/// Remove all indexed files for an owner (used before full rebuild).
pub fn delete_all_file_assets_for_owner(
    db_pool: &DbConnection,
    owner_id: &str,
) -> Result<usize, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("DELETE FROM file_assets WHERE owner_id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, owner_id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(conn.change_count() as usize)
}

/// Remove indexed files scoped to one folder (Saved Messages when folder_id is None).
pub fn delete_file_assets_in_folder(
    db_pool: &DbConnection,
    folder_id: Option<i64>,
    owner_id: &str,
) -> Result<usize, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let changed = if let Some(fid) = folder_id {
        let mut stmt = conn
            .prepare("DELETE FROM file_assets WHERE owner_id = ? AND folder_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind((1, owner_id)).map_err(|e| e.to_string())?;
        stmt.bind((2, fid)).map_err(|e| e.to_string())?;
        stmt.next().map_err(|e| e.to_string())?;
        conn.change_count()
    } else {
        let mut stmt = conn
            .prepare("DELETE FROM file_assets WHERE owner_id = ? AND folder_id IS NULL")
            .map_err(|e| e.to_string())?;
        stmt.bind((1, owner_id)).map_err(|e| e.to_string())?;
        stmt.next().map_err(|e| e.to_string())?;
        conn.change_count()
    };
    Ok(changed as usize)
}

pub fn delete_file_asset_by_name(
    db_pool: &DbConnection,
    owner_id: &str,
    file_name: &str,
) -> Result<bool, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("DELETE FROM file_assets WHERE owner_id = ? AND file_name = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, owner_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, file_name)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(conn.change_count() > 0)
}

/// Delete asset index row by Telegram message id (Bot mode bulk delete).
pub fn delete_file_asset(
    db_pool: &DbConnection,
    message_id: i32,
    owner_id: Option<&str>,
) -> Result<bool, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let changed = if let Some(owner) = owner_id {
        let mut stmt = conn
            .prepare("DELETE FROM file_assets WHERE message_id = ? AND owner_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind((1, message_id as i64))
            .map_err(|e| e.to_string())?;
        stmt.bind((2, owner)).map_err(|e| e.to_string())?;
        stmt.next().map_err(|e| e.to_string())?;
        conn.change_count() > 0
    } else {
        let mut stmt = conn
            .prepare("DELETE FROM file_assets WHERE message_id = ?")
            .map_err(|e| e.to_string())?;
        stmt.bind((1, message_id as i64))
            .map_err(|e| e.to_string())?;
        stmt.next().map_err(|e| e.to_string())?;
        conn.change_count() > 0
    };
    Ok(changed)
}

/// Search file_assets by name (Bot / tenant index path).
pub fn search_file_assets(
    db_pool: &DbConnection,
    query: &str,
    owner_id: Option<&str>,
    folder_id: Option<i64>,
    has_folder_scope: bool,
    limit: usize,
) -> Result<Vec<FileAssetRecord>, String> {
    let pattern = format!("%{}%", query.replace('%', "").replace('_', ""));
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let sql = match (owner_id, has_folder_scope, folder_id) {
        (Some(_), true, Some(_)) => {
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE owner_id = ? AND folder_id = ? AND file_name LIKE ? COLLATE NOCASE
             ORDER BY created_at DESC LIMIT ?"
        }
        (Some(_), true, None) => {
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE owner_id = ? AND folder_id IS NULL AND file_name LIKE ? COLLATE NOCASE
             ORDER BY created_at DESC LIMIT ?"
        }
        (Some(_), false, _) => {
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE owner_id = ? AND file_name LIKE ? COLLATE NOCASE
             ORDER BY created_at DESC LIMIT ?"
        }
        (None, true, Some(_)) => {
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE folder_id = ? AND file_name LIKE ? COLLATE NOCASE
             ORDER BY created_at DESC LIMIT ?"
        }
        (None, true, None) => {
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE folder_id IS NULL AND file_name LIKE ? COLLATE NOCASE
             ORDER BY created_at DESC LIMIT ?"
        }
        (None, false, _) => {
            "SELECT message_id, folder_id, owner_id, file_name, file_size, created_at
             FROM file_assets WHERE file_name LIKE ? COLLATE NOCASE
             ORDER BY created_at DESC LIMIT ?"
        }
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut idx = 1usize;
    if let Some(owner) = owner_id {
        stmt.bind((idx, owner)).map_err(|e| e.to_string())?;
        idx += 1;
        if has_folder_scope {
            if let Some(fid) = folder_id {
                stmt.bind((idx, fid)).map_err(|e| e.to_string())?;
                idx += 1;
            }
        }
        stmt.bind((idx, pattern.as_str()))
            .map_err(|e| e.to_string())?;
        idx += 1;
        stmt.bind((idx, limit as i64)).map_err(|e| e.to_string())?;
    } else {
        if has_folder_scope {
            if let Some(fid) = folder_id {
                stmt.bind((idx, fid)).map_err(|e| e.to_string())?;
                idx += 1;
            }
        }
        stmt.bind((idx, pattern.as_str()))
            .map_err(|e| e.to_string())?;
        idx += 1;
        stmt.bind((idx, limit as i64)).map_err(|e| e.to_string())?;
    }
    let mut out = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        out.push(read_file_asset_row(&mut stmt)?);
    }
    Ok(out)
}

// ── Expired shares cleanup ────────────────────────────────────────────────

pub fn cleanup_expired_shares(db_pool: &DbConnection) -> Result<usize, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let mut stmt = conn
        .prepare("UPDATE shared_links SET revoked = 1 WHERE expires_at IS NOT NULL AND expires_at < ? AND revoked = 0")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, now)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    let cleaned = conn.change_count() as usize;
    if cleaned > 0 {
        log::info!("Cleaned up {} expired share(s)", cleaned);
    }
    Ok(cleaned)
}

// ── Upload session management (resumable + integrity) ────────────────────

#[derive(Debug, Clone)]
pub struct UploadChunkRecord {
    pub chunk_index: i32,
    pub file_id: Option<String>,
    pub sha256: Option<String>,
    pub status: String,
}

pub fn create_upload_session(
    db_pool: &DbConnection,
    session_id: &str,
    filename: &str,
    total_chunks: i32,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let expires = now + 86400 * 7; // 7 days

    let mut stmt = conn
        .prepare("INSERT OR IGNORE INTO upload_sessions (session_id, filename, total_chunks, status, created_at, expires_at) VALUES (?, ?, ?, 'active', ?, ?)")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, session_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, filename)).map_err(|e| e.to_string())?;
    stmt.bind((3, total_chunks as i64))
        .map_err(|e| e.to_string())?;
    stmt.bind((4, now)).map_err(|e| e.to_string())?;
    stmt.bind((5, expires)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;

    // Pre-create chunk rows for fast status queries
    for i in 0..total_chunks {
        let mut c = conn
            .prepare("INSERT OR IGNORE INTO upload_chunks (session_id, chunk_index, status, created_at) VALUES (?, ?, 'pending', ?)")
            .map_err(|e| e.to_string())?;
        c.bind((1, session_id)).map_err(|e| e.to_string())?;
        c.bind((2, i as i64)).map_err(|e| e.to_string())?;
        c.bind((3, now)).map_err(|e| e.to_string())?;
        c.next().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn record_upload_chunk(
    db_pool: &DbConnection,
    session_id: &str,
    chunk_index: i32,
    file_id: &str,
    sha256: &str,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("UPDATE upload_chunks SET file_id = ?, sha256 = ?, status = 'uploaded' WHERE session_id = ? AND chunk_index = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, file_id)).map_err(|e| e.to_string())?;
    stmt.bind((2, sha256)).map_err(|e| e.to_string())?;
    stmt.bind((3, session_id)).map_err(|e| e.to_string())?;
    stmt.bind((4, chunk_index as i64))
        .map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_upload_session_chunks(
    db_pool: &DbConnection,
    session_id: &str,
) -> Result<Vec<UploadChunkRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT chunk_index, file_id, sha256, status FROM upload_chunks WHERE session_id = ? ORDER BY chunk_index")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, session_id)).map_err(|e| e.to_string())?;

    let mut chunks = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        chunks.push(UploadChunkRecord {
            chunk_index: stmt
                .read::<i64, _>("chunk_index")
                .map_err(|e| e.to_string())? as i32,
            file_id: stmt.read::<Option<String>, _>("file_id").ok().flatten(),
            sha256: stmt.read::<Option<String>, _>("sha256").ok().flatten(),
            status: stmt
                .read::<String, _>("status")
                .map_err(|e| e.to_string())?,
        });
    }
    Ok(chunks)
}

pub fn get_upload_session_summary(
    db_pool: &DbConnection,
    session_id: &str,
) -> Result<Option<(i32, String, String)>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT total_chunks, status, filename FROM upload_sessions WHERE session_id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, session_id)).map_err(|e| e.to_string())?;

    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let total = stmt
            .read::<i64, _>("total_chunks")
            .map_err(|e| e.to_string())? as i32;
        let status = stmt
            .read::<String, _>("status")
            .map_err(|e| e.to_string())?;
        let filename = stmt
            .read::<String, _>("filename")
            .map_err(|e| e.to_string())?;
        Ok(Some((total, status, filename)))
    } else {
        Ok(None)
    }
}

pub fn get_upload_session_manifest_file_id(
    db_pool: &DbConnection,
    session_id: &str,
) -> Result<Option<String>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT manifest_file_id FROM upload_sessions WHERE session_id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, session_id)).map_err(|e| e.to_string())?;
    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        Ok(stmt
            .read::<Option<String>, _>("manifest_file_id")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty()))
    } else {
        Ok(None)
    }
}

pub fn complete_upload_session(
    db_pool: &DbConnection,
    session_id: &str,
    manifest_file_id: &str,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("UPDATE upload_sessions SET status = 'completed', manifest_file_id = ? WHERE session_id = ?")
        .map_err(|e| e.to_string())?;
    stmt.bind((1, manifest_file_id))
        .map_err(|e| e.to_string())?;
    stmt.bind((2, session_id)).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

/// Clean up stale upload sessions older than expiry.
pub fn cleanup_stale_uploads(db_pool: &DbConnection) -> Result<usize, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();

    // Delete chunks first (foreign key constraint)
    let mut del_chunks = conn
        .prepare("DELETE FROM upload_chunks WHERE session_id IN (SELECT session_id FROM upload_sessions WHERE expires_at < ?)")
        .map_err(|e| e.to_string())?;
    del_chunks.bind((1, now)).map_err(|e| e.to_string())?;
    let mut chunk_count = 0usize;
    while let sqlite::State::Row = del_chunks.next().map_err(|e| e.to_string())? {
        chunk_count += 1;
    }

    let mut del_sessions = conn
        .prepare("DELETE FROM upload_sessions WHERE expires_at < ?")
        .map_err(|e| e.to_string())?;
    del_sessions.bind((1, now)).map_err(|e| e.to_string())?;
    let mut session_count = 0usize;
    while let sqlite::State::Row = del_sessions.next().map_err(|e| e.to_string())? {
        session_count += 1;
    }

    if session_count > 0 {
        log::info!(
            "Cleaned up {} stale upload session(s) with {} chunk(s)",
            session_count,
            chunk_count
        );
    }
    Ok(session_count)
}

#[derive(Debug, Clone)]
pub struct BotFileRecord {
    pub message_id: i32,
    pub telegram_file_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub caption: Option<String>,
    pub bot_pool_index: u32,
}

pub fn upsert_bot_file_map(
    db_pool: &DbConnection,
    message_id: i32,
    telegram_file_id: &str,
    file_name: &str,
    file_size: u64,
    caption: Option<&str>,
    bot_pool_index: u32,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let mut stmt = conn
        .prepare(
            "INSERT INTO bot_file_map (message_id, telegram_file_id, file_name, file_size, caption, created_at, bot_pool_index)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(message_id) DO UPDATE SET
               telegram_file_id = excluded.telegram_file_id,
               file_name = excluded.file_name,
               file_size = excluded.file_size,
               caption = excluded.caption,
               bot_pool_index = excluded.bot_pool_index",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, message_id as i64))
        .map_err(|e| e.to_string())?;
    stmt.bind((2, telegram_file_id))
        .map_err(|e| e.to_string())?;
    stmt.bind((3, file_name)).map_err(|e| e.to_string())?;
    stmt.bind((4, file_size as i64))
        .map_err(|e| e.to_string())?;
    stmt.bind((5, caption.unwrap_or("")))
        .map_err(|e| e.to_string())?;
    stmt.bind((6, now)).map_err(|e| e.to_string())?;
    stmt.bind((7, bot_pool_index as i64))
        .map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_bot_file_map(
    db_pool: &DbConnection,
    message_id: i32,
) -> Result<Option<BotFileRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT message_id, telegram_file_id, file_name, file_size, caption, bot_pool_index FROM bot_file_map WHERE message_id = ?",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, message_id as i64))
        .map_err(|e| e.to_string())?;
    if let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let caption = stmt
            .read::<Option<String>, _>("caption")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty());
        return Ok(Some(BotFileRecord {
            message_id: stmt
                .read::<i64, _>("message_id")
                .map_err(|e| e.to_string())? as i32,
            telegram_file_id: stmt
                .read::<String, _>("telegram_file_id")
                .map_err(|e| e.to_string())?,
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|e| e.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|e| e.to_string())? as u64,
            caption,
            bot_pool_index: stmt.read::<i64, _>("bot_pool_index").unwrap_or(0) as u32,
        }));
    }
    Ok(None)
}

pub fn list_bot_files(
    db_pool: &DbConnection,
    limit: usize,
    offset: usize,
) -> Result<Vec<BotFileRecord>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT message_id, telegram_file_id, file_name, file_size, caption, bot_pool_index FROM bot_file_map ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .map_err(|e| e.to_string())?;
    stmt.bind((1, limit as i64)).map_err(|e| e.to_string())?;
    stmt.bind((2, offset as i64)).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let caption = stmt
            .read::<Option<String>, _>("caption")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty());
        rows.push(BotFileRecord {
            message_id: stmt
                .read::<i64, _>("message_id")
                .map_err(|e| e.to_string())? as i32,
            telegram_file_id: stmt
                .read::<String, _>("telegram_file_id")
                .map_err(|e| e.to_string())?,
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|e| e.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|e| e.to_string())? as u64,
            caption,
            bot_pool_index: stmt.read::<i64, _>("bot_pool_index").unwrap_or(0) as u32,
        });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> DbConnection {
        let dir = std::env::temp_dir().join(format!("td-db-test-{}", uuid::Uuid::new_v4()));
        init_db_at(&dir).expect("db")
    }

    #[test]
    fn file_asset_upsert_and_list() {
        let db = temp_db();
        upsert_file_asset(&db, 99, Some(-100), "tenant:a", "a.bin", 10).expect("upsert");
        let row = get_file_asset(&db, 99).expect("get").expect("row");
        assert_eq!(row.owner_id, "tenant:a");
        let list = list_file_assets_by_owner(&db, "tenant:a", 10, 0).expect("list");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn tenant_bootstrap_and_lookup() {
        let db = temp_db();
        let hash = crate::commands::api_settings::hash_key_public("secret-key");
        upsert_tenant(&db, "acme", hash, Some("Acme")).expect("tenant");
        let id = find_tenant_id_by_api_key(&db, "secret-key").expect("find");
        assert_eq!(id.as_deref(), Some("acme"));
    }

    #[test]
    fn share_expiry_cleanup() {
        let db = temp_db();
        let info = crate::sharing_core::create_share(
            &db,
            "http://test",
            None,
            1,
            "x".to_string(),
            1,
            None,
            Some(-1),
            Some("system:web"),
        )
        .expect("share");
        let n = cleanup_expired_shares(&db).expect("cleanup");
        assert!(n >= 1);
        let _ = info;
    }

    #[test]
    fn list_file_assets_scoped_by_folder_paginates() {
        let db = temp_db();
        upsert_file_asset(&db, 1, Some(100), "admin", "a.bin", 10).expect("upsert");
        upsert_file_asset(&db, 2, Some(100), "admin", "b.bin", 10).expect("upsert");
        upsert_file_asset(&db, 3, Some(200), "admin", "c.bin", 10).expect("upsert");
        let total = count_file_assets_scoped(&db, None, Some(100), true, None).expect("count");
        assert_eq!(total, 2);
        let page1 = list_file_assets_scoped(&db, None, Some(100), true, None, 1, 0).expect("page1");
        assert_eq!(page1.len(), 1);
        let page2 = list_file_assets_scoped(&db, None, Some(100), true, None, 1, 1).expect("page2");
        assert_eq!(page2.len(), 1);
        assert_ne!(page1[0].message_id, page2[0].message_id);
    }

    #[test]
    fn list_file_assets_scoped_name_filter_paginates_in_sql() {
        let db = temp_db();
        upsert_file_asset(&db, 20, Some(100), "admin", "alpha-report.pdf", 10).expect("upsert");
        upsert_file_asset(&db, 21, Some(100), "admin", "beta-report.pdf", 10).expect("upsert");
        upsert_file_asset(&db, 22, Some(100), "admin", "gamma.bin", 10).expect("upsert");
        let total =
            count_file_assets_scoped(&db, None, Some(100), true, Some("report")).expect("count");
        assert_eq!(total, 2);
        let page1 =
            list_file_assets_scoped(&db, None, Some(100), true, Some("report"), 1, 0).expect("p1");
        assert_eq!(page1.len(), 1);
        assert!(page1[0].file_name.contains("report"));
    }

    #[test]
    fn count_all_file_assets_totals_every_folder() {
        let db = temp_db();
        upsert_file_asset(&db, 1, Some(100), "admin", "a.bin", 10).expect("upsert");
        upsert_file_asset(&db, 2, None, "admin", "b.bin", 10).expect("upsert");
        assert_eq!(count_all_file_assets(&db).expect("count"), 2);
    }

    #[test]
    fn delete_all_file_assets_for_owner_clears_rows() {
        let db = temp_db();
        upsert_file_asset(&db, 1, Some(100), "admin", "a.bin", 10).expect("upsert");
        upsert_file_asset(&db, 2, None, "admin", "b.bin", 10).expect("upsert");
        upsert_file_asset(&db, 3, Some(100), "other", "c.bin", 10).expect("upsert");
        let n = delete_all_file_assets_for_owner(&db, "admin").expect("delete");
        assert_eq!(n, 2);
        assert_eq!(count_all_file_assets(&db).expect("count"), 1);
    }

    #[test]
    fn file_index_complete_flag_persists() {
        let db = temp_db();
        assert!(!is_file_index_complete(&db).expect("read"));
        set_file_index_complete(&db, true).expect("set");
        assert!(is_file_index_complete(&db).expect("read"));
        set_file_index_complete(&db, false).expect("clear");
        assert!(!is_file_index_complete(&db).expect("read"));
    }

    #[test]
    fn search_file_assets_respects_folder_scope() {
        let db = temp_db();
        upsert_file_asset(&db, 10, Some(100), "admin", "report-alpha.pdf", 10).expect("upsert");
        upsert_file_asset(&db, 11, Some(200), "admin", "report-beta.pdf", 10).expect("upsert");
        let in_folder =
            search_file_assets(&db, "report", None, Some(100), true, 10).expect("search");
        assert_eq!(in_folder.len(), 1);
        assert_eq!(in_folder[0].file_name, "report-alpha.pdf");
    }
}
