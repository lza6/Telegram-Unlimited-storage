# Telegram Drive — External Integrations

Generated: 2026-05-28

---

## 1. Telegram API (MTProto)

### 1.1 Client Library

| Aspect | Detail |
|--------|--------|
| Library | `grammers-client` (Git: `Lonami/grammers@d07f96f`) |
| Protocol | MTProto (Telegram's native binary protocol) |
| Transport | TCP with optional proxy layer |
| Session Storage | SQLite via `grammers-session` |

### 1.2 Authentication Flows

| Flow | Method | Frontend Support |
|------|--------|------------------|
| Phone + Code | `grammers_client::Client::sign_in` | Phone input + OTP code |
| Phone + Password | 2FA password step | Password input dialog |
| QR Code | `grammers_client::Client::qr_login` | `qrcode.react` renders QR for mobile scan |
| Session Reuse | `SqliteSession` persistence | Automatic on startup |

### 1.3 Telegram Operations

| Operation | MTProto Function | File |
|-----------|-----------------|------|
| Create folder (channel) | `channels.CreateChannel` | `app/src-tauri/src/commands/fs.rs` |
| List folders | `channels.GetDialogs` | `app/src-tauri/src/commands/fs.rs` |
| Upload file | `messages.SendMedia` (document) | `app/src-tauri/src/commands/fs.rs` |
| Download file | `upload.GetFile` | `app/src-tauri/src/commands/fs.rs` |
| Search messages | `messages.Search` | `app/src-tauri/src/commands/fs.rs` |
| Send message | `messages.SendMessage` | `app/src-tauri/src/http_upload.rs` |

### 1.4 File Storage Model

- **Folders** = Telegram broadcast channels (named with `[TD]` suffix)
- **Files** = Telegram documents sent as messages in channels
- **Metadata** = Message ID, file name, size, MIME type extracted from message objects
- **Thumbnails** = Downloaded from Telegram's thumbnail cache

### 1.5 Proxy Support

| Proxy Type | Library Support | Configuration |
|------------|----------------|---------------|
| SOCKS5 | `grammers-client` feature `proxy` | Host, port, username, password |
| MTProto | `grammers-mtsender` feature `proxy` | Host, port, secret key |

**Key File:** `app/src-tauri/src/vpn_optimizer.rs`

---

## 2. HTTP Server (Actix-web)

### 2.1 Unified Server Architecture

A single Actix-web server hosts all route modules via `start_unified_server()` in `app/src-tauri/src/server_http.rs`.

### 2.2 Route Modules

| Module | Prefix | Purpose |
|--------|--------|---------|
| `api_routes` | `/api/v1` | REST API — health, files, folders, shares, auth |
| `admin_routes` | `/admin` | Web admin panel — login, dashboard, file management |
| `auth_routes` | `/api/v1/auth` | Authentication status and flows |
| `share_api_routes` | `/api/v1/shares` | Share link CRUD API |
| `share_routes` | `/d` | Public share download pages and streaming |
| `legacy_routes` | `/` (root) | Legacy upload, verify, config endpoints |
| `legacy_form` | `/` (root) | HTML form handlers for legacy upload |

### 2.3 Middleware Stack

Applied in order (outer to inner):

1. **SecurityHeaders** — Adds security response headers
   - `X-Content-Type-Options: nosniff`
   - `X-Frame-Options: deny`
   - `Referrer-Policy: strict-origin-when-cross-origin`
   - `Content-Security-Policy` (restrictive default)
   - `Permissions-Policy` (camera/mic/geolocation disabled)

2. **RateLimit** — Sliding-window rate limiting
   - Per-IP limit (`RATE_LIMIT_RPM`, default 120)
   - Per-API-key limit (`RATE_LIMIT_API_RPM`, default 300)
   - In-memory store (resets on restart)

3. **CORS** — Cross-origin resource sharing
   - Configurable via `CORS_ORIGINS` environment variable
   - Empty = same-origin only

4. **ShareBruteForceLimiter** — Brute-force protection for share passwords
   - Cookie-based tracking
   - 5 attempts per window (300 seconds)

**Key File:** `app/src-tauri/src/http_middleware.rs`

### 2.4 Static File Serving

| Path | Source | Purpose |
|------|--------|---------|
| `/` | `deploy/web` | Web admin SPA (static HTML/JS/CSS) |
| `/docs` | `docs/` | API documentation, OpenAPI spec |

---

## 3. Database (SQLite)

### 3.1 Driver

| Aspect | Detail |
|--------|--------|
| Crate | `sqlite = "0.37.0"` |
| Connection Type | `Arc<Mutex<sqlite::Connection>>` (synchronous, serialized) |
| Schema Management | Inline `CREATE TABLE IF NOT EXISTS` in `init_db_at()` |

### 3.2 Schema

#### `shared_links` Table

| Column | Type | Purpose |
|--------|------|---------|
| `id` | TEXT PRIMARY KEY | Share token (16-byte hex) |
| `folder_id` | INTEGER | Source Telegram channel ID |
| `message_id` | INTEGER NOT NULL | Source message ID |
| `file_name` | TEXT NOT NULL | Original file name |
| `file_size` | INTEGER DEFAULT 0 | File size in bytes |
| `password_hash` | TEXT | SHA-256(password + salt) |
| `password_salt` | TEXT | Random 16-byte hex salt |
| `expires_at` | INTEGER | Unix timestamp expiry |
| `revoked` | INTEGER DEFAULT 0 | 0=active, 1=revoked |
| `created_at` | INTEGER NOT NULL | Unix timestamp creation |

#### `upload_sessions` Table

| Column | Type | Purpose |
|--------|------|---------|
| `session_id` | TEXT PRIMARY KEY | UUID for upload session |
| `filename` | TEXT NOT NULL | Original file name |
| `total_chunks` | INTEGER NOT NULL | Expected chunk count |
| `status` | TEXT DEFAULT 'active' | active / completed |
| `manifest_file_id` | TEXT | Telegram message ID of manifest |
| `created_at` | INTEGER NOT NULL | Unix timestamp |
| `expires_at` | INTEGER NOT NULL | Auto-cleanup timestamp (7 days) |

#### `upload_chunks` Table

| Column | Type | Purpose |
|--------|------|---------|
| `session_id` | TEXT | FK to upload_sessions |
| `chunk_index` | INTEGER | 0-based chunk index |
| `file_id` | TEXT | Telegram message ID for chunk |
| `sha256` | TEXT | Chunk integrity hash |
| `status` | TEXT DEFAULT 'pending' | pending / uploaded |
| `created_at` | INTEGER NOT NULL | Unix timestamp |
| PRIMARY KEY | (session_id, chunk_index) | Composite key |

### 3.3 Indexes

- `idx_shares_expires` on `shared_links(expires_at)`
- `idx_shares_revoked` on `shared_links(revoked, created_at)`
- `idx_upload_session` on `upload_chunks(session_id)`

**Key File:** `app/src-tauri/src/db.rs`

---

## 4. Storage Backend Architecture

```
+-------------------+        +-------------------+
|   React Frontend  |        |   External Clients |
|  (Tauri WebView)  |        |   (API consumers)  |
+---------+---------+        +---------+---------+
          |                            |
          v                            v
+-------------------+        +-------------------+
|  Tauri JS Bridge  |        |   HTTP REST API   |
|  (invoke commands)|        |   (Actix-web)     |
+---------+---------+        +---------+---------+
          |                            |
          +------------+---------------+
                       |
                       v
          +------------------------+
          |    Rust Backend Core    |
          |  (TelegramState, etc.)  |
          +------------+------------+
                       |
          +------------+------------+
          |                         |
          v                         v
+-------------------+    +-------------------+
|  Telegram Cloud   |    |   SQLite (local)  |
|  (MTProto/grammers)|   |  (metadata, shares)|
+-------------------+    +-------------------+
```

### 4.1 Storage Layers

| Layer | Technology | Data |
|-------|-----------|------|
| File Content | Telegram Cloud (via MTProto) | Actual file bytes stored as Telegram documents |
| Metadata | SQLite | Share links, upload sessions, chunk manifests |
| Session | SQLite (`grammers-session`) | Telegram MTProto session (auth keys, server salts) |
| App Settings | JSON (Tauri plugin-store) | User preferences, API settings, window state |
| Network Config | JSON (`network_settings.json`) | Proxy/VPN configuration |
| Temp Files | Local filesystem | Upload/download staging, ZIP archives |
| Cache | In-memory HashMap | Peer cache, cancelled transfers |

---

## 5. Share Link System

### 5.1 Public Sharing Flow

1. User creates share via API — specifies file, optional password, expiry
2. System generates 16-byte hex token (`generate_share_token()`)
3. Password (if provided) hashed with SHA-256 + random salt
4. Share record stored in SQLite `shared_links` table
5. Public URL: `{base_url}/d/{token}`

### 5.2 Access Control

| Mechanism | Implementation |
|-----------|---------------|
| Password Verification | SHA-256 comparison with constant-time check |
| Brute-Force Protection | Cookie-based attempt tracking (5 tries / 5 min window) |
| Expiration | Unix timestamp comparison at access time |
| Revocation | `revoked` flag in database |
| Token Streaming | Separate stream token for media delivery |

### 5.3 Share Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/d/{token}` | GET | Share landing page (HTML) |
| `/d/{token}/verify` | POST | Password verification |
| `/stream/{folder_id}/{message_id}` | GET | Direct file streaming (token-authenticated) |

**Key Files:**
- `app/src-tauri/src/sharing_core.rs` — Core share logic
- `app/src-tauri/src/share_routes.rs` — Public share HTTP routes
- `app/src-tauri/src/share_api_routes.rs` — Share management API

---

## 6. VPN & Network Optimization

### 6.1 Configuration

Managed via `NetworkConfig` struct in `app/src-tauri/src/vpn_optimizer.rs`:

| Setting | Range | Default | Purpose |
|---------|-------|---------|---------|
| `timeout_multiplier` | 1-5 | 3 | Multiplies base timeouts when VPN on |
| `retry_attempts` | 0-5 | 3 | Max retry attempts for failed operations |
| `retry_base_backoff_ms` | 500-5000 | 1000 | Initial retry delay |
| `retry_max_backoff_ms` | 8000-60000 | 30000 | Max retry delay |
| `preferred_dc` | auto/dc1-dc5 | auto | Preferred Telegram datacenter |
| `dc_fallback_attempts` | 1-4 | 2 | DC fallback retry count |
| `chunk_size_kb` | 128/256/512 | 512 | Transfer chunk size |
| `bandwidth_limit_up_kbs` | 0+ | 0 | Upload speed limit (0=unlimited) |
| `bandwidth_limit_down_kbs` | 0+ | 0 | Download speed limit (0=unlimited) |
| `keep_alive_interval_sec` | 0/30-120 | 0 | Connection keep-alive interval |

### 6.2 Retry Logic

- Exponential backoff with 25% jitter
- `FLOOD_WAIT` errors sleep then retry (not counting against retry budget)
- VPN-off mode: zero retries, fast failure
- VPN-on mode: full retry stack with configurable parameters

### 6.3 Bandwidth Management

- `BandwidthManager` tracks upload/download bytes
- Configurable per-direction speed limits
- Throttling applied at chunk level

**Key File:** `app/src-tauri/src/vpn_optimizer.rs`

---

## 7. Auto-Updater

| Aspect | Detail |
|--------|--------|
| Plugin | `tauri-plugin-updater` v2.9.0 |
| Endpoint | GitHub Releases (`latest.json`) |
| Signing | Minisign public key embedded in `tauri.conf.json` |
| Private Key | `TAURI_SIGNING_PRIVATE_KEY` environment variable |
| Relaunch | `tauri-plugin-process` v2.3.1 for app restart |

**Key File:** `app/src-tauri/tauri.conf.json` (updater configuration)

---

## 8. CI/CD Integration

### 8.1 GitHub Actions

**Workflow:** `.github/workflows/docker-api.yml`

| Job | Runner | Steps |
|-----|--------|-------|
| `rust-test` | ubuntu-22.04 | Checkout, Rust toolchain, cache, `cargo test --lib --tests` |
| `build-and-smoke` | ubuntu-22.04 | Docker build, container start, health check, bash integration tests, PowerShell integration tests |

### 8.2 Triggers

- Push to `main` branch (filtered paths)
- Pull requests to `main` (filtered paths)

### 8.3 Integration Tests

| Script | Language | Purpose |
|--------|----------|---------|
| `tests/integration/test-api.sh` | Bash | API endpoint validation |
| `tests/integration/test-api.ps1` | PowerShell | Cross-platform API validation |

---

## 9. Docker Integration

### 9.1 Multi-Stage Build

See `Dockerfile` for full stage pipeline. Key stages:

| Stage | Output | Use Case |
|-------|--------|----------|
| `runtime` | Stripped binary + static files | Production deployment |
| `dev` | Binary + cargo-watch + source mount | Local development |

### 9.2 Runtime Configuration

| Environment Variable | Default | Purpose |
|---------------------|---------|---------|
| `TELEGRAM_API_ID` | (required) | Telegram API ID |
| `TELEGRAM_API_HASH` | (required) | Telegram API hash |
| `ACCESS_PWD` | (required) | Web admin password |
| `API_KEY` | (auto-generated) | REST API key |
| `PORT` | 1334 | HTTP server port |
| `BIND_HOST` | 0.0.0.0 | Bind address |
| `DATA_DIR` | /data | SQLite and data storage |
| `STATIC_DIR` | /app/deploy/web | Static web files |
| `BASE_URL` | (empty) | External URL for share links |
| `CORS_ORIGINS` | (empty) | Allowed CORS origins |
| `RATE_LIMIT_RPM` | 120 | IP rate limit (requests/min) |
| `RATE_LIMIT_API_RPM` | 300 | API key rate limit (requests/min) |
| `RUST_LOG` | info | Log level |
| `LOG_FORMAT` | (text) | `json` for structured logging |

### 9.3 Health Check

- Endpoint: `GET /api/v1/health`
- Docker healthcheck: `curl -fsS http://127.0.0.1:1334/api/v1/health`
- Interval: 30s (runtime), 15s (dev)
- Start period: 40s (runtime), 300s (dev)

---

## 10. OpenAPI Contract

| Aspect | Detail |
|--------|--------|
| Version | 3.0.3 |
| API Version | 2.0.0 |
| Security | `ApiKeyAuth` via `X-API-Key` header |
| File | `docs/openapi.json` |

### 10.1 Schemas

- `Health` — Status, version, telegram_connected, uptime_secs, build, ready
- `FileItem` — id, folder_id, name, size, mime_type, created_at
- `FolderItem` — id, name
- `LegacyUpload` — filename, file_id, download_url

### 10.2 Contract Testing

- `route_registry.rs` contains `IMPLEMENTED_ROUTES` (32 routes)
- Unit test `openapi_matches_implementation_exactly` validates parity
- Unit test `no_duplicate_routes` validates uniqueness

---

## 11. External Dependencies Summary

```
+--------------------------------------------------+
|                  Telegram Cloud                   |
|              (MTProto via grammers)               |
+--------------------------------------------------+
                          ^
                          | MTProto/TCP
+--------------------------------------------------+
|              Telegram Drive Backend               |
|  (Rust: Tauri + Actix-web + SQLite + grammers)   |
+--------------------------------------------------+
                          |
          +---------------+---------------+
          |                               |
          v                               v
+-------------------+         +-------------------+
|  Desktop Frontend |         |   HTTP Clients    |
|  (React + Tauri)  |         |  (API consumers)  |
+-------------------+         +-------------------+
          |                               |
          v                               v
+-------------------+         +-------------------+
|  GitHub Releases  |         |   Docker Hub      |
|  (Auto-updater)   |         |  (Container image)|
+-------------------+         +-------------------+
```

### 11.1 Integration Points

| Service | Protocol | Auth | Purpose |
|---------|----------|------|---------|
| Telegram Cloud | MTProto | Session-based | File storage, messaging |
| GitHub Releases | HTTPS | None (public) | Auto-update metadata |
| Docker Registry | HTTPS | Registry auth | Container distribution |
| SQLite | File I/O | None | Local metadata |
| Client Browsers | HTTP/1.1 | API key / Cookie | Web admin, share links |
