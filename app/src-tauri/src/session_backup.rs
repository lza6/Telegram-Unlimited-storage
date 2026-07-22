//! Telegram session backup and recovery — protects against session file corruption.
//!
//! ## Why this exists
//!
//! The `telegram.session` SQLite database is critical for User-mode connections.
//! It can be corrupted by:
//! - Abrupt shutdown during WAL checkpoint
//! - Disk full during write
//! - Concurrent access conflicts
//!
//! ## Strategy
//!
//! ```text
//! telegram.session  ──►  telegram.session.backup (periodic)
//!                              │
//!                    On corruption detected ──► restore from backup
//! ```
//!
//! Backups are taken:
//! - After successful login (auth sign-in / QR poll success)
//! - Periodically (every 6 hours) if session has been active

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Backup file suffix
const BACKUP_SUFFIX: &str = ".backup";
/// Interval between periodic backups
const BACKUP_INTERVAL_SECS: u64 = 21600; // 6 hours

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Tracks whether we have a valid backup
static HAS_BACKUP: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Data directory with session info
#[derive(Clone)]
pub struct SessionBackup {
    data_dir: PathBuf,
}

impl SessionBackup {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub fn session_path(&self) -> PathBuf {
        self.data_dir.join("telegram.session")
    }

    pub fn backup_path(&self) -> PathBuf {
        self.data_dir
            .join(format!("telegram.session{}", BACKUP_SUFFIX))
    }

    /// Create a backup of the current session file.
    /// Returns `true` if backup was created successfully.
    pub fn backup(&self) -> bool {
        let src = self.session_path();
        let dst = self.backup_path();

        if !src.exists() {
            return false;
        }

        // Read current session, write to backup
        match std::fs::read(&src) {
            Ok(data) => {
                if data.is_empty() {
                    log::warn!("Session file is empty, skipping backup");
                    return false;
                }
                match std::fs::write(&dst, &data) {
                    Ok(_) => {
                        HAS_BACKUP.store(true, Ordering::SeqCst);
                        log::info!("Session backed up ({} bytes)", data.len());
                        true
                    }
                    Err(e) => {
                        log::warn!("Failed to write session backup: {e}");
                        false
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to read session for backup: {e}");
                false
            }
        }
    }

    /// Check if session file is valid (non-empty SQLite).
    pub fn session_is_valid(&self) -> bool {
        let path = self.session_path();
        if !path.exists() {
            return false;
        }
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if meta.len() < 100 {
            // SQLite header is 100 bytes; anything smaller is corrupt
            return false;
        }
        // Quick check: first 16 bytes should be SQLite format header
        let mut header = [0u8; 16];
        if std::fs::File::open(&path)
            .and_then(|f| {
                use std::io::Read;
                (&f).read_exact(&mut header)
            })
            .is_err()
        {
            return false;
        }
        // SQLite header: "SQLite format 3\0"
        &header[0..16] == b"SQLite format 3\0"
    }

    /// Check if backup exists and is usable
    pub fn backup_exists(&self) -> bool {
        let path = self.backup_path();
        if !path.exists() {
            return false;
        }
        match std::fs::metadata(&path) {
            Ok(m) => m.len() >= 100,
            Err(_) => false,
        }
    }

    /// Restore session from backup.
    /// Returns `true` if restoration succeeded.
    pub fn restore_from_backup(&self) -> bool {
        let src = self.backup_path();
        let dst = self.session_path();

        if !self.backup_exists() {
            log::warn!("No valid backup to restore from");
            return false;
        }

        let data = match std::fs::read(&src) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to read backup: {e}");
                return false;
            }
        };

        match std::fs::write(&dst, &data) {
            Ok(_) => {
                // Also clean up WAL/SHM files since they refer to old session state
                let _ = std::fs::remove_file(dst.with_extension("session-wal"));
                let _ = std::fs::remove_file(dst.with_extension("session-shm"));
                log::info!("Session restored from backup ({} bytes)", data.len());
                true
            }
            Err(e) => {
                log::warn!("Failed to restore session from backup: {e}");
                false
            }
        }
    }

    /// Ensure a valid session exists — restore from backup if current is corrupt.
    /// Returns the session path to use.
    pub fn ensure_valid_session(&self) -> PathBuf {
        if self.session_is_valid() {
            return self.session_path();
        }

        // Session is missing or corrupt — try to restore from backup
        log::warn!("Session file is missing or corrupt, attempting backup restore...");
        if self.restore_from_backup() {
            log::info!("Session restored successfully from backup");
        } else {
            log::info!("No backup available — will create new session on login");
        }

        self.session_path()
    }
}

/// Create a periodic backup task that runs in the background.
/// Spawns a tokio task that backs up the session every `BACKUP_INTERVAL_SECS`.
pub fn spawn_periodic_backup(data_dir: PathBuf, running: Arc<AtomicBool>) {
    tokio::spawn(async move {
        // Wait a bit before first backup to let session initialize
        tokio::time::sleep(Duration::from_secs(300)).await; // 5 min

        loop {
            if !running.load(Ordering::SeqCst) {
                break;
            }

            let backup = SessionBackup::new(data_dir.clone());
            backup.backup();

            // Sleep for interval
            for _ in 0..(BACKUP_INTERVAL_SECS / 10) {
                tokio::time::sleep(Duration::from_secs(10)).await;
                if !running.load(Ordering::SeqCst) {
                    return;
                }
            }
        }
    });
}

/// Check if a backup is available
pub fn has_backup() -> bool {
    HAS_BACKUP.load(Ordering::SeqCst) || {
        // Also check env vars that indicate headless server
        std::env::var("SESSION_BACKUP_DIR").is_ok()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_sqlite_header() -> Vec<u8> {
        let mut buf = vec![0u8; 256];
        buf[0..16].copy_from_slice(b"SQLite format 3\0");
        buf
    }

    #[test]
    fn detects_valid_session() {
        let dir = tempfile::tempdir().unwrap();
        let backup = SessionBackup::new(dir.path().to_path_buf());
        let path = backup.session_path();

        // No file exists
        assert!(!backup.session_is_valid());

        // Empty file
        std::fs::write(&path, &[]).unwrap();
        assert!(!backup.session_is_valid());

        // Valid SQLite header
        std::fs::write(&path, valid_sqlite_header()).unwrap();
        assert!(backup.session_is_valid());
    }

    #[test]
    fn backup_and_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let backup = SessionBackup::new(dir.path().to_path_buf());

        // Write a fake valid session
        let data = valid_sqlite_header();
        std::fs::write(backup.session_path(), &data).unwrap();

        // Backup
        assert!(backup.backup());
        assert!(backup.backup_path().exists());

        // "Corrupt" the session
        std::fs::write(backup.session_path(), b"garbage data").unwrap();
        assert!(!backup.session_is_valid());

        // Restore
        assert!(backup.restore_from_backup());
        assert!(backup.session_is_valid());
    }

    #[test]
    fn backup_fails_on_missing_session() {
        let dir = tempfile::tempdir().unwrap();
        let backup = SessionBackup::new(dir.path().to_path_buf());
        assert!(!backup.backup());
    }

    #[test]
    fn restore_fails_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let backup = SessionBackup::new(dir.path().to_path_buf());
        assert!(!backup.restore_from_backup());
    }
}
