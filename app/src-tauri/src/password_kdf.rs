//! API keys and share passwords: Argon2id for new hashes, SHA-256 for legacy verification.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sha2::{Digest, Sha256};

pub const ARGON2_MARKER: &str = "$argon2";

fn legacy_sha256_hex(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

/// Constant-time comparison of two hex strings (avoids timing attacks).
fn constant_time_eq_hex(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

/// Hash an API key for storage (always Argon2id PHC string).
pub fn hash_api_key(key: &str) -> String {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    Argon2::default()
        .hash_password(key.as_bytes(), &salt)
        .expect("argon2 hash")
        .to_string()
}

/// Verify API key against stored hash (Argon2 or legacy SHA-256 hex).
/// Returns (is_valid, should_upgrade) where should_upgrade indicates
/// the hash should be migrated from legacy SHA-256 to Argon2id.
pub fn verify_api_key(plaintext: &str, stored_hash: &str) -> (bool, bool) {
    if stored_hash.starts_with(ARGON2_MARKER) {
        let Ok(parsed) = PasswordHash::new(stored_hash) else {
            return (false, false);
        };
        let valid = Argon2::default()
            .verify_password(plaintext.as_bytes(), &parsed)
            .is_ok();
        (valid, false) // Already Argon2, no upgrade needed
    } else {
        // Legacy SHA-256 verification (constant-time comparison)
        let computed = legacy_sha256_hex(plaintext.as_bytes());
        let valid = constant_time_eq_hex(&computed, stored_hash);
        (valid, valid) // If valid, should upgrade to Argon2
    }
}

/// Legacy verification for backward compatibility (returns bool only).
/// Prefer `verify_api_key` which returns upgrade hint.
pub fn verify_api_key_legacy(plaintext: &str, stored_hash: &str) -> bool {
    verify_api_key(plaintext, stored_hash).0
}

/// New share password: Argon2 hash only; salt column left empty.
pub fn hash_share_password(password: &str) -> (String, Option<String>) {
    let hash = hash_api_key(password);
    (hash, None)
}

/// Verify share password (Argon2 PHC or legacy SHA-256(password||salt)).
pub fn verify_share_password(password: &str, stored_hash: &str, salt: Option<&str>) -> bool {
    if stored_hash.starts_with(ARGON2_MARKER) {
        verify_api_key_legacy(password, stored_hash)
    } else {
        let salt = salt.unwrap_or("");
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(salt.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        constant_time_eq_hex(&computed, stored_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_argon2_roundtrip() {
        let key = "ci-test-api-key-abcdef";
        let h = hash_api_key(key);
        assert!(h.starts_with(ARGON2_MARKER));
        let (valid, upgrade) = verify_api_key(key, &h);
        assert!(valid);
        assert!(!upgrade); // Argon2 hash doesn't need upgrade
        let (wrong_valid, _) = verify_api_key("wrong", &h);
        assert!(!wrong_valid);
    }

    #[test]
    fn api_key_legacy_sha256_still_valid() {
        let key = "legacy-key";
        let h = legacy_sha256_hex(key.as_bytes());
        assert!(!h.starts_with(ARGON2_MARKER));
        let (valid, upgrade) = verify_api_key(key, &h);
        assert!(valid);
        assert!(upgrade); // Legacy hash should be upgraded
    }

    #[test]
    fn verify_api_key_legacy_compat() {
        let key = "test-key";
        let h = hash_api_key(key);
        assert!(verify_api_key_legacy(key, &h));
        assert!(!verify_api_key_legacy("wrong", &h));
    }

    #[test]
    fn share_password_legacy_salt() {
        let pwd = "secret";
        let salt = "abcd";
        let mut hasher = Sha256::new();
        hasher.update(pwd.as_bytes());
        hasher.update(salt.as_bytes());
        let h = format!("{:x}", hasher.finalize());
        assert!(verify_share_password(pwd, &h, Some(salt)));
    }

    #[test]
    fn share_password_argon2_roundtrip() {
        let (h, salt) = hash_share_password("pw");
        assert!(salt.is_none());
        assert!(verify_share_password("pw", &h, None));
    }
}
