use tauri::{AppHandle, Manager};
use std::sync::{Arc, Mutex};

pub type DbConnection = Arc<Mutex<sqlite::Connection>>;

pub fn init_db(app: &AppHandle) -> Result<DbConnection, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db_path = dir.join("shares.db");
    
    let conn = sqlite::open(db_path).map_err(|e| e.to_string())?;
    
    // Run migration
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
        )"
    ).map_err(|e| e.to_string())?;
    
    log::info!("SQLite database initialized successfully using sqlite crate.");
    Ok(Arc::new(Mutex::new(conn)))
}
