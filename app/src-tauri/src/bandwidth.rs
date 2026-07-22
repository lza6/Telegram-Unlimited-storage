use chrono::Local;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BandwidthStats {
    pub date: String,
    pub up_bytes: u64,
    pub down_bytes: u64,
}

impl Default for BandwidthStats {
    fn default() -> Self {
        Self {
            date: Local::now().format("%Y-%m-%d").to_string(),
            up_bytes: 0,
            down_bytes: 0,
        }
    }
}

pub struct BandwidthManager {
    pub file_path: PathBuf,
    pub stats: tokio::sync::Mutex<BandwidthStats>,
    pub limit: u64, // Daily limit in bytes (0 = unlimited)
}

impl BandwidthManager {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        // Resolve app data directory
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| PathBuf::from("data"));
        if !app_data_dir.exists() {
            let _ = std::fs::create_dir_all(&app_data_dir);
        }
        let file_path = app_data_dir.join("bandwidth.json");

        let stats = if file_path.exists() {
            let content = std::fs::read_to_string(&file_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            BandwidthStats::default()
        };

        // Read limit from env (GB), default 250GB, 0 = unlimited
        let limit_gb: u64 = std::env::var("BANDWIDTH_LIMIT_GB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(250);
        let limit = if limit_gb == 0 {
            0
        } else {
            limit_gb * 1024 * 1024 * 1024
        };

        Self {
            file_path,
            stats: tokio::sync::Mutex::new(stats),
            limit,
        }
    }

    async fn check_and_reset(&self) {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let mut stats = self.stats.lock().await;
        if stats.date != today {
            log::info!(
                "[Bandwidth] New day detected. Resetting stats. Old date: {}, New date: {}",
                stats.date,
                today
            );
            stats.date = today;
            stats.up_bytes = 0;
            stats.down_bytes = 0;
            self.save_locked(&stats);
        }
    }

    pub async fn can_transfer(&self, bytes: u64) -> Result<(), String> {
        if self.limit == 0 {
            return Ok(());
        }
        self.check_and_reset().await;
        let stats = self.stats.lock().await;
        let total = stats.up_bytes + stats.down_bytes + bytes;
        if total > self.limit {
            return Err(format!(
                "Daily bandwidth limit ({}) exceeded! Used: {}",
                Self::format_bytes(self.limit),
                Self::format_bytes(total)
            ));
        }
        Ok(())
    }

    pub async fn add_up(&self, bytes: u64) {
        self.check_and_reset().await;
        let mut stats = self.stats.lock().await;
        stats.up_bytes += bytes;
        self.save_locked(&stats);
    }

    pub async fn add_down(&self, bytes: u64) {
        self.check_and_reset().await;
        let mut stats = self.stats.lock().await;
        stats.down_bytes += bytes;
        self.save_locked(&stats);
    }

    fn save_locked(&self, stats: &BandwidthStats) {
        if let Ok(json) = serde_json::to_string(stats) {
            let _ = std::fs::write(&self.file_path, json);
        }
    }

    pub async fn get_stats(&self) -> BandwidthStats {
        self.check_and_reset().await;
        self.stats.lock().await.clone()
    }

    fn format_bytes(bytes: u64) -> String {
        const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
        let mut v = bytes as f64;
        let mut i = 0;
        while v >= 1024.0 && i < UNITS.len() - 1 {
            v /= 1024.0;
            i += 1;
        }
        format!("{:.2} {}", v, UNITS[i])
    }

    /// Test helper — isolated stats file and explicit byte limit.
    pub fn new_at_path(file_path: PathBuf, limit_bytes: u64) -> Self {
        Self {
            file_path,
            stats: tokio::sync::Mutex::new(BandwidthStats::default()),
            limit: limit_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn allows_transfer_within_limit() {
        let path = std::env::temp_dir().join(format!("td-bw-{}", uuid::Uuid::new_v4()));
        let mgr = BandwidthManager::new_at_path(path, 1000);
        assert!(mgr.can_transfer(500).await.is_ok());
    }

    #[tokio::test]
    async fn rejects_transfer_over_limit() {
        let path = std::env::temp_dir().join(format!("td-bw-{}", uuid::Uuid::new_v4()));
        let mgr = BandwidthManager::new_at_path(path, 100);
        mgr.add_up(80).await;
        assert!(mgr.can_transfer(30).await.is_err());
    }

    #[tokio::test]
    async fn unlimited_when_limit_zero() {
        let path = std::env::temp_dir().join(format!("td-bw-{}", uuid::Uuid::new_v4()));
        let mgr = BandwidthManager::new_at_path(path, 0);
        mgr.add_up(1_000_000).await;
        assert!(mgr.can_transfer(1_000_000).await.is_ok());
    }
}
