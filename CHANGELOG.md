# Changelog

## [1.6.8] - 2026-05-25

### Features & Fixes

- **In-App Update Permission Fix** — Granted the `"process:allow-restart"` capability permission in `src-tauri/capabilities/default.json` to allow the frontend updater to safely relaunch the app after installing an update.

---

## [1.6.7] - 2026-05-23

### Features & Fixes

- **Windows Build & Git Checkout Fix** — Untracked and ignored `app/.npm-cache` files from Git to fix "Filename too long" checkouts and build errors on Windows platforms.
- **Tauri signing key security** — Replaced updater signing key with a password-protected keypair and restored the secret password integration in the CI pipeline.

---

## [1.6.6] - 2026-05-22

### Features & Fixes

- **Tauri Updater Integration & Dedicated UI** — Fully integrated and resolved production updater configurations.
  - **Updater Build Artifacts**: Set `createUpdaterArtifacts` to `true` in `tauri.conf.json` to generate signing signatures (`.sig`) and the `latest.json` manifest dynamically during production builds.
  - **In-App Update Interface**: Added a native "Check for Updates" control panel within the General Settings tab, complete with a visual download progress bar, status toasts, and automatic "Update & Restart" integration.
  - **Promise Safety**: Handled fire-and-forget background update-check Promises by appending explicit `.catch` error logging to prevent unhandled rejection behaviors.
  - **Automated Workflow Releases**: Enhanced the GitHub Release CI workflow to automatically parse and extract only the latest release notes from `CHANGELOG.md` dynamically using `awk`.

---

## [1.6.5] - 2026-05-21

### Features & Enhancements

- **REST API Enhancements (Actix-web & Rust)** — Fully implemented the comprehensive REST API extension in Rust/Actix-web with backwards-compatible response structures.
  - **Refined Folder Navigation**: Resolved `folder_id` query handling into three deterministic query states: all files when omitted, root-only when `?folder_id=`, and subfolder files when filtering specifically by a folder ID.
  - **Standardized Pagination Envelope**: Wrapped collections in a clean payload format featuring a `data` array, `pagination` metrics (`page`, `limit`, `total_items`, `total_pages`), and a `filters` echo block.
  - **Advanced Query Parameters**: Introduced server-side sorting (`sort_by`, `sort_order`) and robust filters for MIME type, file size bounds, and creation date ranges.
  - **Sparse Fieldsets**: Added a `?fields=` selector enabling clients to request specific metadata subsets to reduce bandwidth overhead.
  - **Bulk Operations & Global Search**: Added `POST /api/v1/files/bulk` for batch moves and deletes, and `GET /api/v1/files/search` supporting the full pagination envelope.
  - **Rate Limiting Integration**: Injected simulated API rate-limit headers (`X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`) to standard responses.

---

## [1.6.0] - 2026-05-21

### Features & Fixes

- **"Copy Telegram Link" Feature** — Added a right-click context menu option to copy raw `t.me` message links for files in public channels (`https://t.me/{username}/{message_id}`). If the channel is private, the item displays in a disabled state with a descriptive tooltip.
- **Tauri 2 Tokio Runtime Panic Fix** — Fixed the `there is no reactor running` panic caused by `tokio::task::spawn_blocking` executing outside of a Tokio runtime context within the Bandwidth Manager. Replaced the asynchronous task with a lightweight, synchronous write, resolving the panic completely.

---

## [1.5.0] - 2026-05-19

### Feature

- **VPN Optimizer & Proxy Configuration** — Added robust support for toggling VPN mode to optimize network connection timeouts, retry limits, backoff delays, adaptive polling, flood wait handling, and peer caches. Fully integrated proxy configuration (SOCKS5 and MTProto) to allow custom routing and bypass geo-blocks.

---

## [1.4.2] - 2026-05-18

### Feature

- **Folder Upload with Automatic Zipping** — Support uploading entire folders directly, automatically compressing them into highly-optimized zip archives before transfer.

---

## [1.1.7] - 2026-05-01

### Feature

- Added a donation button and popup modal to the main login screen to support the project via PayPal, Litecoin, and Bitcoin.

---

## [1.1.6] - 2026-04-28

### Fix

- Fixed process not terminating on Ctrl+C (SIGINT) when launched from a terminal.
  The Actix-web streaming server and grammers network runner were running on
  non-daemon threads with no shutdown signal wired to process exit, causing the
  application to hang indefinitely after the main window closed. The app now
  registers a RunEvent::Exit handler that gracefully stops both background
  services before the process exits.

---

## [1.1.5] - 2026-04-27

### Hotfix

- **CI fix: AppImage patch step now runs cleanly** — Replaced the fragile `grep -oP` Perl lookahead (which exited with code 2 under `set -euo pipefail`) with a safe `awk`-based `.desktop` file lookup. Added `APPIMAGE_EXTRACT_AND_RUN=1` so `appimagetool` doesn't require the FUSE kernel module on GitHub Actions runners.

---

