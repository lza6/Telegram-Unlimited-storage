# Telegram Drive — Directory Structure

Generated: 2026-05-28

---

## 1. Top-Level Directory Layout

```
Telegram-Drive/
├── app/                    # Main application (Tauri + React frontend)
├── data/                   # Runtime data directory (SQLite DB, settings)
├── deploy/                 # Static web assets for headless server
├── docs/                   # Documentation and OpenAPI spec
├── scripts/                # Build and development scripts
├── tests/                  # Integration tests
├── screenshots/            # UI screenshots for documentation
├── .github/                # GitHub Actions workflows
├── .planning/              # Planning documents (this directory)
├── Dockerfile              # Multi-stage Docker build
├── docker-compose.yml      # Production Docker Compose
├── docker-compose.dev.yml  # Development Docker Compose
├── README.md               # Project readme
├── README-DOCKER.md        # Docker-specific documentation
├── CHANGELOG.md            # Version changelog
├── .env                    # Environment variables (gitignored)
├── .env.example            # Environment variable template
├── build.bat               # Windows build script
├── dev.bat                 # Windows dev script
├── setup.bat               # Windows setup script
├── start.bat               # Windows start script
└── skills-lock.json        # Claude skills lock file
```

---

## 2. Application Directory (`app/`)

### Overview
The `app/` directory contains the entire application: a React frontend and a Tauri/Rust backend.

```
app/
├── src/                    # React frontend source
├── src-tauri/              # Rust backend source
├── public/                 # Static assets (logo.svg)
├── index.html              # HTML entry point
├── package.json            # Node.js dependencies
├── package-lock.json       # Locked dependency versions
├── tsconfig.json           # TypeScript config
├── tsconfig.node.json      # TypeScript config for Vite
├── vite.config.ts          # Vite build configuration
├── postcss.config.js       # PostCSS config (Tailwind)
├── README.md               # App-specific readme
└── test_upload.txt         # Test file
```

---

## 3. Frontend Source (`app/src/`)

### Directory Layout

```
app/src/
├── main.tsx                # React application entry point
├── App.tsx                 # Root component with auth routing
├── App.css                 # Global application styles
├── types.ts                # Shared TypeScript interfaces
├── utils.ts                # Utility functions
├── vite-env.d.ts           # Vite environment types
├── assets/                 # Static image assets
│   ├── logo.svg
│   └── react.svg
├── components/             # React components
│   ├── AuthWizard.tsx      # Authentication flow UI
│   ├── Dashboard.tsx       # Main file manager dashboard
│   ├── ErrorBoundary.tsx   # Error boundary component
│   ├── FileTypeIcon.tsx    # File icon component
│   ├── ThemeToggle.tsx     # Dark/light mode toggle
│   ├── UpdateBanner.tsx    # Auto-update notification
│   └── dashboard/          # Dashboard sub-components
│       ├── BandwidthWidget.tsx
│       ├── ContextMenu.tsx
│       ├── DownloadQueue.tsx
│       ├── DragDropOverlay.tsx
│       ├── EmptyState.tsx
│       ├── ExternalDropBlocker.tsx
│       ├── FileCard.tsx
│       ├── FileExplorer.tsx
│       ├── FileListItem.tsx
│       ├── MediaPlayer.tsx
│       ├── MoveToFolderModal.tsx
│       ├── PdfViewer.tsx
│       ├── PreviewModal.tsx
│       ├── SettingsModal.tsx
│       ├── ShareDialog.tsx
│       ├── Sidebar.tsx
│       ├── SidebarItem.tsx
│       ├── TopBar.tsx
│       └── UploadQueue.tsx
├── context/                # React Context providers (legacy naming)
│   ├── ConfirmContext.tsx
│   ├── SettingsContext.tsx
│   └── ThemeContext.tsx
├── contexts/               # React Context providers
│   └── DropZoneContext.tsx
└── hooks/                  # Custom React hooks
    ├── useFileDownload.ts
    ├── useFileDrop.ts
    ├── useFileOperations.ts
    ├── useFileUpload.ts
    ├── useKeyboardShortcuts.ts
    ├── useNetworkStatus.ts
    ├── useTelegramConnection.ts
    └── useUpdateCheck.ts
```

