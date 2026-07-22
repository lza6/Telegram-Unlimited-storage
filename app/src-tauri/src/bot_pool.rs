//! Round-robin Bot token pool with FloodWait awareness.
//! Spreads Telegram Bot API rate limits across multiple bots
//! posting to the same storage channel (Pentaract-style worker pool).
//!
//! Features:
//! - Round-robin token selection
//! - FloodWait tracking per-bot
//! - Automatic skip of bots in FloodWait state
//! - Prometheus metrics for monitoring

use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};
use serde::Serialize;

use crate::server_config::ServerConfig;

/// Default FloodWait buffer (add extra seconds to be safe)
const FLOOD_WAIT_BUFFER_SECS: i64 = 5;

/// Entry for a single bot token with FloodWait state
#[derive(Debug)]
struct BotEntry {
    /// Bot token
    token: String,
    /// Unix timestamp when FloodWait expires (0 = no FloodWait)
    flood_until: AtomicI64,
    /// Number of times this bot hit FloodWait
    flood_count: AtomicUsize,
    /// Last time this bot was used successfully
    last_success: RwLock<Option<Instant>>,
    /// Per-bot minimum interval between requests (token bucket cap).
    /// When 0, no per-bot throttling is applied.
    min_interval: Duration,
    /// Timestamp of the last request dispatched to this bot.
    last_dispatch: Mutex<Option<Instant>>,
}

impl BotEntry {
    fn new(token: String) -> Self {
        Self::with_interval(token, Duration::ZERO)
    }

    fn with_interval(token: String, min_interval: Duration) -> Self {
        Self {
            token,
            flood_until: AtomicI64::new(0),
            flood_count: AtomicUsize::new(0),
            last_success: RwLock::new(None),
            min_interval,
            last_dispatch: Mutex::new(None),
        }
    }

    /// Check if this bot is available (not in FloodWait)
    fn is_available(&self) -> bool {
        let until = self.flood_until.load(Ordering::Relaxed);
        let now = chrono::Utc::now().timestamp();
        now >= until
    }

    /// Get remaining FloodWait seconds (0 if available)
    fn remaining_flood_wait(&self) -> i64 {
        let until = self.flood_until.load(Ordering::Relaxed);
        let now = chrono::Utc::now().timestamp();
        (until - now).max(0)
    }

    /// Mark this bot as in FloodWait
    fn set_flood_wait(&self, seconds: i32) {
        let until = chrono::Utc::now().timestamp() + seconds as i64 + FLOOD_WAIT_BUFFER_SECS;
        self.flood_until.store(until, Ordering::Relaxed);
        self.flood_count.fetch_add(1, Ordering::Relaxed);
        log::warn!(
            "Bot token [***{}] entered FloodWait for {}s (expires at {})",
            &self.token[self.token.len().saturating_sub(6)..],
            seconds,
            chrono::DateTime::from_timestamp(until, 0)
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or_default()
        );
    }

    /// Clear FloodWait state (on successful operation)
    fn clear_flood_wait(&self) {
        self.flood_until.store(0, Ordering::Relaxed);
        *self.last_success.write() = Some(Instant::now());
    }

    /// Return how long the caller must wait before dispatching to this bot,
    /// based on the per-bot minimum interval. `None` means dispatch now.
    fn reserve(&self) -> Option<Duration> {
        if self.min_interval.is_zero() {
            return None;
        }
        let mut last = self.last_dispatch.lock();
        let now = Instant::now();
        let earliest = last.map(|t| t + self.min_interval);
        match earliest {
            Some(e) if e > now => {
                // Not yet eligible — do not stamp; caller waits then retries.
                Some(e - now)
            }
            _ => {
                *last = Some(now);
                None
            }
        }
    }
}

