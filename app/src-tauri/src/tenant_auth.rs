//! Multi-tenant API identity and file ownership.

use actix_web::HttpRequest;
use serde::Deserialize;
use std::path::Path;

use crate::admin_routes::{check_access_pwd, check_pwd_form};
use crate::commands::api_settings;
use crate::db::DbConnection;
use crate::server_config::ServerConfig;

pub const OWNER_WEB: &str = "system:web";
pub const OWNER_ADMIN: &str = "system:admin";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallerIdentity {
    /// Web 管理台密码 / X-Access-Pwd
    Admin,
    /// API Key 对应租户
    Tenant { tenant_id: String },
    Anonymous,
}

impl CallerIdentity {
    pub fn owner_id_for_asset(&self) -> String {
        match self {
            CallerIdentity::Admin => OWNER_WEB.to_string(),
            CallerIdentity::Tenant { tenant_id } => format!("tenant:{tenant_id}"),
            CallerIdentity::Anonymous => String::new(),
        }
    }

    pub fn can_access_owner(&self, asset_owner: &str) -> bool {
        match self {
            CallerIdentity::Admin => true,
            CallerIdentity::Tenant { tenant_id } => {
                asset_owner == format!("tenant:{tenant_id}")
            }
            CallerIdentity::Anonymous => false,
        }
    }
}

#[derive(Debug, Deserialize)]
struct TenantFileEntry {
    tenant_id: String,
    api_key: String,
    #[serde(default)]
    display_name: Option<String>,
}

/// Ensure at least one tenant exists (from API_KEY + optional tenants.json).
pub fn bootstrap_tenants(db: &DbConnection, config: &ServerConfig) -> Result<(), String> {
    let count = crate::db::count_tenants(db)?;
    if count > 0 {
        return Ok(());
    }
    if let Some(ref key) = config.api_key {
        let hash = api_settings::hash_key_public(key);
        crate::db::upsert_tenant(db, "default", hash, Some("Default API tenant"))?;
    }
    let path = config.data_dir.join("tenants.json");
    if path.is_file() {
        load_tenants_file(db, &path)?;
    }
    Ok(())
}

fn load_tenants_file(db: &DbConnection, path: &Path) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let entries: Vec<TenantFileEntry> =
        serde_json::from_str(&raw).map_err(|e| format!("tenants.json invalid: {e}"))?;
    for entry in entries {
        if entry.tenant_id.is_empty() || entry.api_key.is_empty() {
            continue;
        }
        let hash = api_settings::hash_key_public(&entry.api_key);
        crate::db::upsert_tenant(
            db,
            &entry.tenant_id,
            hash,
            entry.display_name.as_deref(),
        )?;
    }
    Ok(())
}

pub fn resolve_caller(
    req: &HttpRequest,
    config: &ServerConfig,
    db: &DbConnection,
) -> CallerIdentity {
    if check_access_pwd(req, config) {
        return CallerIdentity::Admin;
    }
    if let Some(tenant_id) = api_key_tenant(req, db, config) {
        return CallerIdentity::Tenant { tenant_id };
    }
    CallerIdentity::Anonymous
}

pub fn api_key_tenant(
    req: &HttpRequest,
    db: &DbConnection,
    config: &ServerConfig,
) -> Option<String> {
    let key = req.headers().get("X-API-Key")?.to_str().ok()?;
    resolve_tenant_from_api_key(db, config, key)
}

pub fn resolve_tenant_from_api_key(
    db: &DbConnection,
    config: &ServerConfig,
    key: &str,
) -> Option<String> {
    if config.multi_tenant_enabled {
        crate::db::find_tenant_id_by_api_key(db, key).ok().flatten()
    } else if let Some(ref hash) = config.api_key_hash {
        if api_settings::verify_key(key, hash) {
            Some("default".to_string())
        } else {
            None
        }
    } else {
        None
    }
}

pub fn verify_api_key_header(req: &HttpRequest, config: &ServerConfig, db: &DbConnection) -> bool {
    api_key_tenant(req, db, config).is_some()
}

/// Legacy form pwd uploads map to web owner scope.
pub fn caller_from_web_pwd(pwd_ok: bool, config: &ServerConfig) -> CallerIdentity {
    if pwd_ok || config.access_pwd.is_empty() {
        CallerIdentity::Admin
    } else {
        CallerIdentity::Anonymous
    }
}

pub fn check_pwd_caller(pwd: &str, config: &ServerConfig) -> CallerIdentity {
    if check_pwd_form(pwd, config) {
        CallerIdentity::Admin
    } else {
        CallerIdentity::Anonymous
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    fn test_db() -> DbConnection {
        let dir = std::env::temp_dir().join(format!("td-tenant-{}", uuid::Uuid::new_v4()));
        crate::db::init_db_at(&dir).expect("init db")
    }

    #[test]
    fn resolve_tenant_from_api_key_single_tenant() {
        let config = crate::server_config::test_config();
        let db = test_db();
        bootstrap_tenants(&db, &config).unwrap();
        let id = resolve_tenant_from_api_key(&db, &config, "test-api-key");
        assert_eq!(id.as_deref(), Some("default"));
    }

    #[test]
    fn caller_from_web_pwd_maps_to_admin() {
        let config = crate::server_config::test_config();
        assert_eq!(
            caller_from_web_pwd(true, &config),
            CallerIdentity::Admin
        );
    }

    #[test]
    fn resolve_caller_admin_via_header() {
        let config = crate::server_config::test_config();
        let db = test_db();
        let req = TestRequest::default()
            .insert_header(("X-Access-Pwd", config.access_pwd.as_str()))
            .to_http_request();
        assert_eq!(resolve_caller(&req, &config, &db), CallerIdentity::Admin);
    }
}
