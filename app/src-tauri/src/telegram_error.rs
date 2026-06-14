//! Classify Telegram/storage errors for retry vs fail-fast (K-Vault pattern).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelegramErrorClass {
    /// Transient network / timeout — retry with backoff.
    Retryable,
    /// FloodWait / rate limit — sleep then retry (does not consume retry budget when handled upstream).
    RateLimited,
    /// Auth, config, or client missing — do not retry.
    Fatal,
}

pub fn classify_telegram_error_message(msg: &str) -> TelegramErrorClass {
    let lower = msg.to_ascii_lowercase();
    if lower.contains("flood")
        || lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("429")
    {
        return TelegramErrorClass::RateLimited;
    }
    if lower.contains("not connected")
        || lower.contains("unauthorized")
        || lower.contains("invalid token")
        || lower.contains("auth")
        || lower.contains("forbidden")
        || lower.contains("chat not found")
        || lower.contains("bot was blocked")
    {
        return TelegramErrorClass::Fatal;
    }
    if lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("network")
        || lower.contains("temporarily unavailable")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
    {
        return TelegramErrorClass::Retryable;
    }
    TelegramErrorClass::Retryable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_flood_as_rate_limited() {
        assert_eq!(
            classify_telegram_error_message("FLOOD_WAIT_30"),
            TelegramErrorClass::RateLimited
        );
    }

    #[test]
    fn classifies_disconnect_as_fatal() {
        assert_eq!(
            classify_telegram_error_message("Telegram client is not connected"),
            TelegramErrorClass::Fatal
        );
    }

    #[test]
    fn classifies_timeout_as_retryable() {
        assert_eq!(
            classify_telegram_error_message("connection timeout"),
            TelegramErrorClass::Retryable
        );
    }
}
