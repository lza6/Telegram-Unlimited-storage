use actix_web::body::{to_bytes, BoxBody, EitherBody, MessageBody};
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header;
use actix_web::HttpMessage;
use actix_web::HttpResponse;
use futures_util::future::LocalBoxFuture;
use rand::RngCore;
use std::collections::HashMap;
use std::future::{ready, Ready};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::server_config::RateLimitConfig;

/// Per-request CSP nonce stored in request extensions so handlers/templates can reuse it.
pub struct CspNonce(pub String);

/// Generate a random 16-byte nonce for CSP.
fn generate_csp_nonce() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Build a CSP header that uses a nonce for scripts/styles while keeping
/// `unsafe-inline` for style attributes (used by the web console static pages).
fn build_csp(nonce: &str) -> String {
    format!(
        "default-src 'self'; script-src 'self' 'nonce-{nonce}' https://unpkg.com https://cdn.jsdelivr.net; style-src 'self' 'unsafe-inline' 'nonce-{nonce}'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' http://localhost:* http://127.0.0.1:*; frame-ancestors 'none'; base-uri 'self'; form-action 'self';",
    )
}

/// Inject a `nonce` attribute into inline `<script>` and `<style>` opening tags
/// so that modern browsers can execute them under a nonce-based CSP.
fn inject_nonce(html: &str, nonce: &str) -> String {
    let mut out = String::with_capacity(html.len() + nonce.len() * 4);
    let mut i = 0;
    while i < html.len() {
        match html[i..].find('<') {
            Some(pos) => {
                let absolute = i + pos;
                out.push_str(&html[i..absolute + 1]);
                i = absolute + 1;

                let tag_lower = html[i..].chars().take(6).collect::<String>().to_lowercase();
                let tag = if tag_lower.starts_with("script") {
                    Some("script")
                } else if tag_lower.starts_with("style") {
                    Some("style")
                } else {
                    None
                };

                if let Some(tag_name) = tag {
                    let tag_len = tag_name.len();
                    // Only inject into the opening tag, not closing `</script>`.
                    if let Some(end_rel) = html[i + tag_len..].find('>') {
                        let end = i + tag_len + end_rel;
                        let tag_content = &html[i..end];
                        if !tag_content.contains("nonce=") {
                            out.push_str(tag_content);
                            out.push_str(&format!(" nonce=\"{}\"", nonce));
                            out.push('>');
                        } else {
                            out.push_str(&html[i..end + 1]);
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            None => {
                out.push_str(&html[i..]);
                break;
            }
        }
    }
    out
}

/// Constant-time string comparison to prevent timing attacks.
/// Compares up to max(a.len(), b.len()) bytes to avoid leaking length info.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let max_len = a_bytes.len().max(b_bytes.len());
    let mut result = 0u8;
    for i in 0..max_len {
        let x = a_bytes.get(i).copied().unwrap_or(0);
        let y = b_bytes.get(i).copied().unwrap_or(0);
        result |= x ^ y;
    }
    result == 0 && a_bytes.len() == b_bytes.len()
}

#[derive(Clone)]
pub struct SecurityHeaders;

impl<S, B> Transform<S, ServiceRequest> for SecurityHeaders
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = SecurityHeadersMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SecurityHeadersMiddleware { service }))
    }
}