### Naming Conventions (Frontend)
- **Components**: `PascalCase.tsx` (e.g., `AuthWizard.tsx`, `FileCard.tsx`)
- **Hooks**: `camelCase` with `use` prefix (e.g., `useFileUpload.ts`)
- **Contexts**: `PascalCase` with `Context` suffix (e.g., `ThemeContext.tsx`)
- **Types**: `PascalCase` interfaces in `types.ts`
- **Styles**: Component-scoped where possible, global in `App.css`

---

## 4. Rust Backend Source (`app/src-tauri/src/`)

### Directory Layout

```
app/src-tauri/src/
├── main.rs                 # Desktop binary entry point (calls app_lib::run)
├── lib.rs                  # Library root — Tauri app setup, state, server spawning
├── models.rs               # Shared data structs (AuthState, FileMetadata, etc.)
├── db.rs                   # SQLite database layer
├── bandwidth.rs            # Bandwidth tracking and limits
├── vpn_optimizer.rs        # Network config, retry logic, proxy/VPN settings
├── server.rs               # Actix streaming server (port 14201)
├── server_http.rs          # Unified Actix server for headless mode
├── server_config.rs        # Server configuration from environment
├── server_uptime.rs        # Server uptime tracking
├── logging.rs              # env_logger initialization with JSON format support
├── http_middleware.rs      # Actix middleware: CORS, rate limiting, security headers
├── http_download.rs        # Download stream builders (message + manifest)
├── http_upload.rs          # Upload helpers for HTTP routes
├── route_registry.rs       # Canonical route list + OpenAPI contract tests
├── sharing_core.rs         # Share link core logic (token gen, password hash)
├── legacy_form.rs          # Form parsing for legacy routes
├── legacy_routes.rs        # Legacy chunk upload/download routes
├── api_routes.rs           # REST API v1 routes
├── auth_routes.rs          # REST API auth routes
├── admin_routes.rs         # Admin/legacy upload routes
├── share_routes.rs         # Public share link download routes
├── share_api_routes.rs     # REST API share management routes
├── commands/               # Tauri command handlers
│   ├── mod.rs              # Module re-exports + TelegramState definition
│   ├── auth.rs             # Authentication commands
│   ├── fs.rs               # File system commands (upload, download, CRUD)
│   ├── preview.rs          # Preview/thumbnail commands
│   ├── utils.rs            # Utility commands (logging, bandwidth, peer resolve)
│   ├── network.rs          # Network diagnostic commands
│   ├── streaming.rs        # Streaming config commands
│   ├── api_settings.rs     # API settings management
│   ├── settings.rs         # Proxy/VPN settings commands
│   └── sharing.rs          # Share link Tauri commands
└── bin/
    └── telegram-drive-server.rs  # Headless server binary entry point
```

### Module Organization

#### Entry Points
| File | Purpose |
|------|---------|
| `main.rs` | Desktop binary — minimal wrapper, sets Linux env var, calls `app_lib::run()` |
| `lib.rs` | Library root — Tauri builder setup, state init, server spawning, command registration |
| `bin/telegram-drive-server.rs` | Headless server — unified Actix server, no Tauri |

#### Core Data & Infrastructure
| File | Purpose |
|------|---------|
| `models.rs` | Serde-deriving structs: `AuthState`, `AuthResult`, `FileMetadata`, `FolderMetadata`, `Drive` |
| `db.rs` | SQLite operations: `shared_links` table, `upload_sessions`/`upload_chunks` tables |
| `bandwidth.rs` | Daily transfer stats with JSON persistence, limit enforcement |
| `vpn_optimizer.rs` | `NetworkConfig`, `ProxyConfig`, `VpnConfig`, retry wrappers, backoff logic |
| `logging.rs` | `env_logger` setup with optional JSON formatting via `LOG_FORMAT=json` |
| `server_uptime.rs` | Static `OnceLock<Instant>` for uptime tracking |

