# Telegram Drive — Technical Concerns & Technical Debt

Generated: 2026-05-28

---

## 1. Security Concerns

### CRITICAL: Weak Password Hashing

| Issue | Detail |
|-------|--------|
| Location | API key hashing, share password hashing |
| Problem | Single-round SHA-256 used instead of proper KDF (Argon2, PBKDF2, bcrypt) |
| Impact | Passwords vulnerable to brute-force attacks with GPUs/ASICs |
| Files | `app/src-tauri/src/auth_routes.rs`, `app/src-tauri/src/share_routes.rs` |
| Status | ⚠️ Identified, not yet fixed |

### HIGH: unwrap() in Async Contexts

| Location | Count | Risk |
|----------|-------|------|
| `app/src-tauri/src/commands/auth.rs` | Multiple | Panic during auth can crash runner |
| Various route handlers | Scattered | Unhandled errors become panics |
| `app/src-tauri/src/commands/fs.rs` | Some | File operations may panic |
| Impact | Thread panic in Tokio runtime, potential denial of service |
| Status | ⚠️ Partially fixed in some files, remains in auth.rs |

### HIGH: std::sync::Mutex in Async Code

| Issue | Blocking synchronous mutex in async contexts |
|-------|---------------------------------------------|
| Location | Various state management locations |
| Problem | `std::sync::Mutex` blocks async executor threads |
| Fix | Migrate to `tokio::sync::Mutex` |
| Status | ⚠️ Identified, not yet fixed |

### HIGH: runner_shutdown Race Condition

| Location | `app/src-tauri/src/commands/auth.rs` |
|----------|--------------------------------------|
| Problem | Potential race condition in Telegram client shutdown |
| Impact | Client may not clean up properly, resource leaks |
| Status | ⚠️ Under investigation |

### MEDIUM: Error Message Disclosure

| Location | Some route handlers |
|----------|---------------------|
| Problem | Internal error details may leak in HTTP responses |
| Impact | Information disclosure for attackers |
| Fix | Sanitize error messages before sending to client |
| Status | ✅ Partially fixed in recent commits |

### LOW: XSS Potential in Share Routes

| Location | `app/src-tauri/src/share_routes.rs` |
|----------|-------------------------------------|
| Problem | User input rendered without HTML escaping |
| Fix | `html_escape` utility added to `http_middleware.rs` |
| Status | ✅ Fixed |

---

## 2. Performance Concerns

### Bandwidth Management

| Issue | Synchronous bandwidth tracking in async download paths |
|-------|--------------------------------------------------------|
| Location | `app/src-tauri/src/commands/fs.rs` |
| Problem | May block streaming during bandwidth update |
| Fix | Async bandwidth manager integration |
| Status | ✅ Recently fixed (async BandwidthManager) |

### Large File Handling

| Issue | Memory usage during upload/download of large files |
|-------|---------------------------------------------------|
| Solution | Chunked uploads with SHA256 verification |
| Status | ✅ Implemented, but needs stress testing |

### Database Connection Pooling

| Issue | SQLite connection management under concurrent load |
|-------|---------------------------------------------------|
| Status | ⚠️ Single connection may bottleneck under load |

---

## 3. Technical Debt

### Code Duplication

| Area | Description |
|------|-------------|
| Upload handlers | Similar logic in API routes and legacy routes |
| Error handling | Repeated match patterns across route files |
| Response formatting | Similar JSON envelope construction |

### Large Files

| File | Lines | Concern |
|------|-------|---------|
| `app/src-tauri/src/lib.rs` | Very large | Multiple responsibilities |
| `app/src-tauri/src/api_routes.rs` | Large | Could split by domain |
| `app/src-tauri/src/commands/fs.rs` | Large | Upload + download + listing |

### TODO / FIXME Comments

Search results from codebase:
- Various `TODO` markers for future improvements
- `FIXME` in error handling paths
- Legacy code marked for deprecation

### Deprecated Patterns