pub struct SecurityHeadersMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for SecurityHeadersMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let nonce = generate_csp_nonce();
        req.extensions_mut().insert(CspNonce(nonce.clone()));
        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            res.headers_mut().insert(
                header::HeaderName::from_static("x-content-type-options"),
                header::HeaderValue::from_static("nosniff"),
            );
            res.headers_mut().insert(
                header::HeaderName::from_static("x-frame-options"),
                header::HeaderValue::from_static("deny"),
            );
            res.headers_mut().insert(
                header::HeaderName::from_static("referrer-policy"),
                header::HeaderValue::from_static("strict-origin-when-cross-origin"),
            );
            // CSP: nonce-based for scripts/styles; style attributes still allowed via
            // 'unsafe-inline' for the web console static pages.
            let csp = build_csp(&nonce);
            let is_html = res
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.starts_with("text/html"))
                .unwrap_or(false);

            let res = if is_html {
                let req_clone = res.request().clone();
                let status = res.status();
                let mut headers = res.headers().clone();
                let body = res.into_body();
                let bytes = to_bytes(body).await.map_err(|_| {
                    actix_web::error::ErrorInternalServerError(
                        "failed to read response body for CSP nonce injection",
                    )
                })?;
                let html = String::from_utf8_lossy(&bytes);
                let modified = inject_nonce(&html, &nonce);
                let mut new_res = HttpResponse::with_body(status, BoxBody::new(modified));
                std::mem::swap(new_res.headers_mut(), &mut headers);
                ServiceResponse::new(req_clone, new_res)
            } else {
                res.map_body(|_, body| BoxBody::new(body))
            };

            let mut res = res;
            if let Ok(v) = header::HeaderValue::from_str(&csp) {
                res.headers_mut().insert(
                    header::HeaderName::from_static("content-security-policy"),
                    v,
                );
            }
            // Permissions-Policy: disable unnecessary browser features
            res.headers_mut().insert(
                header::HeaderName::from_static("permissions-policy"),
                header::HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=(), magnetometer=(), gyroscope=()"),
            );
            Ok(res)
        })
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    ip_rpm: u32,
    api_key_rpm: u32,
    ip_buckets: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    key_buckets: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(cfg: &RateLimitConfig) -> Self {
        Self {
            ip_rpm: cfg.ip_rpm,
            api_key_rpm: cfg.api_key_rpm,
            ip_buckets: Arc::new(Mutex::new(HashMap::new())),
            key_buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Prune expired entries from a bucket store to prevent unbounded memory growth.
    pub fn prune_stale(store: &Mutex<HashMap<String, Vec<Instant>>>, window: Duration) {
        if let Ok(mut map) = store.lock() {
            let cutoff = Instant::now() - window;
            map.retain(|_, bucket| {
                bucket.retain(|t| *t > cutoff);
                !bucket.is_empty()
            });
        }
    }

    /// Periodic cleanup task — call from a background thread/timer.
    pub fn start_cleanup_task(limiter: Arc<RateLimiter>, interval_secs: u64) {
        if interval_secs == 0 {
            return;
        }
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(interval_secs));
            let window = Duration::from_secs(60);
            Self::prune_stale(&limiter.ip_buckets, window);
            Self::prune_stale(&limiter.key_buckets, window);
        });
    }

    fn prune(bucket: &mut Vec<Instant>, window: Duration) {
        let cutoff = Instant::now() - window;
        bucket.retain(|t| *t > cutoff);
    }

    fn check_bucket(
        store: &Mutex<HashMap<String, Vec<Instant>>>,
        key: &str,
        limit: u32,
    ) -> Result<u32, u64> {
        if limit == 0 {
            return Ok(u32::MAX);
        }
        let window = Duration::from_secs(60);
        let mut map = store.lock().expect("rate limit lock");
        let bucket = map.entry(key.to_string()).or_default();
        Self::prune(bucket, window);
        if bucket.len() as u32 >= limit {
            let retry = bucket
                .first()
                .map(|t| window.saturating_sub(t.elapsed()).as_secs().max(1))
                .unwrap_or(1);
            return Err(retry);
        }
        bucket.push(Instant::now());
        Ok(limit.saturating_sub(bucket.len() as u32))
    }

    pub fn check_ip(&self, ip: &str) -> Result<u32, u64> {
        Self::check_bucket(&self.ip_buckets, ip, self.ip_rpm)
    }

    pub fn check_api_key(&self, key: &str) -> Result<u32, u64> {
        Self::check_bucket(&self.key_buckets, key, self.api_key_rpm)
    }
}

#[derive(Clone)]
pub struct RateLimit {
    limiter: Arc<RateLimiter>,
}

