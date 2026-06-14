# Telegram Drive — Architecture Overview

Generated: 2026-05-28

---

## 1. Overall Architectural Pattern

Telegram Drive is a **dual-mode application** built as a **Tauri desktop app** with an optional **headless server** deployment. It follows a **layered monolith** pattern with clear separation between the frontend (React), Tauri bridge layer, and Rust backend services.

### Dual Build Targets

| Mode | Entry Point | Purpose |
|------|-------------|---------|
| Desktop App | `app/src-tauri/src/main.rs` | Full GUI via Tauri + embedded Actix servers |
| Headless Server | `app/src-tauri/src/bin/telegram-drive-server.rs` | API-only server via `headless-server` Cargo feature |

The same crate (`app_lib`) powers both modes. The `headless-server` feature gates Tauri-specific code.

---

## 2. System Layers and Responsibilities

### Layer Diagram

```
┌─────────────────────────────────────────────────────────────┐
│  Frontend (React 19 + Vite + Tailwind CSS)                  │
│  app/src/App.tsx, app/src/components/, app/src/hooks/       │
├─────────────────────────────────────────────────────────────┤
│  Tauri Bridge (IPC invoke/emit)                             │
│  @tauri-apps/api/core — commands registered in lib.rs       │
├─────────────────────────────────────────────────────────────┤
│  Rust Backend — Tauri Commands Layer                        │
│  app/src-tauri/src/commands/ — #[tauri::command] handlers   │
├─────────────────────────────────────────────────────────────┤
│  Rust Backend — HTTP Server Layer (Actix-web)               │
│  app/src-tauri/src/server.rs, api_routes.rs, etc.           │
├─────────────────────────────────────────────────────────────┤
│  Rust Backend — Core Services                               │
│  Telegram client, DB, bandwidth, VPN optimizer, sharing     │
├─────────────────────────────────────────────────────────────┤
│  External: Telegram MTProto API (via grammers-client)       │
└─────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

#### Frontend Layer (`app/src/`)
- React 19 SPA with TypeScript
- State: React Query for server state, React Context for UI state
- File operations via Tauri IPC (`invoke`)
- Media streaming via HTTP to localhost Actix server

#### Tauri Bridge Layer
- Commands registered in `lib.rs` via `tauri::generate_handler![]`
- Events emitted from Rust: `upload-progress`, `download-progress`
- State managed via Tauri’s `AppHandle::manage()`

#### Commands Layer (`app/src-tauri/src/commands/`)
- `auth.rs` — Telegram authentication (phone, QR, 2FA)
- `fs.rs` — File CRUD, upload/download, folder management
- `preview.rs` — Thumbnail/preview generation with caching
- `network.rs` — Network diagnostics, VPN detection, latency
- `streaming.rs` — Stream config exposure to frontend
- `api_settings.rs` — REST API key management
- `settings.rs` — Proxy/VPN settings application
- `sharing.rs` — Share link creation/management (Tauri commands)
- `utils.rs` — Peer resolution, temp file guards, error mapping

#### HTTP Server Layer (Actix-web)
Two separate Actix server instances run in desktop mode:

1. **Streaming Server** (`server.rs`, port `14201`)
   - Media streaming with range request support
   - Share link public downloads (`/d/{token}`)
   - CORS-restricted to Tauri origins

2. **REST API Server** (`api_routes.rs`, configurable port default `8550`)
   - Full REST API with API key auth (`X-API-Key`)
   - File listing, upload, download, search, bulk ops
   - Folder management
   - Share link CRUD

In headless mode, a **unified server** (`server_http.rs`) binds all routes on a single port.

#### Core Services Layer
- **Telegram Client** (`commands/auth.rs`) — `grammers-client` with SQLite session
- **Database** (`db.rs`) — SQLite for shares, upload sessions
- **Bandwidth Manager** (`bandwidth.rs`) — Daily transfer limits, JSON persistence
- **VPN Optimizer** (`vpn_optimizer.rs`) — Retry logic, proxy config, DC selection
- **Sharing Core** (`sharing_core.rs`) — Share token generation, password hashing

---

## 3. Data Flow Between Components

### Upload Flow
```
Frontend (file drop)
  → invoke("cmd_upload_file", { path, folder_id, transfer_id })
  → commands/fs.rs::cmd_upload_file
    → BandwidthManager::can_transfer()
    → ProgressReader wraps tokio::fs::File
    → tokio::spawn(progress reporter emits every 250ms)
    → client.upload_stream() via grammers-client
    → client.send_message() to Telegram peer
    → BandwidthManager::add_up()
    → emit("upload-progress", 100%)
