# Coding Conventions

> Document version: 2026-05-28
> Scope: Telegram-Drive Rust backend (`app/src-tauri/src/`)

---

## 1. Code Style & Formatting

### rustfmt
- The project relies on **default rustfmt settings** (4-space indent, 100-character line width).
- No `rustfmt.toml` or `clippy.toml` exists in the repository.
- Run `cargo fmt` before committing.

### clippy
- Run `cargo clippy -- -D warnings` (treat warnings as errors).
- No custom clippy configuration is present.

### Line Width & Indentation
- 4-space indentation (rustfmt default).
- Max line width: 100 characters.
- Use trailing commas in multi-line struct/enum definitions and match arms.

---

## 2. Naming Conventions

Follow standard Rust conventions:

| Category | Case | Examples from codebase |
|----------|------|------------------------|
| Functions, methods, variables, modules | `snake_case` | `cmd_upload_file`, `json_error`, `peer_cache` |
| Types, traits, enums, structs | `PascalCase` | `TelegramState`, `AuthState`, `ErrorDetail` |
| Constants, statics | `SCREAMING_SNAKE_CASE` | `STREAM_PORT`, `MAX_RETRIES` |
| Lifetimes | Short lowercase (`'a`, `'de`) | — |

### Tauri Command Naming
- All Tauri commands use the `cmd_` prefix to distinguish them from internal helpers:
  - `cmd_upload_file` in `app/src-tauri/src/commands/fs.rs`
  - `cmd_login_start` in `app/src-tauri/src/commands/auth.rs`
  - `cmd_get_thumbnail` in `app/src-tauri/src/commands/preview.rs`

### Error Response Codes
- Error codes use `snake_case` strings:
  - `"unauthorized"`, `"invalid_request"`, `"not_found"`, `"internal_error"` in `app/src-tauri/src/api_routes.rs`

---

## 3. Error Handling Patterns

### Production Code: `Result<T, E>` with `?`
- Use `Result` propagation with `?` throughout.
- Never use `unwrap()` or `expect()` in production code except for truly unreachable states.

### Application Errors: `anyhow`
- The codebase uses `anyhow::Context` for adding error context:
  ```rust
  // From app/src-tauri/src/commands/auth.rs
  let session = SqliteSession::new(&db_path)
      .with_context(|| format!("failed to open session DB at {db_path}"))?;
  ```

### API Error Responses
- Standardized envelope in `app/src-tauri/src/api_routes.rs`:
  ```rust
  #[derive(Serialize)]
  struct ErrorBody { error: ErrorDetail }

  #[derive(Serialize)]
  struct ErrorDetail { code: String, message: String }

  fn json_error(code: &str, message: &str, status: u16) -> HttpResponse {
      HttpResponse::build(StatusCode::from_u16(status).unwrap())
          .json(ErrorBody { error: ErrorDetail { code: code.to_string(), message: message.to_string() } })
  }
  ```
- All REST API errors return JSON with `code` and `message` fields.

### Tauri Command Errors
- Tauri commands return `Result<T, String>` where the error is a user-facing message:
  ```rust
  #[tauri::command]
  pub async fn cmd_upload_file(...) -> Result<String, String>
  ```

### Logging Errors
- Use `tracing` / `log` macros for server-side error logging:
  - `tracing::error!` for unexpected errors
  - `tracing::info!` for routine events (e.g., "order not found")

---

## 4. Common Code Patterns

### State Management: `Arc<Mutex<T>>` / `Arc<RwLock<T>>`
- Shared state is wrapped in `Arc` with tokio's async synchronization primitives:
  ```rust
  // From app/src-tauri/src/commands/mod.rs
  pub struct TelegramState {
      pub client: Arc<Mutex<Option<Client>>>,
      pub peer_cache: Arc<tokio::sync::RwLock<HashMap<i64, Peer>>>,
      pub cancelled_transfers: Arc<tokio::sync::RwLock<HashSet<String>>>,
  }
  ```

### RAII Guards
- `TempFileGuard` in `app/src-tauri/src/commands/fs.rs` for automatic temp file cleanup:
  ```rust
  struct TempFileGuard(PathBuf);
  impl Drop for TempFileGuard {
      fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
  }
  ```

### Progress Tracking
- `ProgressReader` in `app/src-tauri/src/commands/fs.rs` wraps a reader to emit progress events:
  ```rust
  struct ProgressReader<R> { inner: R, emitted: u64, total: u64, ... }
  impl<R: AsyncRead + Unpin> AsyncRead for ProgressReader<R> { ... }
  ```

### Retry Logic
- `with_retry` and `with_retry_telegram` in `app/src-tauri/src/vpn_optimizer.rs`:
  - Exponential backoff with jitter
  - Special handling for Telegram `FLOOD_WAIT` errors
  ```rust
  pub async fn with_retry_telegram<F, Fut, T>(f: F) -> Result<T, anyhow::Error>
  where F: FnMut() -> Fut, Fut: std::future::Future<Output = Result<T, InvocationError>>
  ```

### Peer Caching
- `peer_cache: Arc<RwLock<HashMap<i64, Peer>>>` avoids O(N) dialog scanning on every operation.

---

## 5. API Response Formats

