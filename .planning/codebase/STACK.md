# Telegram Drive — Technology Stack

Generated: 2026-05-28

---

## 1. Primary Languages & Versions

| Language | Version | Usage |
|----------|---------|-------|
| Rust | 2021 Edition | Backend core, Tauri commands, HTTP server, Telegram MTProto client |
| TypeScript | ~5.8.3 | Frontend UI components, hooks, state management |
| JavaScript | ES2020 | Vite configuration, build tooling |
| SQL | SQLite dialect | Local metadata schema (shares, upload sessions) |
| Shell / PowerShell | POSIX / PowerShell 7 | CI scripts, integration tests, dev helpers |

---

## 2. Runtime Environments

| Runtime | Version | Purpose |
|---------|---------|---------|
| Tokio | 1.x (full features) | Rust async runtime — drives all async I/O, Telegram client, HTTP servers |
| Node.js | 20+ (implied) | Frontend build pipeline, Vite dev server, npm package management |
| WebView2 (Windows) / WebKitGTK (Linux) | OS-bundled | Tauri desktop shell for rendering React UI |
| Debian bookworm-slim | 12.x | Docker runtime base image |

---

## 3. Frameworks & Core Libraries

### 3.1 Backend (Rust)

| Crate | Version | Purpose |
|-------|---------|---------|
| `tauri` | 2 | Desktop app framework — JS/Rust IPC bridge, window management, event system |
| `actix-web` | 4 | HTTP server framework — REST API, file streaming, static file serving |
| `actix-cors` | 0.7 | CORS middleware for cross-origin API access |
| `actix-files` | 0.6 | Static file serving (`deploy/web`, `docs`) |
| `actix-multipart` | 0.7 | Multipart form parsing for file uploads |
| `actix-rt` | 2 | Actix runtime for async test execution |
| `grammers-client` | git@d07f96f | Telegram MTProto client — authentication, messaging, file upload/download |
| `grammers-session` | git@d07f96f | Session persistence — SQLite-backed session storage |
| `grammers-mtsender` | git@d07f96f | Low-level MTProto message sender with proxy support |
| `grammers-tl-types` | git@d07f96f | Telegram TL schema type definitions |
| `tokio` | 1 (full) | Async runtime — timers, channels, sync primitives, I/O |
| `futures` / `futures-util` | 0.3 | Stream utilities, async combinators |
| `async-stream` | 0.3 | Stream generation macros for chunked responses |

### 3.2 Frontend (TypeScript/React)

| Package | Version | Purpose |
|---------|---------|---------|
| `react` | ^19.1.0 | UI framework — functional components, hooks |
| `react-dom` | ^19.1.0 | React DOM renderer |
| `vite` | ^7.0.4 | Build tool, dev server, HMR, bundling |
| `@vitejs/plugin-react` | ^4.6.0 | Vite React plugin (Fast Refresh) |
| `tailwindcss` | ^4.1.18 | Utility-first CSS framework |
| `@tailwindcss/postcss` | ^4.1.18 | Tailwind PostCSS integration |
| `autoprefixer` | ^10.4.23 | CSS vendor prefix automation |
| `postcss` | ^8.5.6 | CSS transformation pipeline |
| `typescript` | ~5.8.3 | Static type checking |
| `@types/react` / `@types/react-dom` | ^19.1.x | React type definitions |

### 3.3 Frontend State & UI Libraries

| Package | Version | Purpose |
|---------|---------|---------|
| `@tanstack/react-query` | ^5.90.17 | Server state management — caching, background refetch, mutations |
| `@tanstack/react-virtual` | ^3.13.18 | Virtual scrolling for large file/folder lists |
| `framer-motion` | ^12.26.2 | Animation library — transitions, gestures, layout animations |
| `lucide-react` | ^0.562.0 | Icon library |
| `sonner` | ^2.0.7 | Toast notification system |
| `qrcode.react` | ^4.2.0 | QR code rendering for Telegram QR login |
| `pdfjs-dist` | ^5.6.205 | PDF preview rendering in browser |

### 3.4 Tauri Plugins

| Plugin | Version | Purpose |
|--------|---------|---------|
| `tauri-plugin-opener` | 2 | Open files/URLs in external applications |
| `tauri-plugin-store` | 2 | Key-value persistent storage (settings, preferences) |
| `tauri-plugin-window-state` | 2 | Persist window position and size across restarts |
| `tauri-plugin-shell` | 2 | Execute shell commands from Rust |
| `tauri-plugin-dialog` | 2.6.0 | Native file picker dialogs |
| `tauri-plugin-fs` | 2 | File system access API |
| `tauri-plugin-updater` | 2.9.0 | Auto-updater — check, download, install updates |
| `tauri-plugin-process` | 2.3.1 | Process relaunch after update installation |

