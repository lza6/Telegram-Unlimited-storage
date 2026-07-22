use serde::Serialize;
use sha2::{Sha256, Digest};
use tauri::State;
use rand::Rng;
use crate::db::DbConnection;

#[derive(Debug, Serialize)]
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
    let bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[tauri::command]
pub async fn cmd_create_share(
    folder_id: Option<i64>,
    message_id: i32,
    file_name: String,
    file_size: i64,
    password: Option<String>,
    expiry_hours: Option<i64>,
    db_pool: State<'_, DbConnection>,
) -> Result<ShareInfo, String> {
    let token = generate_share_token();
    let created_at = chrono::Utc::now().timestamp();
    let expires_at = expiry_hours.map(|hours| created_at + hours * 3600);
    
    let (password_hash, password_salt) = if let Some(ref pwd) = password {
        if pwd.is_empty() {
            (None, None)
        } else {
            let mut rng = rand::thread_rng();
            let salt_bytes: Vec<u8> = (0..16).map(|_| rng.gen()).collect();
            let salt: String = salt_bytes.iter().map(|b| format!("{:02x}", b)).collect();
            let hash = hash_password(pwd, &salt);
            (Some(hash), Some(salt))
        }
    } else {
        (None, None)
    };

    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare(
        "INSERT INTO shared_links (id, folder_id, message_id, file_name, file_size, password_hash, password_salt, expires_at, revoked, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?)"
    ).map_err(|e| e.to_string())?;

    stmt.bind((1, token.as_str())).map_err(|e| e.to_string())?;
    stmt.bind((2, folder_id)).map_err(|e| e.to_string())?;
    stmt.bind((3, message_id as i64)).map_err(|e| e.to_string())?;
    stmt.bind((4, file_name.as_str())).map_err(|e| e.to_string())?;
    stmt.bind((5, file_size)).map_err(|e| e.to_string())?;
    stmt.bind((6, password_hash.as_deref())).map_err(|e| e.to_string())?;
    stmt.bind((7, password_salt.as_deref())).map_err(|e| e.to_string())?;
    stmt.bind((8, expires_at)).map_err(|e| e.to_string())?;
    stmt.bind((9, created_at)).map_err(|e| e.to_string())?;

    stmt.next().map_err(|e| e.to_string())?;

    let link = format!("http://localhost:{}/d/{}", crate::STREAM_PORT, token);

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

#[tauri::command]
pub async fn cmd_list_shares(
    db_pool: State<'_, DbConnection>,
) -> Result<Vec<ShareInfo>, String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, folder_id, message_id, file_name, file_size, password_hash, expires_at, created_at 
             FROM shared_links WHERE revoked = 0 ORDER BY created_at DESC"
        )
        .map_err(|e| e.to_string())?;

    let mut shares = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|e| e.to_string())? {
        let id = stmt.read::<String, _>("id").map_err(|e| e.to_string())?;
        let has_password = stmt.read::<Option<String>, _>("password_hash").ok().flatten().is_some();
        let expires_at = stmt.read::<Option<i64>, _>("expires_at").ok().flatten();
        let file_name = stmt.read::<String, _>("file_name").map_err(|e| e.to_string())?;
        let file_size = stmt.read::<i64, _>("file_size").map_err(|e| e.to_string())?;
        let created_at = stmt.read::<i64, _>("created_at").map_err(|e| e.to_string())?;
        let link = format!("http://localhost:{}/d/{}", crate::STREAM_PORT, id);
        
        shares.push(ShareInfo {
            id,
            file_name,
            file_size,
            created_at,
            expires_at,
            has_password,
            link,
        });
    }
    
    Ok(shares)
}

#[tauri::command]
pub async fn cmd_revoke_share(
    id: String,
    db_pool: State<'_, DbConnection>,
) -> Result<(), String> {
    let conn = db_pool.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("UPDATE shared_links SET revoked = 1 WHERE id = ?").map_err(|e| e.to_string())?;
    stmt.bind((1, id.as_str())).map_err(|e| e.to_string())?;
    stmt.next().map_err(|e| e.to_string())?;
    
    Ok(())
}
