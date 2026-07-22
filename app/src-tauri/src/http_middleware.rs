use actix_web::body::{EitherBody, MessageBody};
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header;
use actix_web::HttpMessage;
use actix_web::HttpResponse;
use futures_util::future::LocalBoxFuture;
use std::collections::HashMap;
use std::future::{ready, Ready};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::server_config::RateLimitConfig;

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
    type Response = ServiceResponse<EitherBody<B>>;
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
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
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
            // CSP: restrict to self-hosted resources only.
            // Note: Swagger UI (if used) should be served from /docs static files
            // rather than loaded from external CDN. If external CDN is required,
            // add its domain to script-src and style-src explicitly.
            res.headers_mut().insert(
                header::HeaderName::from_static("content-security-policy"),
                header::HeaderValue::from_static(
                    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' http://localhost:* http://127.0.0.1:*; frame-ancestors 'none'; base-uri 'self'; form-action 'self';",
                ),
            );
            // Permissions-Policy: disable unnecessary browser features
            res.headers_mut().insert(
                header::HeaderName::from_static("permissions-policy"),
                header::HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=(), magnetometer=(), gyroscope=()"),
            );
            Ok(res.map_into_left_body())
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
    path == "/api/v1/health"
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
}