---

## 4. Data, Storage & Cryptography

### 4.1 Storage Stack

| Layer | Technology | Crate/Tool | Purpose |
|-------|-----------|------------|---------|
| File Content | Telegram Cloud | `grammers-client` | Actual file storage via MTProto |
| Local Metadata | SQLite | `sqlite = "0.37.0"` | Share links, upload sessions, chunk tracking |
| Session Storage | SQLite | `grammers-session` | Telegram MTProto session persistence |
| App Settings | JSON files | `tauri-plugin-store` | User preferences, window state |
| Network Config | JSON files | `std::fs` (via `vpn_optimizer.rs`) | Proxy/VPN settings persistence |
| Temporary Files | Local filesystem | `std::fs` / `tempfile` | Upload/download staging |
| In-Memory Cache | HashMap | `std::collections` | Peer cache, cancelled transfer tracking |

### 4.2 Cryptography & Security Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| `sha2` | 0.10 | SHA-256 hashing — API key hashing, share password hashing, chunk integrity |
| `base64` | 0.21 | Base64 encoding/decoding — URL-safe tokens, data URLs |
| `rand` | 0.8 | Cryptographically secure random generation — tokens, salts, stream tokens |
| `uuid` | 1 (v4) | UUID generation for share links |
| `zip` | 2 (deflate only) | ZIP archive creation for bulk downloads |

> **Security Note:** Current password hashing uses single-round SHA-256 with a random salt. This is not a proper key derivation function (KDF). See `CONCERNS.md` for details.

---

## 5. Build Tools & Package Managers

| Tool | Role | Configuration |
|------|------|---------------|
| **Cargo** | Rust package manager, build tool, test runner | `app/src-tauri/Cargo.toml`, `Cargo.lock` |
| **npm** | Node.js package manager | `app/package.json`, `package-lock.json` |
| **Vite** | Frontend bundler, dev server, HMR | `app/vite.config.ts` |
| **Tauri CLI** | Desktop app bundling, code generation | `@tauri-apps/cli` (dev dependency) |
| `tauri-build` | Rust build dependency for Tauri codegen | `app/src-tauri/build.rs` |
| **Docker** | Containerization for headless server | `Dockerfile`, `docker-compose.yml` |
| **cargo-chef** | 0.1.68 | Docker layer caching for Rust dependencies |
| **cargo-watch** | latest | Auto-rebuild during Docker development |

---

## 6. Configuration Files

| File | Purpose |
|------|---------|
| `app/src-tauri/Cargo.toml` | Rust dependencies, features, binary targets, crate metadata |
| `app/src-tauri/Cargo.lock` | Pinned dependency versions for reproducible builds |
| `app/src-tauri/tauri.conf.json` | Tauri app config — window, CSP, updater, bundle, security |
| `app/src-tauri/build.rs` | Tauri build script — code generation at compile time |
| `app/package.json` | Frontend dependencies, scripts, project metadata |
| `app/vite.config.ts` | Vite build config — plugins, dev server, HMR settings |
| `app/tsconfig.json` | TypeScript compiler options — strict mode, ES2020 target |
| `app/tsconfig.node.json` | TypeScript config for Node tooling files |
| `app/postcss.config.js` | PostCSS pipeline — Tailwind, autoprefixer |
| `.env` / `.env.example` | Environment variables — secrets, API keys, server config |
| `Dockerfile` | Multi-stage container build — dev, builder, runtime stages |
| `docker-compose.yml` | Production Docker Compose — ports, volumes, env |
| `docker-compose.dev.yml` | Development Docker Compose — volume mounts, cargo-watch |

---

## 7. Build Features & Binary Targets

### 7.1 Cargo Features

```toml
[features]
default = ["desktop"]
desktop = []
headless-server = []
```

| Feature | Description |
|---------|-------------|
| `desktop` (default) | Full Tauri desktop app with WebView GUI, window management, native dialogs |
| `headless-server` | API-only server without Tauri/WebView — for Docker deployment, CI testing |

### 7.2 Binary Targets

| Binary | Source | Required Features | Purpose |
|--------|--------|-------------------|---------|
| `app` | `src/main.rs` | `desktop` | Tauri desktop application entry point |
| `telegram-drive-server` | `src/bin/telegram-drive-server.rs` | `headless-server` | Headless HTTP server entry point |

### 7.3 Library Target