| Pattern | Location | Replacement |
|---------|----------|-------------|
| Legacy upload endpoint | `app/src-tauri/src/legacy_routes.rs` | New chunked upload API |
| Legacy form parsing | `app/src-tauri/src/legacy_form.rs` | Modern multipart handling |

---

## 4. Dependency Risks

### Git Dependencies

| Dependency | Source | Risk |
|------------|--------|------|
| `grammers-*` | Git: Lonami/grammers@d07f96f | No crates.io versioning, API may change |
| Impact | Updates may break compatibility without semver guarantees |
| Mitigation | Pin to specific rev, monitor upstream changes |

### Version Pinned Dependencies

- `sqlite = "0.37.0"` — Pin prevents security updates
- `zip = { version = "2", ... }` — Major version, may have API changes

### Tauri v2 Plugin Ecosystem

- Plugins tied to Tauri v2 major version
- Updates require coordinated upgrades

---

## 5. Build & Deployment Concerns

### Docker Build

| Issue | Detail |
|-------|--------|
| Dockerfile | Present but needs validation for headless-server feature |
| Multi-arch | Not confirmed for ARM64 builds |
| Image size | Rust builds can produce large images |

### CI/CD

| Issue | Detail |
|-------|--------|
| GitHub Actions | Only Docker API workflow present |
| Test stage | No automated test execution in CI |
| Security scanning | No dependency vulnerability scanning |

---

## 6. Fragile Areas

### Telegram Client Lifecycle

| Aspect | Risk |
|--------|------|
| Session handling | Session file corruption may lock users out |
| Flood waits | Telegram rate limits may cause unexpected delays |
| Proxy failures | Proxy configuration errors hard to diagnose |
| Reconnection | Network interruption handling needs testing |

### File Upload Pipeline

| Stage | Risk |
|-------|------|
| Chunking | Large files may fail mid-upload |
| Manifest merging | Manifest corruption may orphan chunks |
| SHA256 verification | Hash mismatch handling |
| Cleanup | Temp files may not be cleaned on panic |

### Share Link System

| Aspect | Risk |
|--------|------|
| Brute-force | Password protection relies on rate limiting |
| Link expiration | Expired links may not be cleaned promptly |
| Token validation | Session token forgery attempts |

---

## 7. Monitoring & Observability Gaps

| Gap | Impact |
|-----|--------|
| No metrics export | Cannot monitor performance in production |
| No health check endpoint | Load balancers cannot detect failures |
| Limited structured logging | JSON logging present but not comprehensive |
| No tracing | Cannot trace requests across async boundaries |

**Key Files:**
- `app/src-tauri/src/logging.rs` — Basic logging setup
- `app/src-tauri/src/server_uptime.rs` — Uptime tracking (partial)

---

## 8. Recent Security Improvements (May 2026)

| Improvement | File | Status |
|-------------|------|--------|
| Constant-time string comparison | `app/src-tauri/src/http_middleware.rs` | ✅ Added |
| Timing attack fix (stream_media) | `app/src-tauri/src/share_routes.rs` | ✅ Fixed |
| Timing attack fix (admin password) | `app/src-tauri/src/admin_routes.rs` | ✅ Fixed |
| unwrap() removal | `app/src-tauri/src/auth_routes.rs` | ✅ Fixed |
| Chunk validation (10MB cap) | `app/src-tauri/src/legacy_routes.rs` | ✅ Added |
| Error message sanitization | Multiple route files | ✅ Fixed |
| html_escape XSS prevention | `app/src-tauri/src/share_routes.rs` | ✅ Added |
| Manifest size limit (1MB) | `app/src-tauri/src/http_download.rs` | ✅ Added |
| Configurable upload limit | `app/src-tauri/src/server_http.rs` | ✅ Added |
| TempFileGuard RAII cleanup | `app/src-tauri/src/commands/utils.rs` | ✅ Added |

---

## 9. Detailed unwrap() / expect() / panic! Analysis

### 9.1 unwrap() Call Inventory