#### HTTP Server Layer
| File | Purpose |
|------|---------|
| `server.rs` | Streaming server: media streaming with Range support, share routes mount |
| `server_http.rs` | Unified server for headless mode: all routes + static files on single port |
| `server_config.rs` | `ServerConfig::from_env()` — env var parsing with defaults |
| `http_middleware.rs` | `SecurityHeaders`, `RateLimiter`, `RateLimit`, `ShareBruteForceLimiter`, `build_cors` |
| `http_download.rs` | `download_message_stream()`, `download_manifest_stream()` for chunked downloads |
| `http_upload.rs` | `upload_file_path()`, `upload_bytes_with_caption()`, `upload_text_file()` |
| `route_registry.rs` | `IMPLEMENTED_ROUTES` const + OpenAPI contract validation tests |

#### Route Modules (Actix-web handlers)
| File | Route Prefix | Purpose |
|------|-------------|---------|
| `api_routes.rs` | `/api/v1/*` | REST API: health, files, folders, upload, download, bulk, search |
| `auth_routes.rs` | `/api/v1/auth/*` | REST API auth: status, phone, QR |
| `share_api_routes.rs` | `/api/v1/shares` | REST API share CRUD |
| `share_routes.rs` | `/d/{token}` | Public share downloads with password protection |
| `admin_routes.rs` | `/verify`, `/config`, `/upload` | Legacy/admin endpoints |
| `legacy_routes.rs` | `/upload_chunk`, `/upload_status`, `/merge_chunks`, `/d` | Chunked upload legacy API |

#### Command Modules (Tauri IPC)
| File | Commands |
|------|----------|
| `commands/mod.rs` | `TelegramState` struct definition, module re-exports |
| `commands/auth.rs` | `cmd_connect`, `cmd_check_connection`, `cmd_logout`, `cmd_auth_request_code`, `cmd_auth_sign_in`, `cmd_auth_check_password`, `cmd_auth_qr_login`, `cmd_auth_qr_poll` |
| `commands/fs.rs` | `cmd_create_folder`, `cmd_delete_folder`, `cmd_upload_file`, `cmd_delete_file`, `cmd_download_file`, `cmd_move_files`, `cmd_get_files`, `cmd_search_global`, `cmd_scan_folders`, `cmd_zip_folder`, `cmd_delete_temp_zip`, `cmd_cancel_transfer` |
| `commands/preview.rs` | `cmd_get_preview`, `cmd_clean_cache`, `cmd_get_thumbnail`, `cmd_delete_image_thumbnail` |
| `commands/utils.rs` | `resolve_peer`, `clear_peer_cache`, `cmd_log`, `cmd_get_bandwidth`, `TempFileGuard`, `map_error` |
| `commands/network.rs` | `cmd_is_network_available`, `cmd_check_latency`, `cmd_detect_vpn` |
| `commands/streaming.rs` | `cmd_get_stream_info` |
| `commands/api_settings.rs` | `cmd_get_api_settings`, `cmd_update_api_settings`, `cmd_regenerate_api_key` |
| `commands/settings.rs` | `cmd_apply_proxy_settings`, `cmd_apply_vpn_settings`, `cmd_get_network_config` |
| `commands/sharing.rs` | `cmd_create_share`, `cmd_list_shares`, `cmd_revoke_share` |

#### Supporting Modules
| File | Purpose |
|------|---------|
| `sharing_core.rs` | Share token generation, SHA256 password hashing, link building |
| `legacy_form.rs` | Form body parsing for legacy non-JSON endpoints |

### Naming Conventions (Rust)
- **Modules**: `snake_case.rs` (e.g., `api_routes.rs`, `vpn_optimizer.rs`)
- **Structs/Enums**: `PascalCase` (e.g., `TelegramState`, `NetworkConfig`)
- **Functions**: `snake_case` (e.g., `cmd_upload_file`, `resolve_peer`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `STREAM_PORT`, `DC_ADDRESSES`)
- **Tauri commands**: `cmd_` prefix (e.g., `cmd_get_files`)
- **Actix handlers**: No strict prefix, often `api_*` or descriptive names

---

## 5. Configuration File Locations