impl RateLimit {
    pub fn new(limiter: Arc<RateLimiter>) -> Self {
        Self { limiter }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RateLimitMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddleware {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

pub struct RateLimitMiddleware<S> {
    service: S,
    limiter: Arc<RateLimiter>,
}

fn client_ip(req: &ServiceRequest) -> String {
    req.connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get("X-Forwarded-For")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_health_path(path: &str) -> bool {
    matches!(path, "/api/v1/health" | "/health/live" | "/health/ready")
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let limiter = self.limiter.clone();

        if is_health_path(&path) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        let ip = client_ip(&req);
        match limiter.check_ip(&ip) {
            Err(retry) => {
                return Box::pin(async move {
                    Ok(req.into_response(
                        HttpResponse::TooManyRequests()
                            .insert_header((header::RETRY_AFTER, retry.to_string()))
                            .insert_header(("X-RateLimit-Limit", limiter.ip_rpm.to_string()))
                            .insert_header(("X-RateLimit-Remaining", "0"))
                            .json(serde_json::json!({
                                "error": { "code": "RATE_LIMITED", "message": "Too many requests from this IP" }
                            }))
                            .map_into_right_body(),
                    ))
                });
            }
            Ok(ip_remaining) => {
                if let Some(api_key) = req
                    .headers()
                    .get("X-API-Key")
                    .and_then(|v| v.to_str().ok())
                    .filter(|k| !k.is_empty())
                {
                    if let Err(retry) = limiter.check_api_key(api_key) {
                        return Box::pin(async move {
                            Ok(req.into_response(
                                HttpResponse::TooManyRequests()
                                    .insert_header((header::RETRY_AFTER, retry.to_string()))
                                    .insert_header(("X-RateLimit-Limit", limiter.api_key_rpm.to_string()))
                                    .insert_header(("X-RateLimit-Remaining", "0"))
                                    .json(serde_json::json!({
                                        "error": { "code": "RATE_LIMITED", "message": "API key rate limit exceeded" }
                                    }))
                                    .map_into_right_body(),
                            ))
                        });
                    }
                }
                let fut = self.service.call(req);
                Box::pin(async move {
                    let mut res = fut.await?;
                    res.headers_mut().insert(
                        actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
                        actix_web::http::header::HeaderValue::from_str(&limiter.ip_rpm.to_string())
                            .unwrap(),
                    );
                    res.headers_mut().insert(
                        actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
                        actix_web::http::header::HeaderValue::from_str(&ip_remaining.to_string())
                            .unwrap(),
                    );
                    Ok(res.map_into_left_body())
                })
            }
        }
    }
}

/// Per-token rate limiter for share password verification (brute-force protection).
#[derive(Clone)]
pub struct ShareBruteForceLimiter {
    token_buckets: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_attempts: u32,
    window: Duration,
}

impl ShareBruteForceLimiter {
    pub fn new(max_attempts: u32, window_secs: u64) -> Self {
        Self {
            token_buckets: Arc::new(Mutex::new(HashMap::new())),
            max_attempts,
            window: Duration::from_secs(window_secs),
        }
    }

    fn prune(bucket: &mut Vec<Instant>, window: Duration) {
        let cutoff = Instant::now() - window;
        bucket.retain(|t| *t > cutoff);
    }

    pub fn check_token(&self, token: &str) -> Result<(), (u64, &'static str)> {
        if self.max_attempts == 0 {
            return Ok(());
        }
        let mut map = self.token_buckets.lock().expect("share limiter lock");
        let bucket = map.entry(token.to_string()).or_default();
        Self::prune(bucket, self.window);
        if bucket.len() as u32 >= self.max_attempts {
            let retry = bucket
                .first()
                .map(|t| self.window.saturating_sub(t.elapsed()).as_secs().max(1))
                .unwrap_or(1);
            return Err((retry, "Too many password attempts for this link"));
        }
        bucket.push(Instant::now());
        Ok(())
    }
}

pub fn build_cors(origins: &[String]) -> actix_cors::Cors {
    if origins.is_empty() {
        actix_cors::Cors::default()
    } else {
        let mut cors = actix_cors::Cors::default()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
        for origin in origins {
            cors = cors.allowed_origin(origin.as_str());
        }
        cors
    }
}

fn new_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Logs structured request metadata and adds `X-Request-Id` to responses.
#[derive(Clone, Default)]
pub struct RequestLog;

impl<S, B> Transform<S, ServiceRequest> for RequestLog
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = RequestLogMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestLogMiddleware { service }))
    }
}