| File | Count | Critical Locations |
|------|-------|-------------------|
| `app/src-tauri/src/commands/fs.rs` | 15 | `client_opt.unwrap()` repeated 11 times (lines 42, 106, 240, 366, 404, 534, 578, 615, 700); `d.document.unwrap()` (lines 639, 665) |
| `app/src-tauri/src/vpn_optimizer.rs` | 15 | All `self.vpn.read().unwrap()` and `self.proxy.read().unwrap()` — uses `std::sync::RwLock` in async context |
| `app/src-tauri/src/commands/auth.rs` | 6 | `std::sync::Mutex` lock unwraps in async context; `app_handle.path().app_data_dir().unwrap()` (line 254) |
| `app/src-tauri/src/api_routes.rs` | 3 | `StatusCode::from_u16(status).unwrap()` (line 33); `fields.unwrap().contains(...)` pattern repeated 6 times (lines 312-327) |
| `app/src-tauri/src/lib.rs` | 2 | `handle_for_thread.lock().unwrap()` (lines 118, 207); `"149.154.167.50:443".parse().unwrap()` (line 235) |
| `app/src-tauri/src/commands/preview.rs` | 2 | Thumbnail generation unwraps |
| `app/src-tauri/src/commands/utils.rs` | 2 | Utility function unwraps |
| `app/src-tauri/src/share_routes.rs` | 2 | Share link handling unwraps |
| `app/src-tauri/src/server.rs` | 1 | Server startup unwrap |
| **Total** | **48** | |

### 9.2 expect() Call Inventory

| File | Count | Locations |
|------|-------|-----------|
| `app/src-tauri/src/route_registry.rs` | 4 | `std::fs::read_to_string(&path).expect("openapi.json readable")` (lines 47, 48, 53, 55) |
| `app/src-tauri/src/http_middleware.rs` | 2 | `store.lock().expect("rate limit lock")` (line 124); `self.token_buckets.lock().expect("share limiter lock")` (line 311) |
| `app/src-tauri/src/lib.rs` | 1 | `.expect("error while building tauri application")` (line 287) |
| `app/src-tauri/src/commands/network.rs` | 1 | Network config expect |
| **Total** | **8** | |

### 9.3 panic!() Macros

**Zero** `panic!` macros found in production code. This is a positive finding.

### 9.4 unsafe Blocks

**Effectively zero** `unsafe` blocks in the codebase. All memory safety is handled through safe Rust abstractions.

---

## 10. Hardcoded Values Inventory

### 10.1 Network Ports

| Location | Value | Context |
|----------|-------|---------|
| `app/src-tauri/src/lib.rs` | `STREAM_PORT: u16 = 14201` | Streaming server port |
| `app/src-tauri/src/lib.rs` | `API_PORT: u16 = 14200` | REST API server port |

### 10.2 Rate Limits

| Location | Value | Context |
|----------|-------|---------|
| `app/src-tauri/src/http_middleware.rs` | `ip_rpm = 120` | IP-based rate limit (requests per minute) |
| `app/src-tauri/src/http_middleware.rs` | `api_key_rpm = 300` | API key rate limit (requests per minute) |
| `app/src-tauri/src/http_middleware.rs` | `share_brute_limit = 10` | Share link brute force protection limit |

### 10.3 Timeouts, Intervals, and Magic Numbers

| Location | Value | Context |
|----------|-------|---------|
| `app/src-tauri/src/commands/auth.rs` | `100ms` | Session database retry sleep |
| `app/src-tauri/src/db.rs` | `86400 * 7` | Share link expiry (7 days in seconds) |
| `app/src-tauri/src/vpn_optimizer.rs` | Various | Retry delays, flood-wait multipliers, backoff exponents |
| `app/src-tauri/src/legacy_routes.rs` | `10 * 1024 * 1024` | Chunk size cap (10MB) |
| `app/src-tauri/src/http_download.rs` | `1 * 1024 * 1024` | Manifest size limit (1MB) |