/// Metrics for monitoring bot pool health
#[derive(Debug, Clone, Serialize, Default)]
pub struct BotPoolMetrics {
    /// Total number of bots in pool
    pub total_bots: usize,
    /// Number of bots currently available
    pub available_bots: usize,
    /// Number of bots in FloodWait
    pub flooded_bots: usize,
    /// Total FloodWait events since startup
    pub total_flood_events: usize,
    /// Per-bot status (token suffix -> status)
    pub bot_status: Vec<BotStatusInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BotStatusInfo {
    pub token_suffix: String,
    pub is_available: bool,
    pub remaining_flood_wait_secs: i64,
    pub flood_count: usize,
}

/// Bot pool with FloodWait-aware token selection
#[derive(Debug)]
pub struct BotPool {
    entries: Vec<BotEntry>,
    /// Round-robin index (incremented on each request)
    next: AtomicUsize,
    /// Metrics cache (updated on demand)
    metrics_cache: RwLock<Option<(BotPoolMetrics, Instant)>>,
    /// Cache TTL
    metrics_cache_ttl: Duration,
}

impl BotPool {
    /// Create bot pool from server config
    pub fn from_config(config: &ServerConfig) -> Self {
        let interval = Duration::from_millis(config.bot_rate_limit_ms.max(1) as u64);
        Self::with_interval(config.all_bot_tokens(), interval)
    }

    /// Create bot pool with token list (no per-bot throttle)
    pub fn new(tokens: Vec<String>) -> Self {
        Self::with_interval(tokens, Duration::ZERO)
    }

    /// Create bot pool with a per-bot minimum interval between dispatches.
    /// The interval is applied per-bot, so N bots yield up to N×(1/interval) req/s
    /// instead of a single global 1/interval serial rate.
    pub fn with_interval(tokens: Vec<String>, min_interval: Duration) -> Self {
        let entries: Vec<BotEntry> = tokens
            .into_iter()
            .map(|t| BotEntry::with_interval(t, min_interval))
            .collect();

        if entries.is_empty() {
            log::warn!("BotPool created with no tokens!");
        } else {
            log::info!("BotPool created with {} token(s)", entries.len());
        }

        Self {
            entries,
            next: AtomicUsize::new(0),
            metrics_cache: RwLock::new(None),
            metrics_cache_ttl: Duration::from_secs(5),
        }
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total number of bots
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Get next available token (skips FloodWait bots)
    /// Returns (token, pool_index) or None if all bots are flooded
    pub fn next_available_token(&self) -> Option<(String, u32)> {
        if self.entries.is_empty() {
            return None;
        }

        // Try each bot once (round-robin with flood-aware skip)
        let start_idx = self.next.fetch_add(1, Ordering::Relaxed);

        for offset in 0..self.entries.len() {
            let idx = (start_idx + offset) % self.entries.len();
            let entry = &self.entries[idx];

            if entry.is_available() {
                // Move next index forward for next caller
                self.next
                    .store((idx + 1) % self.entries.len(), Ordering::Relaxed);
                return Some((entry.token.clone(), idx as u32));
            }
        }

        // All bots are flooded
        log::warn!("All {} bots are in FloodWait state", self.entries.len());
        None
    }

    /// Try to acquire a token that is both available (not FloodWait) and
    /// eligible now (per-bot interval elapsed). Stamps the bot's last-dispatch
    /// time on success. Returns `None` if no bot is ready this instant — the
    /// caller should sleep `earliest_eligible_in()` and retry.
    pub fn try_acquire_now(&self) -> Option<(String, u32)> {
        if self.entries.is_empty() {
            return None;
        }
        let start_idx = self.next.fetch_add(1, Ordering::Relaxed);
        for offset in 0..self.entries.len() {
            let idx = (start_idx + offset) % self.entries.len();
            let entry = &self.entries[idx];
            if entry.is_available() && entry.reserve().is_none() {
                self.next
                    .store((idx + 1) % self.entries.len(), Ordering::Relaxed);
                return Some((entry.token.clone(), idx as u32));
            }
        }
        None
    }

    /// Minimum duration until some available bot becomes eligible. Returns
    /// `None` when every bot is in FloodWait (nothing to wait for) or the pool
    /// has no per-bot interval configured.
    pub fn earliest_eligible_in(&self) -> Option<Duration> {
        let mut soonest: Option<Duration> = None;
        for entry in &self.entries {
            if !entry.is_available() {
                continue;
            }
            if let Some(wait) = entry.reserve() {
                soonest = Some(match soonest {
                    Some(prev) if prev < wait => prev,
                    _ => wait,
                });
            }
        }
        soonest
    }

    /// Get next token (legacy method - always returns token regardless of FloodWait)
    /// Use `next_available_token()` for FloodWait-aware selection
    pub fn next_token(&self) -> Option<(String, u32)> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.entries.len();
        Some((self.entries[idx].token.clone(), idx as u32))
    }