```

### Download Flow
```
Frontend (click download)
  → invoke("cmd_download_file", { message_id, save_path, transfer_id })
  → commands/fs.rs::cmd_download_file
    → resolve_peer() (cached or scan dialogs)
    → client.get_messages_by_id()
    → client.iter_download() chunked stream
    → tokio::io::AsyncWriteExt::write_all() to save_path
    → emit("download-progress") every 250ms
    → BandwidthManager::add_down()
```

### Media Streaming Flow
```
Frontend <video src="http://localhost:14201/stream/{folder}/{msg}?token=...">
  → Actix server.rs::stream_media
    → Token validation (constant-time comparison)
    → resolve_peer() → client.get_messages_by_id()
    → Parse Range header for seek support
    → client.iter_download() with chunk_size/skip_chunks
    → async_stream::stream! yields web::Bytes
    → HttpResponse::PartialContent() or Ok() with streaming body
```

### REST API Flow
```
External client → GET /api/v1/files?folder_id=123
  → Actix api_routes.rs::api_list_files
    → check_auth() (X-API-Key vs SHA256 hash)
    → resolve_peer() → client.iter_messages()
    → Filter, sort, paginate in-memory
    → JSON response with sparse fieldset support
```

### Share Link Flow
```
User creates share → invoke("cmd_create_share")
  → commands/sharing.rs → SQLite INSERT shared_links
  → Returns /d/{token} URL

Public visitor → GET /d/{token}
  → share_routes.rs::get_shared_file
    → DB lookup → check expiry/revocation
    → Password? → render HTML form or validate cookie
    → Stream file from Telegram same as media streaming
```

---

## 4. Key Abstractions and Interfaces

### TelegramState
Central state struct managed by Tauri (`app.manage()`):
```rust
pub struct TelegramState {
    pub client: Arc<Mutex<Option<Client>>>,
    pub login_token: Arc<Mutex<Option<LoginToken>>>,
    pub password_token: Arc<Mutex<Option<PasswordToken>>>,
    pub api_id: Arc<Mutex<Option<i32>>>,
    pub runner_shutdown: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
    pub runner_count: Arc<AtomicU32>,
    pub peer_cache: Arc<RwLock<HashMap<i64, Peer>>>,
    pub cancelled_transfers: Arc<RwLock<HashSet<String>>>,
}
```

### NetworkConfig
VPN/proxy configuration with helper methods:
```rust
pub struct NetworkConfig {
    pub proxy: RwLock<ProxyConfig>,
    pub vpn: RwLock<VpnConfig>,
}
```
Provides: `connect_timeout_secs()`, `retry_attempts()`, `upload_limit_bytes_per_sec()`, etc.

### with_retry / with_retry_telegram
Unified retry wrapper in `vpn_optimizer.rs`:
- VPN off: zero retries, fast fail
- VPN on: configurable retries with exponential backoff + jitter
- FLOOD_WAIT respected without consuming retry budget

### DbConnection
Type alias: `Arc<Mutex<sqlite::Connection>>`
Used for: shared links, upload session tracking, chunk integrity (SHA256)

### BandwidthManager
Daily transfer limit enforcement with JSON persistence:
```rust
pub struct BandwidthManager {
    pub file_path: PathBuf,
    pub stats: tokio::sync::Mutex<BandwidthStats>,
    pub limit: u64, // 0 = unlimited
}
```

---

## 5. Entry Points

### Desktop Mode
- **`app/src-tauri/src/main.rs`** — Minimal wrapper calling `app_lib::run()`
- **`app/src-tauri/src/lib.rs`** — Full Tauri app setup:
  - Plugin initialization (opener, store, shell, dialog, fs, updater, process, window-state)
  - State management (`TelegramState`, `BandwidthManager`, `NetworkConfig`, etc.)
  - DB initialization
  - Actix streaming server spawn on dedicated thread
  - Actix API server spawn (if enabled)
  - VPN keep-alive background task
  - Invoke handler registration (28 commands)
  - Graceful shutdown on `RunEvent::Exit`

### Headless Server Mode
- **`app/src-tauri/src/bin/telegram-drive-server.rs`** — Standalone server:
  - `tokio::main` async runtime
  - `ServerConfig::from_env()` for configuration
  - `start_unified_server()` binds all routes on single port
  - No Tauri dependency

### Frontend
- **`app/src/main.tsx`** — React root render
- **`app/src/App.tsx`** — Main app with auth routing, providers

---

## 6. State Management Approach

### Rust Backend State
- **Tauri-managed state**: `TelegramState`, `BandwidthManager`, `NetworkConfig`, `DbConnection`, `StreamConfig`, server handles
- **Thread-safe wrappers**: `Arc<Mutex<T>>`, `Arc<RwLock<T>>`, `Arc<AtomicBool>`
- **Static globals**: `UPLOAD_CANCELLATIONS` (OnceLock<Mutex<HashMap<...>>>)

### Frontend State
- **React Query** (`@tanstack/react-query`) — Server state, caching, background refetch
- **React Context** — Theme, settings, confirmation dialogs, drop zone
- **Tauri Store plugin** — Persistent config (`config.json` with `api_id`)
- **Local component state** — UI transient state

### Cross-Layer Communication
- **Commands**: Frontend `invoke()` → Rust `#[tauri::command]` (request/response)
- **Events**: Rust `app_handle.emit()` → Frontend `listen()` (progress updates)