pub struct RequestLogMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestLogMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = new_request_id();
        let method = req.method().to_string();
        let path = req.path().to_string();
        let started = Instant::now();
        req.extensions_mut().insert(request_id.clone());
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            let status = res.status().as_u16();
            let duration_ms = started.elapsed().as_millis();
            log::info!(
                target: "http_request",
                "{{\"request_id\":\"{request_id}\",\"method\":\"{method}\",\"path\":\"{path}\",\"status\":{status},\"duration_ms\":{duration_ms}}}"
            );
            let mut res = res;
            if let Ok(v) = actix_web::http::header::HeaderValue::from_str(&request_id) {
                res.headers_mut().insert(
                    actix_web::http::header::HeaderName::from_static("x-request-id"),
                    v,
                );
            }
            Ok(res.map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server_config::RateLimitConfig;

    #[test]
    fn rate_limiter_blocks_after_limit() {
        let cfg = RateLimitConfig {
            ip_rpm: 2,
            api_key_rpm: 100,
        };
        let limiter = RateLimiter::new(&cfg);
        assert!(limiter.check_ip("1.2.3.4").is_ok());
        assert!(limiter.check_ip("1.2.3.4").is_ok());
        assert!(limiter.check_ip("1.2.3.4").is_err());
    }

    #[test]
    fn all_health_probes_bypass_rate_limiting() {
        assert!(is_health_path("/api/v1/health"));
        assert!(is_health_path("/health/live"));
        assert!(is_health_path("/health/ready"));
        assert!(!is_health_path("/api/v1/files"));
    }

    #[test]
    fn api_key_limit_is_separate() {
        let cfg = RateLimitConfig {
            ip_rpm: 1000,
            api_key_rpm: 1,
        };
        let limiter = RateLimiter::new(&cfg);
        assert!(limiter.check_api_key("key-a").is_ok());
        assert!(limiter.check_api_key("key-a").is_err());
        assert!(limiter.check_api_key("key-b").is_ok());
    }

    #[test]
    fn csp_nonce_is_hex_and_unique() {
        let a = generate_csp_nonce();
        let b = generate_csp_nonce();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn csp_header_contains_nonce_and_restricts_external_scripts() {
        let nonce = generate_csp_nonce();
        let csp = build_csp(&nonce);
        assert!(csp.contains(&format!("'nonce-{nonce}'")));
        assert!(csp.contains("script-src 'self'"));
        assert!(csp.contains("https://unpkg.com"));
        assert!(csp.contains("https://cdn.jsdelivr.net"));
        assert!(!csp.contains("'unsafe-eval'"));
    }

    #[test]
    fn inject_nonce_adds_nonce_to_inline_tags() {
        let html = "<script>alert(1)</script><style>body{}</style>";
        let nonce = "abc123";
        let out = inject_nonce(html, nonce);
        assert!(out.contains("<script nonce=\"abc123\">"));
        assert!(out.contains("<style nonce=\"abc123\">"));
        assert!(out.contains("</script>"));
        assert!(out.contains("</style>"));
    }

    #[test]
    fn inject_nonce_does_not_duplicate_existing_nonce() {
        let html = "<script nonce=\"old\">alert(1)</script>";
        let out = inject_nonce(html, "new");
        assert!(out.contains("nonce=\"old\""));
        assert!(!out.contains("nonce=\"new\""));
    }

    #[test]
    fn inject_nonce_ignores_closing_tags() {
        let html = "<script></script>";
        let out = inject_nonce(html, "n");
        assert_eq!(out.matches("nonce=").count(), 1);
    }
}