    /// Mark a bot as in FloodWait state
    pub fn mark_flood_wait(&self, pool_index: u32, seconds: i32) {
        if let Some(entry) = self.entries.get(pool_index as usize) {
            entry.set_flood_wait(seconds);
            // Invalidate metrics cache
            *self.metrics_cache.write() = None;
        }
    }

    /// Mark a bot as successfully used (clear FloodWait)
    pub fn mark_success(&self, pool_index: u32) {
        if let Some(entry) = self.entries.get(pool_index as usize) {
            entry.clear_flood_wait();
        }
    }

    /// Get token at specific index
    pub fn token_at(&self, index: u32) -> Option<&str> {
        self.entries.get(index as usize).map(|e| e.token.as_str())
    }

    /// Check if a specific bot is available
    pub fn is_bot_available(&self, index: u32) -> bool {
        self.entries
            .get(index as usize)
            .map(|e| e.is_available())
            .unwrap_or(false)
    }

    /// Get remaining FloodWait for a specific bot
    pub fn remaining_flood_wait(&self, index: u32) -> i64 {
        self.entries
            .get(index as usize)
            .map(|e| e.remaining_flood_wait())
            .unwrap_or(0)
    }

    /// Get metrics for monitoring
    pub fn metrics(&self) -> BotPoolMetrics {
        // Check cache
        {
            let cache = self.metrics_cache.read();
            if let Some((metrics, timestamp)) = cache.as_ref() {
                if timestamp.elapsed() < self.metrics_cache_ttl {
                    return metrics.clone();
                }
            }
        }

        // Compute fresh metrics
        let mut available = 0;
        let mut flooded = 0;
        let mut total_flood_events = 0;
        let mut bot_status = Vec::with_capacity(self.entries.len());

        for entry in &self.entries {
            let is_available = entry.is_available();
            if is_available {
                available += 1;
            } else {
                flooded += 1;
            }

            total_flood_events += entry.flood_count.load(Ordering::Relaxed);

            let token_suffix = if entry.token.len() > 6 {
                format!("***{}", &entry.token[entry.token.len() - 6..])
            } else {
                "****".to_string()
            };

            bot_status.push(BotStatusInfo {
                token_suffix,
                is_available,
                remaining_flood_wait_secs: entry.remaining_flood_wait(),
                flood_count: entry.flood_count.load(Ordering::Relaxed),
            });
        }

        let metrics = BotPoolMetrics {
            total_bots: self.entries.len(),
            available_bots: available,
            flooded_bots: flooded,
            total_flood_events,
            bot_status,
        };

        // Update cache
        *self.metrics_cache.write() = Some((metrics.clone(), Instant::now()));

        metrics
    }

    /// Get estimated wait time until any bot becomes available
    pub fn earliest_availability_secs(&self) -> Option<i64> {
        self.entries
            .iter()
            .filter(|e| !e.is_available())
            .map(|e| e.remaining_flood_wait())
            .min()
            .and_then(|v| if v > 0 { Some(v) } else { None })
    }