| Library | Crate Type | Purpose |
|---------|-----------|---------|
| `app_lib` | `staticlib`, `cdylib`, `rlib` | Shared library for Tauri FFI and integration tests |

---

## 8. Special Technology Choices & Architectural Patterns

### 8.1 Dual-Mode Architecture

The codebase supports two distinct deployment modes via Cargo features:

- **Desktop Mode**: Tauri wraps a React frontend; Rust provides native APIs for Telegram, filesystem, and window management.
- **Headless Server Mode**: Pure Actix-web server with no GUI; serves REST API and static files; intended for Docker/cloud deployment.

This is achieved through `#[cfg(not(feature = "headless-server"))]` and `#[cfg(feature = "headless-server")]` conditional compilation throughout the Rust codebase.

### 8.2 Dual-Server HTTP Architecture

| Server | Default Port | Purpose |
|--------|-------------|---------|
| API Server | 1334 (无头/Docker) | REST API, admin routes, share routes, legacy upload |
| Streaming Server | 14201 | Media streaming, file download, thumbnail delivery |

The streaming server runs on a fixed port (`STREAM_PORT = 14201`) to simplify frontend integration. The API server port is configurable.

### 8.3 Telegram MTProto Integration

- Uses `grammers` ecosystem (not `teloxide` or official Bot API) for full user-account access
- Git dependency pinned to specific revision (`d07f96f`) for reproducibility
- Supports SOCKS5 and MTProto proxies via `grammers-client` feature flags
- Session persistence via `grammers-session` SQLite storage

### 8.4 Resumable Chunked Uploads

- Files split into configurable chunks (default 10MB)
- Each chunk uploaded as a separate Telegram document
- SHA-256 checksums verify chunk integrity
- SQLite tracks upload session state (`upload_sessions`, `upload_chunks` tables)
- Supports resume after interruption

### 8.5 VPN-Aware Network Layer

- `vpn_optimizer.rs` provides centralized network configuration
- Configurable: proxy settings, timeout multipliers, retry backoff, DC fallback, bandwidth limits
- Retry wrapper with exponential backoff + jitter
- Special handling for Telegram `FLOOD_WAIT` errors

### 8.6 Security Middleware Stack

Layered Actix middleware (outer to inner):
1. `SecurityHeaders` — HSTS, X-Frame-Options, CSP, Referrer-Policy, Permissions-Policy
2. `RateLimit` — Sliding-window rate limiter (per-IP + per-API-key)
3. `CORS` — Configurable cross-origin origins
4. `ShareBruteForceLimiter` — Cookie-based brute-force protection for share links

### 8.7 OpenAPI Contract Testing

- `docs/openapi.json` defines OpenAPI 3.0.3 contract
- `route_registry.rs` maintains canonical route list (`IMPLEMENTED_ROUTES`)
- Unit test `openapi_matches_implementation_exactly` verifies parity between code and spec
- 32 routes tracked across API v1, legacy, and share endpoints

### 8.8 Structured JSON Logging

- `env_logger` with custom formatter
- `LOG_FORMAT=json` environment variable enables one-JSON-object-per-line output
- Includes timestamp (RFC 3339), level, target, and message fields

### 8.9 Docker Multi-Stage Build

| Stage | Purpose |
|-------|---------|
| `base` | Rust toolchain + system dependencies |
| `chef-bin` | Install `cargo-chef` for dependency caching |
| `planner` | Generate `recipe.json` from Cargo.toml/Cargo.lock |
| `deps` | Pre-compile dependencies (layer cached) |
| `builder` | Compile application binary, strip symbols |
| `dev` | Development image with `cargo-watch` for auto-rebuild |
| `runtime` | Minimal Debian image with compiled binary only |

BuildKit cache mounts used for Cargo registry, git, and target directories.

---

## 9. Deployment Targets

| Target | Method | Artifacts |
|--------|--------|-----------|
| Desktop (Windows) | Tauri bundler | `.msi`, `.exe` installer |
| Desktop (macOS) | Tauri bundler | `.app`, `.dmg` |
| Desktop (Linux) | Tauri bundler | `.AppImage`, `.deb` |
| Headless Server | Docker | `telegram-drive-api` image |
| CI/CD | GitHub Actions | Docker image push, cross-platform builds |

---

## 10. Version Information

| Component | Version |
|-----------|---------|
| Application (Cargo) | 2.0.0 |
| Application (Tauri config) | 1.6.8 |
| Frontend (package.json) | 2.0.0 |
| API (OpenAPI) | 2.0.0 |
| Rust Edition | 2021 |
| Docker Rust Base | 1.85-bookworm |
| Docker Runtime Base | bookworm-slim (Debian 12) |