## [1.1.4] - 2026-04-27

### Hotfix

- **Deeper AppImage EGL fix for Arch/rolling-release Linux** — Added a CI post-build patching step that strips the Ubuntu-bundled `libEGL`, `libGL`, `libGLdispatch`, `libGLX`, and `libGLESv2` from the AppImage squashfs and replaces the `AppRun` wrapper with one that: normalises the locale to `C.UTF-8`, sets `NO_AT_BRIDGE=1` to silence ATK warnings, auto-detects `EGL_PLATFORM` from `$WAYLAND_DISPLAY`/`$DISPLAY`, points GLVND at the system ICD vendor dirs, preloads the system `libEGL.so.1`, and orders `LD_LIBRARY_PATH` so host GPU drivers are always resolved before bundled stubs.

---

## [1.1.3] - 2026-04-27

### Hotfix

- **Fixed Arch Linux AppImage crash** — Resolved `EGL_BAD_ALLOC` error on Arch Linux (and other rolling-release distros) caused by bundled Mesa/EGL libraries conflicting with the host GPU driver stack. The app now automatically disables WebKitGTK's DMA-BUF renderer on Linux before the WebView initializes, with no impact to Windows or macOS builds.

---

## [1.0.4] - 2026-02-13

### Fixes

- Finally squashed the grid overlap bug for real. Cards were using CSS `aspect-[4/3]` to size themselves, but the virtualizer was computing row heights separately — at certain window widths these disagreed and rows would bleed into each other. Now both use the same explicit pixel height, so no more overlap regardless of how you resize the window.

### Cleanup

- Went through the whole codebase and ripped out every `console.log` / `console.error` we'd left in from debugging (16 of them). The one in `ErrorBoundary` stays since that's the whole point of an error boundary.
- Got rid of all `as any` casts on the frontend — everything's properly typed now.
- Ran Clippy and fixed all 7 warnings, including a couple of `collapsible_match` ones in `fs.rs` that needed manual refactoring.
- Dropped `clsx`, `tailwind-merge`, and `@tauri-apps/plugin-opener` from `package.json` — none of them were actually imported anywhere.
- General comment cleanup throughout.

---

## [1.0.3] - 2026-02-09

### Bug Fixes

- **Grid Spacing Fix** - Fixed cards overlapping in grid view
- **Dynamic Row Height** - Grid now properly calculates row height based on window size
- **Virtualizer Re-measurement** - Grid correctly updates when resizing window

---

## [1.0.2] - 2026-02-07

### Automated Release Pipeline

- **GitHub Actions Workflow** - Automatic builds triggered on version tags
- **Cross-Platform Builds** - Windows, Linux, macOS (Intel + ARM) built in parallel
- **Signed Updates** - All builds signed with Ed25519 for secure auto-updates
- **Automatic Publishing** - Releases published to GitHub automatically

---

## [1.0.1] - 2026-02-07

### Auto-Update System

- **Automatic Update Checks** - App checks for updates 5 seconds after startup
- **Update Banner** - Beautiful animated banner when new version available
- **One-Click Updates** - Download and install updates with progress indicator
- **Cross-Platform** - Windows, Mac, and Linux users get platform-specific updates

### 🔧 Technical

- Added Tauri updater plugin with Ed25519 signing
- Created `useUpdateCheck` hook for update lifecycle management
- Added `UpdateBanner` component with download progress

---

## [1.0.0] - 2026-02-06 🎉

### First Stable Release

Telegram Drive is now production-ready! This release focuses on performance, reliability, and user experience polish.

### ✨ New Features

- **Virtual Scrolling** - Smooth performance with folders containing 1000+ files
- **Inline Thumbnails** - Image files now display thumbnails directly in the file grid
- **Thumbnail Caching** - Thumbnails are cached locally for instant loading on revisit
- **API Setup Help Guide** - Step-by-step modal explaining how to get Telegram API credentials

### 🚀 Performance Improvements

- Grid and list views now only render visible items (virtualized)
- Responsive column layout adapts to window width
- Lazy loading of thumbnails to reduce initial load time

### 🎨 UI/UX Improvements

- Refined grid spacing (6px gaps between cards)
- Gradient overlay on thumbnail cards for text readability
- Improved light mode support across all components

### 🔧 Technical

- Added `@tanstack/react-virtual` for virtualization
- Separate thumbnail cache directory (`app_data_dir/thumbnails/`)
- FileTypeIcon now supports multiple sizes

---

## [0.6.0] - 2026-02-05

### Reliability Update

- Session persistence (window state, UI state, active folder)
- Network resilience with connection status indicator
- Queue persistence for uploads/downloads
- Light mode UI fixes

---

## [0.5.0] - 2026-02-04

### Drag & Drop Update

- Stable hybrid drag-drop system
- External drop blocker
- GitHub Actions workflow fixes

---

## [0.4.0] - 2026-02-01

### Media & Performance

- Audio/Video streaming player
- Global search filter
- Internal drag & drop between folders