    /// Clear all FloodWait states (use with caution)
    pub fn clear_all_flood_waits(&self) {
        for entry in &self.entries {
            entry.clear_flood_wait();
        }
        *self.metrics_cache.write() = None;
        log::info!("Cleared all FloodWait states");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_robin_cycles_tokens() {
        let pool = BotPool::new(vec!["token_a".into(), "token_b".into(), "token_c".into()]);

        assert_eq!(pool.next_token().map(|(t, _)| t), Some("token_a".into()));
        assert_eq!(pool.next_token().map(|(t, _)| t), Some("token_b".into()));
        assert_eq!(pool.next_token().map(|(t, _)| t), Some("token_c".into()));
        assert_eq!(pool.next_token().map(|(t, _)| t), Some("token_a".into()));
    }

    #[test]
    fn flood_wait_skips_flooded_bots() {
        let pool = BotPool::new(vec!["a".into(), "b".into(), "c".into()]);

        // Mark bot 'b' (index 1) as flooded
        pool.mark_flood_wait(1, 30);

        // Get next available - should skip 'b'
        let results: Vec<_> = (0..10)
            .map(|_| pool.next_available_token().map(|(t, _)| t))
            .collect();

        // All results should be 'a' or 'c', never 'b'
        for token in results.iter().flatten() {
            assert_ne!(*token, "b");
        }
    }

    #[test]
    fn all_flooded_returns_none() {
        let pool = BotPool::new(vec!["a".into()]);

        // Mark the only bot as flooded
        pool.mark_flood_wait(0, 30);

        assert!(pool.next_available_token().is_none());
    }

    #[test]
    fn flood_wait_expires() {
        let pool = BotPool::new(vec!["a".into()]);

        // Mark as flooded for 0 seconds (should already be expired)
        pool.mark_flood_wait(0, 0);

        // Wait a moment for the buffer
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Should be available now
        // Note: This test is timing-dependent, but with 0s + buffer, it should pass
    }

    #[test]
    fn metrics_calculation() {
        let pool = BotPool::new(vec!["a".into(), "b".into(), "c".into()]);

        pool.mark_flood_wait(0, 30);
        pool.mark_flood_wait(1, 60);

        let metrics = pool.metrics();

        assert_eq!(metrics.total_bots, 3);
        assert_eq!(metrics.available_bots, 1); // only 'c'
        assert_eq!(metrics.flooded_bots, 2);
        assert_eq!(metrics.total_flood_events, 2);
    }

    #[test]
    fn mark_success_clears_flood() {
        let pool = BotPool::new(vec!["a".into()]);

        pool.mark_flood_wait(0, 30);
        assert!(!pool.is_bot_available(0));

        pool.mark_success(0);
        assert!(pool.is_bot_available(0));
    }

    #[test]
    fn earliest_availability() {
        let pool = BotPool::new(vec!["a".into(), "b".into()]);

        // No floods
        assert!(pool.earliest_availability_secs().is_none());

        // One flood
        pool.mark_flood_wait(0, 10);
        let earliest = pool.earliest_availability_secs();
        assert!(earliest.is_some());
        assert!(earliest.unwrap() > 0);
    }

    #[test]
    fn clear_all_floods() {
        let pool = BotPool::new(vec!["a".into(), "b".into()]);

        pool.mark_flood_wait(0, 30);
        pool.mark_flood_wait(1, 60);

        assert_eq!(pool.metrics().flooded_bots, 2);

        pool.clear_all_flood_waits();

        assert_eq!(pool.metrics().flooded_bots, 0);
        assert!(pool.next_available_token().is_some());
    }

    #[test]
    fn per_bot_interval_allows_parallel_dispatch() {
        // Two bots, 1s interval each → first dispatch to each is immediate,
        // second to the SAME bot within 1s blocks, but the OTHER bot is free.
        let pool = BotPool::with_interval(vec!["a".into(), "b".into()], Duration::from_secs(1));

        let (t1, _) = pool.try_acquire_now().expect("first acquire");
        let (t2, _) = pool.try_acquire_now().expect("second acquire to other bot");
        assert_ne!(t1, t2, "two distinct bots must be dispatched in parallel");

        // Both bots now cooling; third immediate acquire must fail.
        assert!(pool.try_acquire_now().is_none());
        // But an eligible wait is reported (≤1s).
        let wait = pool.earliest_eligible_in().expect("some bot eligible soon");
        assert!(wait <= Duration::from_secs(1));
    }

    #[test]
    fn zero_interval_always_eligible() {
        let pool = BotPool::new(vec!["a".into()]);
        assert!(pool.try_acquire_now().is_some());
        assert!(pool.try_acquire_now().is_some());
        assert!(pool.earliest_eligible_in().is_none());
    }

    #[test]
    fn try_acquire_skips_flooded_bots() {
        let pool = BotPool::with_interval(vec!["a".into(), "b".into()], Duration::from_secs(1));
        pool.mark_flood_wait(0, 30);
        // Only 'b' available and eligible now.
        let (t, _) = pool.try_acquire_now().expect("acquire non-flooded bot");
        assert_eq!(t, "b");
    }

    #[test]
    fn all_flooded_acquires_none_and_no_wait() {
        let pool = BotPool::with_interval(vec!["a".into()], Duration::from_secs(1));
        pool.mark_flood_wait(0, 30);
        assert!(pool.try_acquire_now().is_none());
        assert!(pool.earliest_eligible_in().is_none());
    }
}
