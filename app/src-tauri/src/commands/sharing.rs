use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use crate::db::DbConnection;
use crate::sharing_core;
use crate::tenant_auth::OWNER_WEB;

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

fn share_base_for_app(app: &AppHandle) -> Result<String, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(crate::ui_settings::share_base_url_from_data_dir(
        &dir,
        crate::STREAM_PORT,
    ))
}

#[tauri::command]
pub async fn cmd_create_share(
    app: AppHandle,
    folder_id: Option<i64>,
    message_id: i32,
    file_name: String,
    file_size: i64,
    password: Option<String>,
    expiry_hours: Option<i64>,
    db_pool: State<'_, DbConnection>,
) -> Result<ShareInfo, String> {
    if message_id <= 0 || file_name.trim().is_empty() {
        return Err("message_id must be positive and file_name is required".into());
    }
    let password = password.and_then(|p| {
        let t = p.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });
    let base = share_base_for_app(&app)?;
    let info = sharing_core::create_share(
        &db_pool,
        &base,
        folder_id,
        message_id,
        file_name,
        file_size,
        password,
        expiry_hours,
        Some(OWNER_WEB),
    )?;
    Ok(ShareInfo {
        id: info.id,
        file_name: info.file_name,
        file_size: info.file_size,
        created_at: info.created_at,
        expires_at: info.expires_at,
        has_password: info.has_password,
        link: info.link,
    })
}

#[tauri::command]
pub async fn cmd_list_shares(
    app: AppHandle,
    db_pool: State<'_, DbConnection>,
) -> Result<Vec<ShareInfo>, String> {
    let base = share_base_for_app(&app)?;
    let rows = sharing_core::list_shares(&db_pool, &base, None)?;
    Ok(rows
        .into_iter()
        .map(|info| ShareInfo {
            id: info.id,
            file_name: info.file_name,
            file_size: info.file_size,
            created_at: info.created_at,
            expires_at: info.expires_at,
            has_password: info.has_password,
            link: info.link,
        })
        .collect())
}

#[tauri::command]
pub async fn cmd_revoke_share(id: String, db_pool: State<'_, DbConnection>) -> Result<(), String> {
    sharing_core::revoke_share(&db_pool, &id)
}
