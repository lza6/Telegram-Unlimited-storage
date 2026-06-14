use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

pub fn mark_started() {
    let _ = START.set(Instant::now());
}

pub fn uptime_secs() -> u64 {
    START
        .get()
        .map(|s| s.elapsed().as_secs())
        .unwrap_or(0)
}
