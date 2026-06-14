//! Brute-force protection for web admin `ACCESS_PWD` verification.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct AccessLockout {
    failures: Mutex<HashMap<String, (u32, Instant)>>,
    max_failures: u32,
    lockout: Duration,
    /// Trusted proxy IPs — only these proxies' X-Forwarded-For headers are honored.
    /// Empty means trust no proxy (X-Forwarded-For is ignored).
    trusted_proxies: Vec<String>,
}

impl AccessLockout {
    pub fn new(max_failures: u32, lockout_secs: u64) -> Self {
        Self {
            failures: Mutex::new(HashMap::new()),
            max_failures: max_failures.max(3),
            lockout: Duration::from_secs(lockout_secs.max(30)),
            trusted_proxies: Self::load_trusted_proxies(),
        }
    }

    fn load_trusted_proxies() -> Vec<String> {
        std::env::var("TRUSTED_PROXIES")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn lockout_remaining_secs(&self, key: &str) -> Option<u64> {
        let map = self.failures.lock().ok()?;
        let (count, since) = map.get(key)?;
        if *count < self.max_failures {
            return None;
        }
        let elapsed = since.elapsed();
        if elapsed >= self.lockout {
            return None;
        }
        Some((self.lockout - elapsed).as_secs().max(1))
    }

    pub fn record_failure(&self, key: &str) {
        let Ok(mut map) = self.failures.lock() else {
            return;
        };
        let entry = map.entry(key.to_string()).or_insert((0, Instant::now()));
        if entry.0 >= self.max_failures && entry.1.elapsed() >= self.lockout {
            *entry = (1, Instant::now());
            return;
        }
        entry.0 = entry.0.saturating_add(1);
        if entry.0 == 1 {
            entry.1 = Instant::now();
        }
        if entry.0 >= self.max_failures {
            entry.1 = Instant::now();
        }
    }

    pub fn clear(&self, key: &str) {
        if let Ok(mut map) = self.failures.lock() {
            map.remove(key);
        }
    }

    pub fn client_key(
        &self,
        connection_host: &str,
        real_ip: Option<&str>,
        forwarded_for: Option<&str>,
    ) -> String {
        if let Some(ip) = real_ip.filter(|s| !s.is_empty()) {
            return ip.to_string();
        }
        if let Some(ff) = forwarded_for {
            if !self.trusted_proxies.is_empty() {
                if let Some(first) = ff.split(',').next() {
                    let t = first.trim();
                    if !t.is_empty() {
                        return t.to_string();
                    }
                }
            }
        }
        connection_host.to_string()
    }
}

/// Standalone function for backward compatibility (ignores trusted_proxies).
/// Prefer AccessLockout::client_key() for production use.
pub fn client_key_from_request(
    connection_host: &str,
    real_ip: Option<&str>,
    forwarded_for: Option<&str>,
) -> String {
    if let Some(ip) = real_ip.filter(|s| !s.is_empty()) {
        return ip.to_string();
    }
    if let Some(ff) = forwarded_for {
        if let Some(first) = ff.split(',').next() {
            let t = first.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    connection_host.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_max_failures() {
        let lock = AccessLockout::new(3, 60);
        let k = "1.2.3.4";
        lock.record_failure(k);
        lock.record_failure(k);
        assert!(lock.lockout_remaining_secs(k).is_none());
        lock.record_failure(k);
        assert!(lock.lockout_remaining_secs(k).is_some());
        lock.clear(k);
        assert!(lock.lockout_remaining_secs(k).is_none());
    }
}
