use serde::{Deserialize, Serialize};

use crate::db::DbConnection;
use crate::telegram_transport::TelegramUploadReceipt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetLocatorRecord {
    pub asset_id: String,
    pub owner_id: String,
    pub transport_mode: String,
    pub storage_peer_id: i64,
    pub storage_peer_kind: String,
    pub message_id: i32,
    pub legacy_folder_id: Option<i64>,
    pub telegram_file_id: Option<String>,
    pub file_name: String,
    pub file_size: i64,
    pub bot_pool_index: Option<u32>,
    pub uploader_bot_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocatorResolution {
    Found(AssetLocatorRecord),
    NotFound,
    Ambiguous { candidates: usize },
}

pub fn init_asset_locator_table(conn: &sqlite::Connection) -> Result<(), String> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS asset_locators (
            asset_id TEXT PRIMARY KEY,
            owner_id TEXT NOT NULL,
            transport_mode TEXT NOT NULL CHECK (transport_mode IN ('bot','user')),
            storage_peer_id INTEGER NOT NULL,
            storage_peer_kind TEXT NOT NULL,
            message_id INTEGER NOT NULL,
            legacy_folder_id INTEGER,
            telegram_file_id TEXT,
            file_name TEXT NOT NULL,
            file_size INTEGER NOT NULL DEFAULT 0,
            bot_pool_index INTEGER,
            uploader_bot_id TEXT,
            locator_state TEXT NOT NULL DEFAULT 'ready' CHECK (locator_state IN ('ready','deleted','reconcile')),
            locator_version INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(owner_id, transport_mode, storage_peer_id, storage_peer_kind, message_id)
        )",
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_asset_locators_message_owner
         ON asset_locators(message_id, owner_id, legacy_folder_id)",
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_asset_locators_peer_message
         ON asset_locators(storage_peer_id, storage_peer_kind, message_id)",
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn upsert_from_receipt(
    db: &DbConnection,
    receipt: &TelegramUploadReceipt,
    transport_mode: &str,
    legacy_folder_id: Option<i64>,
    owner_id: &str,
) -> Result<AssetLocatorRecord, String> {
    if !matches!(transport_mode, "bot" | "user") {
        return Err("Unsupported asset locator transport mode".to_string());
    }
    if receipt.storage_peer_id == 0 || receipt.message_id <= 0 || owner_id.trim().is_empty() {
        return Err("Asset locator identity is incomplete".to_string());
    }
    let conn = db.get().map_err(|error| error.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let existing_id = {
        let mut stmt = conn
            .prepare(
                "SELECT asset_id FROM asset_locators
                 WHERE owner_id=? AND transport_mode=? AND storage_peer_id=?
                   AND storage_peer_kind=? AND message_id=?",
            )
            .map_err(|error| error.to_string())?;
        stmt.bind((1, owner_id))
            .map_err(|error| error.to_string())?;
        stmt.bind((2, transport_mode))
            .map_err(|error| error.to_string())?;
        stmt.bind((3, receipt.storage_peer_id))
            .map_err(|error| error.to_string())?;
        stmt.bind((4, receipt.storage_peer_kind.as_str()))
            .map_err(|error| error.to_string())?;
        stmt.bind((5, receipt.message_id as i64))
            .map_err(|error| error.to_string())?;
        if let sqlite::State::Row = stmt.next().map_err(|error| error.to_string())? {
            Some(
                stmt.read::<String, _>(0)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        }
    };
    let asset_id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut stmt = conn
        .prepare(
            "INSERT INTO asset_locators(
                asset_id,owner_id,transport_mode,storage_peer_id,storage_peer_kind,message_id,
                legacy_folder_id,telegram_file_id,file_name,file_size,bot_pool_index,uploader_bot_id,
                locator_state,locator_version,created_at,updated_at
             ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?, 'ready',1,?,?)
             ON CONFLICT(owner_id,transport_mode,storage_peer_id,storage_peer_kind,message_id)
             DO UPDATE SET legacy_folder_id=excluded.legacy_folder_id,
                telegram_file_id=excluded.telegram_file_id,file_name=excluded.file_name,
                file_size=excluded.file_size,bot_pool_index=excluded.bot_pool_index,
                uploader_bot_id=excluded.uploader_bot_id,locator_state='ready',
                locator_version=asset_locators.locator_version+1,updated_at=excluded.updated_at",
        )
        .map_err(|error| error.to_string())?;
    stmt.bind((1, asset_id.as_str()))
        .map_err(|error| error.to_string())?;
    stmt.bind((2, owner_id))
        .map_err(|error| error.to_string())?;
    stmt.bind((3, transport_mode))
        .map_err(|error| error.to_string())?;
    stmt.bind((4, receipt.storage_peer_id))
        .map_err(|error| error.to_string())?;
    stmt.bind((5, receipt.storage_peer_kind.as_str()))
        .map_err(|error| error.to_string())?;
    stmt.bind((6, receipt.message_id as i64))
        .map_err(|error| error.to_string())?;
    stmt.bind((7, legacy_folder_id))
        .map_err(|error| error.to_string())?;
    stmt.bind((8, receipt.telegram_file_id.as_deref()))
        .map_err(|error| error.to_string())?;
    stmt.bind((9, receipt.file_name.as_str()))
        .map_err(|error| error.to_string())?;
    stmt.bind((10, receipt.file_size as i64))
        .map_err(|error| error.to_string())?;
    stmt.bind((11, receipt.bot_pool_index.map(i64::from)))
        .map_err(|error| error.to_string())?;
    stmt.bind((12, receipt.uploader_bot_id.as_deref()))
        .map_err(|error| error.to_string())?;
    stmt.bind((13, now)).map_err(|error| error.to_string())?;
    stmt.bind((14, now)).map_err(|error| error.to_string())?;
    stmt.next().map_err(|error| error.to_string())?;
    Ok(AssetLocatorRecord {
        asset_id,
        owner_id: owner_id.to_string(),
        transport_mode: transport_mode.to_string(),
        storage_peer_id: receipt.storage_peer_id,
        storage_peer_kind: receipt.storage_peer_kind.clone(),
        message_id: receipt.message_id,
        legacy_folder_id,
        telegram_file_id: receipt.telegram_file_id.clone(),
        file_name: receipt.file_name.clone(),
        file_size: receipt.file_size as i64,
        bot_pool_index: receipt.bot_pool_index,
        uploader_bot_id: receipt.uploader_bot_id.clone(),
    })
}

pub fn resolve(
    db: &DbConnection,
    message_id: i32,
    legacy_folder_id: Option<i64>,
    owner_id: Option<&str>,
) -> Result<LocatorResolution, String> {
    let conn = db.get().map_err(|error| error.to_string())?;
    let mut sql = String::from(
        "SELECT asset_id,owner_id,transport_mode,storage_peer_id,storage_peer_kind,message_id,
                legacy_folder_id,telegram_file_id,file_name,file_size,bot_pool_index,uploader_bot_id
         FROM asset_locators WHERE locator_state='ready' AND message_id=?",
    );
    if owner_id.is_some() {
        sql.push_str(" AND owner_id=?");
    }
    if legacy_folder_id.is_some() {
        sql.push_str(" AND legacy_folder_id=?");
    }
    sql.push_str(" ORDER BY updated_at DESC LIMIT 2");
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let mut index = 1;
    stmt.bind((index, message_id as i64))
        .map_err(|error| error.to_string())?;
    index += 1;
    if let Some(owner) = owner_id {
        stmt.bind((index, owner))
            .map_err(|error| error.to_string())?;
        index += 1;
    }
    if let Some(folder) = legacy_folder_id {
        stmt.bind((index, folder))
            .map_err(|error| error.to_string())?;
    }
    let mut rows = Vec::new();
    while let sqlite::State::Row = stmt.next().map_err(|error| error.to_string())? {
        rows.push(AssetLocatorRecord {
            asset_id: stmt
                .read::<String, _>("asset_id")
                .map_err(|error| error.to_string())?,
            owner_id: stmt
                .read::<String, _>("owner_id")
                .map_err(|error| error.to_string())?,
            transport_mode: stmt
                .read::<String, _>("transport_mode")
                .map_err(|error| error.to_string())?,
            storage_peer_id: stmt
                .read::<i64, _>("storage_peer_id")
                .map_err(|error| error.to_string())?,
            storage_peer_kind: stmt
                .read::<String, _>("storage_peer_kind")
                .map_err(|error| error.to_string())?,
            message_id: stmt
                .read::<i64, _>("message_id")
                .map_err(|error| error.to_string())? as i32,
            legacy_folder_id: stmt
                .read::<Option<i64>, _>("legacy_folder_id")
                .ok()
                .flatten(),
            telegram_file_id: stmt
                .read::<Option<String>, _>("telegram_file_id")
                .ok()
                .flatten(),
            file_name: stmt
                .read::<String, _>("file_name")
                .map_err(|error| error.to_string())?,
            file_size: stmt
                .read::<i64, _>("file_size")
                .map_err(|error| error.to_string())?,
            bot_pool_index: stmt
                .read::<Option<i64>, _>("bot_pool_index")
                .ok()
                .flatten()
                .map(|value| value as u32),
            uploader_bot_id: stmt
                .read::<Option<String>, _>("uploader_bot_id")
                .ok()
                .flatten(),
        });
    }
    Ok(match rows.len() {
        0 => LocatorResolution::NotFound,
        1 => LocatorResolution::Found(rows.remove(0)),
        count => LocatorResolution::Ambiguous { candidates: count },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> DbConnection {
        let dir = std::env::temp_dir().join(format!("td-locator-{}", uuid::Uuid::new_v4()));
        crate::db::init_db_at(&dir).expect("db")
    }

    fn receipt(peer: i64, message_id: i32) -> TelegramUploadReceipt {
        TelegramUploadReceipt {
            message_id,
            telegram_file_id: Some(format!("file-{peer}-{message_id}")),
            file_name: "a.bin".to_string(),
            file_size: 1,
            mime_type: "application/octet-stream".to_string(),
            storage_peer_id: peer,
            storage_peer_kind: "channel".to_string(),
            bot_pool_index: Some(0),
            uploader_bot_id: Some("bot-a".to_string()),
        }
    }

    #[test]
    fn same_message_id_in_two_peers_is_not_overwritten() {
        let db = temp_db();
        upsert_from_receipt(&db, &receipt(-1001, 42), "bot", Some(1), "tenant:a").unwrap();
        upsert_from_receipt(&db, &receipt(-1002, 42), "bot", Some(2), "tenant:a").unwrap();
        assert_eq!(
            resolve(&db, 42, None, Some("tenant:a")).unwrap(),
            LocatorResolution::Ambiguous { candidates: 2 }
        );
        let LocatorResolution::Found(found) = resolve(&db, 42, Some(2), Some("tenant:a")).unwrap()
        else {
            panic!("expected exact locator");
        };
        assert_eq!(found.storage_peer_id, -1002);
    }

    #[test]
    fn owner_scope_prevents_cross_tenant_resolution() {
        let db = temp_db();
        upsert_from_receipt(&db, &receipt(-1001, 9), "bot", None, "tenant:a").unwrap();
        assert_eq!(
            resolve(&db, 9, None, Some("tenant:b")).unwrap(),
            LocatorResolution::NotFound
        );
    }
}
