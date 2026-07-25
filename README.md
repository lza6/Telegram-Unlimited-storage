# Telegram Drive

**Telegram Drive** 是基于浏览器 + Python API 网关的开源无限云存储服务，将 Telegram 账号转换为安全的大容量存储驱动器。前端为静态 HTML/CSS/JS（部署在 `deploy/web`），后端为 Python FastAPI + Telethon（Headless 7×24 运行）。无桌面端，支持 Docker 或本地 Python 进程部署。

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![平台](https://img.shields.io/badge/platform-Windows%20%7C%20MacOS%20%7C%20Linux-blue)]()
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/lza6/Telegram-Unlimited-storage/pkgs/container/telegram-unlimited-storage)
[![测试覆盖率](https://img.shields.io/badge/coverage-80%25-green)]()

</div>

## 什么是 Telegram Drive？

Telegram Drive 利用 Telegram API，让你可以直接在 Telegram 服务器上上传、整理和管理文件。它把你的"收藏夹"和创建的频道当作文件夹，在浏览器中提供熟悉的文件管理器界面。

### 核心功能

- **无限云存储** — 以 Telegram 云基础设施作为你的存储驱动器
- **高性能列表** — 虚拟滚动支持数千个文件
- **流媒体播放** — 无需下载即可在线播放视频/音频
- **PDF 查看器** — 内置 PDF 支持，无限滚动
- **拖拽上传** — 仪表板内 HTML5 拖拽移动文件
- **缩略图预览** — 图片和媒体的嵌入式缩略图
- **文件夹管理** — 创建"文件夹"（私有 Telegram 频道）
- **分享链接** — 直链下载，可选密码保护和过期时间，随时可撤销
- **REST API（AI 集成）** — 带 API 密钥认证的开放接口和 OpenAPI 文档
- **代理支持** — SOCKS5 代理，保存后自动重连
- **隐私优先** — API 密钥和数据留在本地，无第三方服务器

## 技术栈

| 层级 | 技术 | 位置 |
|------|------|------|
| **前端** | 静态 HTML/CSS/JS | `deploy/web/` |
| **后端** | Python 3.11 + FastAPI + uvicorn + Telethon | `backend/app/` |
| **数据库** | SQLite（默认）/ PostgreSQL（控制面模式） | `backend/app/storage.py` |
| **部署** | Docker + docker-compose 或本地 Python 进程 | `Dockerfile`, `start.bat` |

## 默认登录凭据

> ⚠️ **重要**：部署前必须修改！

首次启动后访问 http://127.0.0.1:1334/login.html 登录

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `ACCESS_PWD` | `change-me-strong-password` | Web 管理台登录密码 |
| `API_KEY` | `generate-a-long-random-hex-key` | 外部 API 集成密钥 |

**修改方法**：编辑 `.env` 中的 `ACCESS_PWD` 和 `API_KEY`，然后重启服务。

## 快速开始

### 前置依赖

- **Python 3.11+**：[官网下载](https://www.python.org/downloads/)
- **Telegram API 凭据**：从 [my.telegram.org](https://my.telegram.org) 获取
  1. 登录 [my.telegram.org](https://my.telegram.org)
  2. 进入"API development tools"创建应用，获取 `api_id` 和 `api_hash`

### 本地 Python 部署

1. **克隆仓库**
   ```bash
   git clone https://github.com/lza6/Telegram-Unlimited-storage.git
   cd Telegram-Unlimited-storage
   ```

2. **创建虚拟环境并安装依赖**
   ```bash
   cd backend
   python -m venv .venv
   .venv\Scripts\activate        # Windows
   # source .venv/bin/activate   # macOS/Linux
   pip install -r requirements.txt
   cd ..
   ```

3. **配置凭据**
   ```bash
   cp .env.example .env
   # 编辑 .env：设置 TELEGRAM_API_ID、TELEGRAM_API_HASH、ACCESS_PWD、API_KEY
   ```

4. **启动服务**
   ```bash
   # 从仓库根目录：
   backend\.venv\Scripts\python -m uvicorn app.main:app --app-dir backend --host 127.0.0.1 --port 1334
   # 或使用启动器（Windows 菜单驱动）：
   start.bat
   ```

   浏览器打开 http://127.0.0.1:1334

### Docker 部署

详细指南见 [README-DOCKER.md](README-DOCKER.md)。

```bash
docker compose up -d --build
```

## Web 管理台页面

`deploy/web` 提供以下管理界面：

| 页面 | 功能 |
|------|------|
| `/dashboard.html` | 服务状态 + 分片上传（可选目标文件夹） |
| `/files.html` | 文件列表 / 搜索 / 批量删除·移动 / 下载 / 创建分享 |
| `/shares.html` | 分享管理 + 手动创建（需 Telegram 就绪） |
| `/settings.html` | 传输模式、分享域名、Headless 网络、Metrics |
| `/upload.html` | tg-disk 兼容上传（统一侧栏） |
| `/docs.html` | OpenAPI 静态文档（统一侧栏） |
| `/telegram.html` | User 模式 Telegram 登录（统一侧栏） |

## API 认证方式

- **Web 管理台**：使用 `X-Access-Pwd` 请求头（`.env` 中的 `ACCESS_PWD`）
- **外部集成**：使用 `X-API-Key`（`.env` 中的 `API_KEY`，Argon2 hash 校验）

## 相关文档

| 文档 | 内容 |
|------|------|
| [README-DOCKER.md](README-DOCKER.md) | Docker 部署详细指南 |
| [docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md) | 生产部署与 500 并发指南 |
| [docs/DESKTOP-API.md](docs/DESKTOP-API.md) | REST API 完整说明 |

## 开源与许可

本项目为**自由开源软件**，你可以自由使用、修改和分发。

采用 **MIT 许可**。

---
*免责声明：本应用与 Telegram FZ-LLC 无关联。请根据 Telegram 服务条款负责任地使用。*

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
