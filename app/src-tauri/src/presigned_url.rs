//! HMAC-SHA256 presigned download URLs (OSS-style, no DB row for the link itself).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const CANONICAL_VERSION: &str = "v1";

#[derive(Debug, Clone)]
pub struct PresignedParams {
    pub message_id: i32,
    pub folder_id: Option<i64>,
    pub expires_at: i64,
    pub owner_id: String,
    pub max_downloads: Option<u32>,
}

fn canonical_payload(p: &PresignedParams) -> String {
    let folder = p.folder_id.map(|f| f.to_string()).unwrap_or_default();
    let max_dl = p.max_downloads.map(|m| m.to_string()).unwrap_or_default();
    format!(
        "{}|{}|{}|{}|{}|{}",
        CANONICAL_VERSION, p.message_id, folder, p.expires_at, p.owner_id, max_dl
    )
}

pub fn sign(params: &PresignedParams, secret: &str) -> Result<String, String> {
    if secret.len() < 32 {
        return Err("DOWNLOAD_SIGNING_SECRET must be at least 32 characters".to_string());
    }
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("HMAC init failed: {e}"))?;
    mac.update(canonical_payload(params).as_bytes());
    Ok(format!("{:x}", mac.finalize().into_bytes()))
}

pub fn verify(params: &PresignedParams, secret: &str, sig_hex: &str) -> bool {
    let Ok(expected) = sign(params, secret) else {
        return false;
    };
    constant_time_eq(expected.as_bytes(), sig_hex.trim().as_bytes())
}

pub fn build_presigned_url(
    base_url: &str,
    params: &PresignedParams,
    secret: &str,
) -> Result<String, String> {
    let sig = sign(params, secret)?;
    let base = base_url.trim_end_matches('/');
    let folder_q = params
        .folder_id
        .map(|f| format!("&folder_id={f}"))
        .unwrap_or_default();
    let max_dl_q = params
        .max_downloads
        .map(|m| format!("&max_downloads={m}"))
        .unwrap_or_default();
    Ok(format!(
        "{base}/d/signed?file_id={}&exp={}{folder_q}&owner={}&sig={}{max_dl_q}",
        params.message_id,
        params.expires_at,
        urlencoding::encode(&params.owner_id),
        sig
    ))
}

pub fn parse_query(
    file_id: i32,
    folder_id: Option<i64>,
    exp: i64,
    owner: &str,
    sig: &str,
    max_downloads: Option<u32>,
) -> PresignedParams {
    PresignedParams {
        message_id: file_id,
        folder_id,
        expires_at: exp,
        owner_id: owner.to_string(),
        max_downloads,
    }
}

pub fn is_expired(expires_at: i64) -> bool {
    if expires_at <= 0 {
        return false;
    }
    chrono::Utc::now().timestamp() > expires_at
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let secret = "a".repeat(32);
        let p = PresignedParams {
            message_id: 42,
            folder_id: Some(-1001),
            expires_at: 4_102_444_800,
            owner_id: "tenant:acme".to_string(),
            max_downloads: None,
        };
        let sig = sign(&p, &secret).unwrap();
        assert!(verify(&p, &secret, &sig));
        assert!(!verify(&p, &secret, "deadbeef"));
    }

    #[test]
    fn sign_and_verify_with_max_downloads() {
        let secret = "a".repeat(32);
        let p = PresignedParams {
            message_id: 42,
            folder_id: Some(-1001),
            expires_at: 4_102_444_800,
            owner_id: "tenant:acme".to_string(),
            max_downloads: Some(3),
        };
        let sig = sign(&p, &secret).unwrap();
        assert!(verify(&p, &secret, &sig));

        // Different max_downloads should fail
        let mut p2 = p.clone();
        p2.max_downloads = Some(2);
        assert!(!verify(&p2, &secret, &sig));
    }

    #[test]
    fn signature_binds_owner_and_cannot_cross_tenant() {
        let secret = "b".repeat(32);
        let original = PresignedParams {
            message_id: 7,
            folder_id: Some(-1007),
            expires_at: 4_102_444_800,
            owner_id: "tenant:alpha".to_string(),
            max_downloads: Some(1),
        };
        let signature = sign(&original, &secret).expect("signature");
        let mut tampered = original.clone();
        tampered.owner_id = "tenant:beta".to_string();
        assert!(!verify(&tampered, &secret, &signature));
    }

    #[test]
    fn exp_zero_never_expires() {
        assert!(!is_expired(0));
        assert!(!is_expired(-1));
    }
}