---

## 7. Concurrency and Async Patterns

### Runtime Architecture
- **Tauri async runtime**: `tauri::async_runtime::spawn` for background tasks
- **Tokio**: Full feature set, used by grammers-client and file I/O
- **Actix runtime**: Each Actix server runs on its own `std::thread` with `actix_rt::System::new()`

### Key Patterns

#### Dedicated Threads for Actix Servers
```rust
std::thread::spawn(move || {
    let sys = actix_rt::System::new();
    sys.block_on(async move {
        match server::start_server(...).await {
            Ok(server) => { *handle.lock().unwrap() = Some(server.handle()); server.await.ok(); }
            Err(e) => log::error!("..."),
        }
    });
});
```

#### Cooperative Cancellation
- Uploads: `tokio::sync::oneshot::channel` + `tokio::select!`
- Downloads: Check `cancelled_transfers` HashSet in chunk loop
- Runner shutdown: `oneshot::Sender<()>` signals grammers runner exit

#### Progress Reporting
- Time-based emission (every 250ms) to avoid event flooding
- Separate `tokio::spawn` task for upload progress
- Inline progress checks for downloads

#### Peer Caching
- `Arc<RwLock<HashMap<i64, Peer>>>` for O(1) folder resolution
- Lazily populated on first `resolve_peer()` call
- Eagerly warmed during `cmd_scan_folders()`

#### Bandwidth Throttling
- Download: Sleep calculation based on rate vs limit
- Upload: Implicit via `ProgressReader` + sleep in progress task

---

## 8. Security Architecture Overview

### Authentication
- **Telegram MTProto**: Phone + code + optional 2FA, or QR login
- **REST API**: SHA256-hashed API keys via `X-API-Key` header
- **Share links**: Optional password protection with SHA256+salt
- **Admin routes**: `X-Access-Pwd` or form-based password (constant-time compare)

### Authorization
- API endpoints require valid API key (configurable, can be disabled)
- Share links check revocation, expiry, password before serving
- Brute-force protection on share password attempts (5 attempts / 5 min window)

### Transport Security
- Stream token validation with constant-time comparison (timing attack prevention)
- CORS restricted to Tauri origins for streaming server
- Security headers middleware: `X-Content-Type-Options`, `X-Frame-Options`, CSP, etc.

### Rate Limiting
- Per-IP rate limiting (configurable RPM)
- Per-API-key rate limiting (separate bucket)
- Health endpoint exempted

### Data Protection
- API keys stored as SHA256 hashes only (plaintext returned once on generation)
- Share passwords stored as SHA256+salt
- Session file (`telegram.session`) in app data directory
- Temp file cleanup via `TempFileGuard` (RAII)

### Input Validation
- Port range validation (must be >= 1024, cannot collide with stream port)
- File size limits on uploads (100MB default for API)
- Chunk parameter bounds checking
- Path traversal prevention in temp file deletion (`canonicalize` + `starts_with` check)

---

## 9. Notable Design Decisions

### Folder-as-Channel Mapping
Telegram channels (broadcast groups) with `[TD]` in the title serve as folders. This avoids needing a separate metadata layer for folder structure.

### Two-Server Architecture
The streaming server (port 14201) is separate from the REST API server to:
- Allow media streaming without API key (token-based auth instead)
- Enable CORS restrictions specific to frontend origins
- Support independent restart of API server from settings changes

### Mock Mode
Many commands check `client_opt.is_none()` and return mock data, enabling frontend development without Telegram credentials.

### Headless Feature Gating
The `headless-server` Cargo feature removes all Tauri dependencies, allowing the same codebase to compile as a standalone server binary.
