# Telegram Drive

**Telegram Drive** is an open-source, cross-platform desktop application that turns
your Telegram account into an unlimited, secure cloud storage drive. Built with
**Tauri**, **Rust**, and **React**.

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20MacOS%20%7C%20Linux-blue)]()
[![Downloads](https://img.shields.io/github/downloads/caamer20/telegram-drive/total)](https://github.com/caamer20/telegram-drive/releases)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](ghcr.io/caamer20/telegram-drive)
[![Coverage](https://img.shields.io/badge/coverage-80%25-green)]()

</div>

![Auth Screen](screenshots/AuthScreen.png)

## v4.0.0-beta Highlights

**Security Hardening**
- Argon2id API key hashing with auto-migration from legacy SHA-256
- CSP nonce-based Content Security Policy
- RFC 5987 Content-Disposition encoding (filename injection prevention)

**Architecture**
- SQLite connection pooling (r2d2) for high concurrency
- Configurable Telegram DC address

**Frontend**
- Component decomposition (AuthWizard → 11 modules, Dashboard hooks)
- Full A11y compliance (ARIA roles, labels, focus traps, keyboard navigation)
- Responsive design for mobile

**DevOps**
- Docker image <400MB (UPX compression, non-root user)
- CI/CD with coverage gates, multi-platform builds (amd64/arm64)

##  What is Telegram Drive?

Telegram Drive leverages the Telegram API to allow you to upload, organize, and manage files directly on Telegram's servers. It treats your "Saved Messages" and created Channels as folders, giving you a familiar file explorer interface for your Telegram cloud.

###  Key Features

*   **Unlimited Cloud Storage**: Utilizing Telegram's generous cloud infrastructure.
*   **High Performance Grid**: Virtual scrolling handles folders with thousands of files instantly.
*   **Auto-Updates**: Seamless updates for Windows, macOS, and Linux.
*   **Media Streaming**: Stream video and audio files directly without downloading.
*   **PDF Viewer:** Built-in PDF support with infinite scrolling for seamless document reading.
*   **Drag & Drop Upload**: Desktop — drag files from Finder/Explorer onto the window (Tauri `onDragDropEvent`); in-dashboard HTML5 drag for file moves stays enabled via `dragDropEnabled: false`. Browser dev — use Upload button.
*   **Thumbnail Previews**: Inline thumbnails for images and media files.
*   **Folder Management**: Create "Folders" (private Telegram Channels) to organize content.
*   **Shareable Links**: Generate direct download links with optional password protection and expiration, and revoke access anytime from the dashboard. Also supports copying native Telegram message links for files in public channels.
*   **REST API for AI Integration**: Secure local API (off by default) with configurable port and API key auth. OpenAPI spec for seamless LLM and tool integration.
*   **Proxy Support**: SOCKS5 proxy (grammers-backed); applied on save with automatic reconnect.
*   **VPN Optimizer**: Aggressive network tuning including bandwidth throttling, adjustable transfer chunk sizing, adaptive keep-alives, and auto-detect VPN to enable optimizer when VPN interfaces are present.
*   **Privacy Focused**: API keys and data stay local. No third-party servers.
*   **Cross-Platform**: Native apps for macOS (Intel/ARM), Windows, and Linux.

## Server API (7×24 headless)

独立 Docker API 网关（Bot/User 双模式、分片上传、预签名下载、多租户）：

- 快速开始：[README-DOCKER.md](README-DOCKER.md)
- 生产 / 高并发：[docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md)
- 闭环审计记录：[docs/AUDIT-CLOSURE.md](docs/AUDIT-CLOSURE.md)（第三十九轮：Connection Hook 测试 + E2E API 管线）
- 深度审查（R56）：[docs/ROUND-56-AUDIT.md](docs/ROUND-56-AUDIT.md) ·（R55）：[docs/ROUND-55-AUDIT.md](docs/ROUND-55-AUDIT.md) · E2E 增量登记：[docs/E2E-CHECKPOINTS.md](docs/E2E-CHECKPOINTS.md) · 扩展：[docs/PRODUCT-EXTENSION-IDEAS.md](docs/PRODUCT-EXTENSION-IDEAS.md)
- 扩展建议：[docs/EXTENSION-UX-R37.md](docs/EXTENSION-UX-R37.md)
- TDD 计划：[docs/ROUND-25-TDD.md](docs/ROUND-25-TDD.md) · [docs/ROUND-16-TDD.md](docs/ROUND-16-TDD.md)
- 桌面 REST 与 Headless 差异：[docs/DESKTOP-API.md](docs/DESKTOP-API.md)

Web 控制台（`deploy/web`）提供**上传、文件列表（下载/分享）、分享管理、传输模式、分享域名与 Headless 网络开关**；深度文件操作请用**桌面端**或 **REST API**（`/api/v1/*`）。Web 与后端统一通过 `TdApi.ensureServiceReady()` 校验 Telegram 就绪（User 模式含 `auth/status.connected`）；`dashboard.html` 与 `upload.html` 共用 `page-readiness.js`；登录页与 Telegram 登录均用 `safeNext()` 防开放重定向。桌面端启动时 `connectionStatus=checking`，首检通过前禁止传输；8550/14201 传输模式通过 `transport_mode.json` 同步。`GET /api/v1/settings` 返回 `effective_share_link_base`（Headless 与 API 同端口；桌面 REST 为流媒体 **14201**）；另含 `effective_share_base_url` 供桌面流媒体参考。

| 页面 | 说明 |
|------|------|
| `/dashboard.html` | 服务状态 + 分片上传（可选目标文件夹） |
| `/files.html` | 列表 / 搜索 / 批量删除·移动 / 下载 / 创建分享 |
| `/shares.html` | 分享管理 + 手动创建（需 Telegram 就绪） |
| `/settings.html` | 传输模式、分享域名、Headless 网络、Metrics |
| `/upload.html` | tg-disk 兼容上传（统一侧栏） |
| `/docs.html` | OpenAPI 静态文档（统一侧栏） |
| `/telegram.html` | User 模式 Telegram 登录（统一侧栏） |

Web 调用 API 使用登录密码作为 `X-Access-Pwd` 请求头（OpenAPI 中 admin 路由均标注 `AccessPwdAuth` + `ApiKeyAuth`）；分享域名写入服务端 `ui_settings.json`（`PUT /api/v1/settings`），桌面端通过 `cmd_set_ui_share_domain` 同步，写入失败会 toast。外部集成使用 `X-API-Key`（Argon2 hash 校验）。Web 文件列表行内「分享」会确认创建无密码永久链接；带密码/有效期请用分享管理页。

**桌面可选 REST API**（Settings → API）：启用后自动生成 **Local Access Password**（`X-Access-Pwd`），亦可生成 API Key。老版本已开启 API 的用户在下次启动时会自动补全本地密码。侧栏显示三种连接态（会话活跃 / 会话过期 / 无网络）。开发环境下 REST 端口（默认 **8550**）可同时提供 `deploy/web` 静态页（含 `/telegram.html`），Settings 切 User 时优先在浏览器打开该页完成绑定。详见 [docs/DESKTOP-API.md](docs/DESKTOP-API.md)。

##  Screenshots

| Dashboard | File Preview |
|-----------|--------------|
| ![Dashboard](screenshots/DashboardWithFiles.png) | ![Preview](screenshots/ImagePreview.png) |

| Grid View | Authentication |
|-----------|----------------|
| ![Dark Mode](screenshots/DarkModeGrid.png) | ![Login](screenshots/LoginScreen.png) |

| Audio Playback | Video Playback |
|----------------|----------------|
| ![Audio Playback](screenshots/AudioPlayback.png) | ![Video Playback](screenshots/VideoPlayback.png) |

| Auth Code Screen | Upload Example |
|------------------|-------------|
| ![Auth Code Screen](screenshots/AuthCodeScreen.png) | ![Upload Example](screenshots/UploadExample.png) |

| Folder Creation | Folder List View |
|-----------------|------------------|
| ![Folder Creation](screenshots/FolderCreation.png) | ![Folder List View](screenshots/FolderListView.png) |

##  Tech Stack

*   **Frontend**: React, TypeScript, TailwindCSS, Framer Motion
*   **Backend**: Rust (Tauri), Grammers (Telegram Client)
*   **Build Tool**: Vite


##  Getting Started

### Prerequisites

*   **Node.js (v18+)**: [Download here](https://nodejs.org/)
*   **Rust (latest stable)**: Required to compile the Tauri backend. Install via [rustup](https://rustup.rs/):
    *   **macOS/Linux:** `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
    *   **Windows:** Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs/)
    *   *Verify installation:* run `rustc --version` and `cargo --version` in your terminal.
*   **OS-Specific Build Tools for Tauri**: 
    *   **macOS:** Xcode Command Line Tools (`xcode-select --install`).
    *   **Linux (Ubuntu/Debian):** `sudo apt update && sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`
    *   **Windows (CRITICAL):** You **must** install the [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/). During installation, select the **"Desktop development with C++"** workload. Without this, you will get a `linker 'link.exe' not found` error.
    *   **Windows (WebView2):** Windows 10/11 users usually have this pre-installed. If not, download the [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section).
    *   *Reference:* See the official [Tauri v2 Prerequisites Guide](https://v2.tauri.app/start/prerequisites/) for detailed instructions.
*   **Telegram API Credentials**: You need your own API ID and API Hash to communicate with Telegram's servers.
    1. Log into [my.telegram.org](https://my.telegram.org).
    2. Go to "API development tools" and create a new application to get your `api_id` and `api_hash`.

> [!NOTE]  
> **First-run Compile Time:** The initial build (`npm run tauri dev` or `npm run tauri build`) will download and compile over 300 Rust crates. This process can take **5 to 15 minutes** depending on your hardware. Subsequent builds will be much faster.

> [!TIP]
> **NPM Vulnerabilities:** You may see vulnerability warnings during `npm install`. These are usually related to build tools and dev dependencies. You can optionally run `npm audit fix`, but it is not strictly required to run the app.

### Installation

1.  **Clone the repository**
    ```bash
    git clone https://github.com/caamer20/Telegram-Drive.git
    cd Telegram-Drive
    ```

2.  **Install Dependencies**
    ```bash
    cd app
    npm install
    ```

3.  **Run in Development Mode**
    ```bash
    npm run tauri dev
    ```

4.  **Build/Compile**
    ```bash
    npm run tauri build
    ```

##  Open Source & License

This project is **Free and Open Source Software**. You are free to use, modify, and distribute it.

Licensed under the **MIT License**.

---
*Disclaimer: This application is not affiliated with Telegram FZ-LLC. Use responsibly and in accordance with Telegram's Terms of Service.*

If you're looking for a version of this app that's optimized for VPNs check out this repo:
https://github.com/caamer20/Telegram-Drive-ForVPNs

<div align="center">
  <!-- PayPal -->
  <div style="margin: 15px 0;">
    <a href="https://www.paypal.me/Caamer20">
      <img src="https://raw.githubusercontent.com/stefan-niedermann/paypal-donate-button/master/paypal-donate-button.png" alt="Donate with PayPal" width="200">
    </a>
    <div style="font-size: 14px; margin-top: 8px;">paypal.me/Caamer20</div>
  </div>

  <!-- Litecoin -->
  <div style="margin: 15px 0;">
    <a href="litecoin:ltc1q6wkr5ac4u0pxx4hx7xgwn0gsaku25ws0df73rp">
      <img src="https://img.shields.io/badge/Donate-LTC-345D9D?style=for-the-badge&logo=litecoin&logoColor=white" alt="Donate LTC">
    </a>
    <div style="font-family: monospace; font-size: 13px; margin-top: 8px; word-break: break-all;">
      ltc1q6wkr5ac4u0pxx4hx7xgwn0gsaku25ws0df73rp
    </div>
  </div>

  <!-- Bitcoin -->
  <div style="margin: 15px 0;">
    <a href="bitcoin:bc1q5pt7m2fk6w0dzsnf6vvd5k6nw5k44785286ujy">
      <img src="https://img.shields.io/badge/Donate-BTC-F7931A?style=for-the-badge&logo=bitcoin&logoColor=white" alt="Donate BTC">
    </a>
    <div style="font-family: monospace; font-size: 13px; margin-top: 8px; word-break: break-all;">
      bc1q5pt7m2fk6w0dzsnf6vvd5k6nw5k44785286ujy
    </div>
  </div>
</div>
