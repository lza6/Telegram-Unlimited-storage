# Telegram Drive

**Telegram Drive** is an open-source Web UI + Headless API service that turns your
Telegram account into an unlimited, secure storage drive. The delivery surface is
the browser console backed by a Python FastAPI + Telethon server. There is no
desktop shell; deployment is via Docker or a local Python process.

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20MacOS%20%7C%20Linux-blue)]()
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/lza6/Telegram-Unlimited-storage/pkgs/container/telegram-unlimited-storage)
[![Coverage](https://img.shields.io/badge/coverage-80%25-green)]()

</div>

## What is Telegram Drive?

Telegram Drive leverages the Telegram API to let you upload, organize, and manage
files directly on Telegram's servers. It treats your "Saved Messages" and created
Channels as folders, giving you a familiar file-explorer interface in the browser.

### Key Features

*   **Unlimited Cloud Storage** — Telegram's cloud infrastructure as your drive.
*   **High-Performance Grid** — virtual scrolling handles thousands of files.
*   **Media Streaming** — stream video/audio without downloading.
*   **PDF Viewer** — built-in PDF support with infinite scrolling.
*   **Drag & Drop Upload** — HTML5 drag for file moves in the dashboard.
*   **Thumbnail Previews** — inline thumbnails for images and media.
*   **Folder Management** — create "Folders" (private Telegram Channels).
*   **Shareable Links** — direct download links with optional password + expiry, revocable anytime.
*   **REST API for AI Integration** — secure API with API-key auth and OpenAPI spec.
*   **Proxy Support** — SOCKS5 proxy applied on save with automatic reconnect.
*   **Privacy Focused** — API keys and data stay local. No third-party servers.

## Server API (7×24 headless)

独立 Docker / Python API 网关（Bot/User 双模式、分片上传、预签名下载、多租户）：

- 快速开始：[README-DOCKER.md](README-DOCKER.md)
- 生产 / 高并发：[docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md)
- REST API 说明：[docs/DESKTOP-API.md](docs/DESKTOP-API.md)（桌面端已移除，现为统一网关）

Web 控制台（`deploy/web`）提供**上传、文件列表（下载/分享）、分享管理、传输模式、分享域名与 Headless 网络开关**。

| 页面 | 说明 |
|------|------|
| `/dashboard.html` | 服务状态 + 分片上传（可选目标文件夹） |
| `/files.html` | 列表 / 搜索 / 批量删除·移动 / 下载 / 创建分享 |
| `/shares.html` | 分享管理 + 手动创建（需 Telegram 就绪） |
| `/settings.html` | 传输模式、分享域名、Headless 网络、Metrics |
| `/upload.html` | tg-disk 兼容上传（统一侧栏） |
| `/docs.html` | OpenAPI 静态文档（统一侧栏） |
| `/telegram.html` | User 模式 Telegram 登录（统一侧栏） |

Web 调用 API 使用登录密码作为 `X-Access-Pwd` 请求头；外部集成使用 `X-API-Key`（Argon2 hash 校验）。

## Tech Stack

*   **Frontend**: Static HTML/CSS/JS (served from `deploy/web`)
*   **Backend**: Python 3.11 + FastAPI + uvicorn + Telethon (Telegram Client)
*   **Database**: SQLite (default) / PostgreSQL (control-plane mode)
*   **Deployment**: Docker + docker-compose, or local Python process

## Getting Started

### Prerequisites

*   **Python 3.11+**: [Download here](https://www.python.org/downloads/)
*   **Telegram API Credentials**: You need your own API ID and API Hash.
    1. Log into [my.telegram.org](https://my.telegram.org).
    2. Go to "API development tools" and create a new application to get your `api_id` and `api_hash`.

### Installation (local Python)

1.  **Clone the repository**
    ```bash
    git clone https://github.com/lza6/Telegram-Unlimited-storage.git
    cd Telegram-Unlimited-storage
    ```

2.  **Create a virtualenv and install dependencies**
    ```bash
    cd backend
    python -m venv .venv
    .venv\Scripts\activate        # Windows
    # source .venv/bin/activate   # macOS/Linux
    pip install -r requirements.txt
    cd ..
    ```

3.  **Configure credentials**
    ```bash
    cp .env.example .env
    # Edit .env: set TELEGRAM_API_ID, TELEGRAM_API_HASH, ACCESS_PWD, API_KEY
    ```

4.  **Run the server**
    ```bash
    # From the repository root:
    .venv\Scripts\python -m uvicorn app.main:app --app-dir backend --host 127.0.0.1 --port 1334
    # Or use the launcher:
    start.bat   # Windows menu-driven launcher
    ```

    Open http://127.0.0.1:1334 in your browser.

### Installation (Docker)

See [README-DOCKER.md](README-DOCKER.md) for the full Docker deployment guide.

```bash
docker compose up -d --build
```

## Open Source & License

This project is **Free and Open Source Software**. You are free to use, modify, and distribute it.

Licensed under the **MIT License**.

---
*Disclaimer: This application is not affiliated with Telegram FZ-LLC. Use responsibly and in accordance with Telegram's Terms of Service.*

<div align="center">
  <div style="margin: 15px 0;">
    <a href="https://www.paypal.me/Caamer20">
      <img src="https://raw.githubusercontent.com/stefan-niedermann/paypal-donate-button/master/paypal-donate-button.png" alt="Donate with PayPal" width="200">
    </a>
    <div style="font-size: 14px; margin-top: 8px;">paypal.me/Caamer20</div>
  </div>
  <div style="margin: 15px 0;">
    <a href="litecoin:ltc1q6wkr5ac4u0pxx4hx7xgwn0gsaku25ws0df73rp">
      <img src="https://img.shields.io/badge/Donate-LTC-345D9D?style=for-the-badge&logo=litecoin&logoColor=white" alt="Donate LTC">
    </a>
    <div style="font-family: monospace; font-size: 13px; margin-top: 8px; word-break: break-all;">
      ltc1q6wkr5ac4u0pxx4hx7xgwn0gsaku25ws0df73rp
    </div>
  </div>
  <div style="margin: 15px 0;">
    <a href="bitcoin:bc1q5pt7m2fk6w0dzsnf6vvd5k6nw5k44785286ujy">
      <img src="https://img.shields.io/badge/Donate-BTC-F7931A?style=for-the-badge&logo=bitcoin&logoColor=white" alt="Donate BTC">
    </a>
    <div style="font-family: monospace; font-size: 13px; margin-top: 8px; word-break: break-all;">
      bc1q5pt7m2fk6w0dzsnf6vvd5k6nw5k44785286ujy
    </div>
  </div>
</div>