### 10.4 Telegram DC IP Addresses

`app/src-tauri/src/commands/network.rs`:
```rust
const DC_ADDRESSES: &[&str] = &[
    "149.154.167.50:443",  // DC2
    "149.154.167.51:443",  // DC3
    "149.154.167.91:443",  // DC4
];
```

`app/src-tauri/src/lib.rs` line 235:
```rust
"149.154.167.50:443".parse().unwrap()
```

These should be configurable or fetched from Telegram's official DC configuration endpoint.

---

## 11. Error Handling Density

Files with the highest `map_err()` call counts (indicating complex error translation):

| File | map_err() Count | Concern |
|------|-----------------|---------|
| `app/src-tauri/src/db.rs` | 58 | Complex DB error mapping; consider structured error types |
| `app/src-tauri/src/commands/fs.rs` | 34 | File operation error translation |
| `app/src-tauri/src/api_routes.rs` | 28 | API response error mapping |
| `app/src-tauri/src/share_routes.rs` | 22 | Share link error handling |
| `app/src-tauri/src/legacy_routes.rs` | 18 | Legacy upload error paths |

High `map_err()` density suggests ad-hoc string-based error handling. Consider migrating to `thiserror` or `anyhow` for structured error propagation.

---

## 12. Mock Code in Production

Mock/test code blocks remain in production source files:

- `app/src-tauri/src/commands/fs.rs` — mock filesystem blocks at lines 31-36, 103, 236-238, 363, 400-401, 531, 575-576
- `app/src-tauri/src/commands/fs.rs` — `#[cfg(test)]` mod with `MockFilesystem` struct compiled in test builds

These should be extracted to dedicated test helper modules or removed if no longer needed.

---

## 13. Code Duplication Hotspots

| Duplication | Locations | Impact |
|-------------|-----------|--------|
| Chunk size constants | `api_routes.rs`, `legacy_routes.rs`, `share_routes.rs`, `share_api_routes.rs` | Drift risk, inconsistent limits |
| Retry logic | `vpn_optimizer.rs`, `http_upload.rs` | Divergent behavior, maintenance burden |
| Upload handlers | `api_routes.rs`, `legacy_routes.rs` | Security fixes must be applied in multiple places |
| Response formatting | Multiple route files | Inconsistent API envelopes |
| Sharing helpers | `share_routes.rs`, `share_api_routes.rs`, `sharing_core.rs` | Overlapping password/validation logic |

---

## 14. Remediation Priority

### CRITICAL (Fix Immediately)

1. **Replace `client_opt.unwrap()` in `fs.rs`** — Panics when Telegram client is disconnected; affects all file operations
2. **Replace SHA256 password hashing with Argon2id/bcrypt** — Vulnerable to brute-force attacks
3. **Replace `std::sync::Mutex` with `tokio::sync::Mutex` in async contexts** — Causes thread blocking and potential deadlocks

### HIGH (Fix in Next Sprint)

4. **Replace `unwrap()` on Telegram message document access** — `d.document.unwrap()` panics on non-document messages
5. **Add rate limit store entry expiry/cleanup** — Unbounded memory growth over long uptimes
6. **Extract hardcoded constants to config module** — Ports, rate limits, timeouts, DC addresses
7. **Unify duplicate retry logic** — Merge `vpn_optimizer.rs` and `http_upload.rs` retry implementations

### MEDIUM (Fix in Next Quarter)

8. **Migrate high `map_err()` files to structured error types** — `thiserror`/`anyhow` for maintainability
9. **Remove or isolate mock code from production files**
10. **Add SQLite connection pooling** — Replace single serialized connection
11. **Add health check endpoint** — Required for production load balancer integration

### LOW (Backlog)

12. **Remove remaining `expect()` calls** — Mostly in startup and config paths
13. **Add request tracing across async boundaries**
14. **Add dependency vulnerability scanning to CI**
15. **Validate Docker builds for headless-server feature**