### Rust / Cargo
| File | Purpose |
|------|---------|
| `app/src-tauri/Cargo.toml` | Crate manifest — dependencies, features (`desktop`/`headless-server`), bins |
| `app/src-tauri/Cargo.lock` | Locked dependency versions |
| `app/src-tauri/build.rs` | Tauri build script |
| `app/src-tauri/tauri.conf.json` | Tauri app config — window, CSP, updater, bundle |
| `app/src-tauri/.cargo/config.toml` | Cargo configuration |

### Frontend
| File | Purpose |
|------|---------|
| `app/package.json` | Node dependencies — React 19, Vite, Tailwind, Tauri APIs |
| `app/tsconfig.json` | TypeScript compiler options |
| `app/vite.config.ts` | Vite bundler configuration |
| `app/postcss.config.js` | PostCSS with Tailwind plugin |

### Runtime / Environment
| File | Purpose |
|------|---------|
| `.env` | Environment variables (gitignored) |
| `.env.example` | Template showing required env vars |
| `data/api_settings.json` | Persisted API server settings |
| `data/shares.db` | SQLite database for share links and upload sessions |
| `data/bandwidth.json` | Daily bandwidth statistics |
| `data/network_settings.json` | Persisted proxy/VPN configuration |
| `data/telegram.session` | grammers-client SQLite session file |

---

## 6. Build Artifact Locations

### Desktop Build
| Location | Contents |
|----------|----------|
| `app/src-tauri/target/` | Cargo build artifacts (debug/release binaries) |
| `app/dist/` | Vite frontend build output (consumed by Tauri) |
| `app/src-tauri/target/release/app.exe` | Desktop binary (Windows) |

### Headless Server Build
| Location | Contents |
|----------|----------|
| `app/src-tauri/target/release/telegram-drive-server` | Headless server binary |

### Docker Build
| Stage | Output |
|-------|--------|
| `builder` | Compiled `telegram-drive-server` binary |
| `runtime` | Minimal Debian image with binary + static assets |

---

## 7. Test Locations

```
app/src-tauri/tests/
└── health_api.rs           # Integration test for health endpoint

tests/
└── integration/
    ├── test-api.ps1        # PowerShell API integration tests
    └── test-api.sh         # Bash API integration tests
```

Unit tests are embedded in source files using `#[cfg(test)] mod tests { ... }`:
- `api_routes.rs` — Auth check tests
- `http_middleware.rs` — Rate limiter tests
- `server_config.rs` — CORS parsing tests
- `commands/api_settings.rs` — Hash/verify roundtrip tests
- `route_registry.rs` — OpenAPI contract tests

---

## 8. Static Assets and Deployment

```
deploy/web/                 # Static files served by headless server
├── index.html              # Web UI entry
├── login.html              # Login page
├── dashboard.html          # Dashboard page
├── upload.html             # Upload page
├── telegram.html           # Telegram connection page
└── docs.html               # Documentation page

docs/
├── openapi.json            # OpenAPI 3.0 specification
└── planning/               # Project planning documents
    ├── README.md
    ├── 下一步改进指南.md
    ├── 代码库规格.md
    └── 运维Runbook.md
```

---

## 9. Scripts Directory

```
scripts/
├── build-api.bat           # Build API server
├── dev-build-api.ps1       # Dev build for API
├── dev-build-rust.ps1      # Dev build for Rust
├── dev-reload.bat          # Dev reload script
├── dev-sync-web.ps1        # Sync web assets
├── dev-up.ps1              # Start dev environment
├── dev-update.ps1          # Update dev dependencies
├── setup-local.bat         # Local setup
├── start-api.bat           # Start API server
├── wait-and-open-browser.ps1
├── _common.bat             # Common script utilities
├── _docker-dev.ps1         # Docker dev helpers
├── _load-env.bat           # Environment loader
└── _log.bat                # Logging utilities
```

---

## 10. CI/CD Configuration

```
.github/
├── FUNDING.yml
└── workflows/
    ├── main.yml            # Main CI workflow
    ├── release.yml         # Release workflow
    └── docker-api.yml      # Docker API build/publish
```