### REST API Success Responses
- Plain JSON payloads (no envelope wrapper for success):
  ```rust
  // From app/src-tauri/src/api_routes.rs
  HttpResponse::Ok().json(file_list)
  HttpResponse::Ok().json(json!({ "id": file_id, "name": name }))
  ```

### REST API Error Responses
- Wrapped in `ErrorBody` / `ErrorDetail`:
  ```json
  {
    "error": {
      "code": "unauthorized",
      "message": "Invalid or missing API key"
    }
  }
  ```

### Tauri Command Responses
- Success: direct JSON-serializable value
- Error: `Err(String)` with user-facing message

---

## 6. Logging Patterns

### Logger: `env_logger` with optional JSON
- Environment variable `LOG_FORMAT=json` switches to JSON logging.
- Otherwise uses human-readable format.

### Log Levels
- `tracing::error!` — unexpected failures, internal errors
- `tracing::warn!` — recoverable issues, degraded states
- `tracing::info!` — routine operational events
- `tracing::debug!` — diagnostic detail during development

### Structured Logging
- Use key-value pairs with tracing:
  ```rust
  tracing::info!(order_id = id, "order not found");
  tracing::error!(order_id = id, error = %e, "unexpected error");
  ```

---

## 7. Documentation Conventions

### Doc Comments
- Use `///` for public API documentation.
- Use `//` for implementation notes.
- No strict requirement for module-level docs (`//!`), but encouraged for new modules.

### TODO / FIXME
- Mark temporary workarounds with `// TODO:` or `// FIXME:`.
- No automated enforcement; rely on code review.

---

## 8. Linting & CI Rules

### GitHub Actions Workflows

#### `.github/workflows/docker-api.yml`
- Runs `cargo test --no-default-features --features headless-server --lib --tests`
- Builds Docker image and runs smoke tests (bash + PowerShell)

#### `.github/workflows/main.yml`
- Tauri app publishing for Windows, Linux, macOS

### Cargo Features
- `default = ["desktop"]` — Tauri desktop app mode
- `headless-server` — Standalone server mode without Tauri
- Tests run with `--no-default-features --features headless-server` in CI

### Security Headers (enforced in code)
- `Content-Security-Policy`
- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: strict-origin-when-cross-origin`
- `Permissions-Policy` — restricted camera/microphone/geolocation

### Rate Limiting
- Per-IP and per-API-key sliding window buckets in `app/src-tauri/src/http_middleware.rs`
- `ShareBruteForceLimiter` for password-protected share links

---

## 9. Module Organization

Organize by domain, not by type:

```text
app/src-tauri/src/
├── main.rs                    # Binary entry point (headless-server)
├── lib.rs                     # Tauri app setup, command registration
├── commands/                  # Tauri commands by domain
│   ├── mod.rs                 # TelegramState definition
│   ├── auth.rs                # Login/logout commands
│   ├── fs.rs                  # File operations
│   ├── preview.rs             # Thumbnail generation
│   ├── api_settings.rs        # API key management
│   └── utils.rs               # Utility commands
├── api_routes.rs              # REST API endpoints (Actix-web)
├── share_routes.rs            # Public share link endpoints
├── auth_routes.rs             # Authentication endpoints
├── admin_routes.rs            # Admin endpoints
├── legacy_routes.rs           # Legacy upload endpoints
├── share_api_routes.rs        # Share API endpoints
├── http_middleware.rs         # Security headers, rate limiting, CORS
├── http_upload.rs             # HTTP upload handlers
├── http_download.rs           # HTTP download handlers
├── vpn_optimizer.rs           # Retry logic, Telegram flood-wait handling
├── db.rs                      # SQLite operations
├── server.rs                  # Actix server setup
├── server_http.rs             # HTTP server configuration
├── server_config.rs           # Server configuration
├── server_uptime.rs           # Uptime tracking
├── route_registry.rs          # Canonical route list, OpenAPI contract test
├── models.rs                  # Shared data models
├── bandwidth.rs               # Bandwidth throttling
├── logging.rs                 # Logging setup
├── sharing_core.rs            # Share link core logic
├── legacy_form.rs             # Legacy form parsing
└── tests/                     # Integration tests
    └── health_api.rs
```

---

## 10. Feature Gating

- Use `#[cfg(feature = "desktop")]` for Tauri-specific code.
- Use `#[cfg(feature = "headless-server")]` for headless-specific code.
- Use `#[cfg(test)]` for test-only modules and imports.

---

## 11. Security Patterns

### Constant-Time Comparison
- `constant_time_eq` in `app/src-tauri/src/http_middleware.rs` for timing-safe string comparison:
  ```rust
  pub fn constant_time_eq(a: &str, b: &str) -> bool {
      if a.len() != b.len() { return false; }
      a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
  }
  ```

### API Key Hashing
- API keys are stored as SHA256 hashes, verified with constant-time comparison.

### Input Validation
- Validate at system boundaries before processing.
- Reject invalid input with clear error messages.

---

## 12. Async Patterns

### tokio Runtime
- Uses `tokio::sync::Mutex` and `tokio::sync::RwLock` for async synchronization.
- Avoid `std::sync::Mutex` in async contexts unless necessary.

### Cancellation
- `cancelled_transfers: Arc<RwLock<HashSet<String>>>` tracks cancelled operations.
- Check cancellation token before expensive operations.

---

*End of CONVENTIONS.md*
