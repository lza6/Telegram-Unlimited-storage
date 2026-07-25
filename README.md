# Telegram Drive

**Telegram Drive** 是基于浏览器 + Python API 网关的开源无限云存储服务，将 Telegram 账号转换为安全的大容量存储驱动器。前端为静态 HTML/CSS/JS（`deploy/web`），后端为 Python FastAPI + Telethon（Headless 7×24 运行）。无桌面端，支持 Docker 或本地 Python 进程部署。

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![平台](https://img.shields.io/badge/platform-Windows%20%7C%20MacOS%20%7C%20Linux-blue)]()
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/lza6/Telegram-Unlimited-storage/pkgs/container/telegram-unlimited-storage)
[![测试覆盖率](https://img.shields.io/badge/coverage-80%25-green)]()

</div>

## 目录

- [什么是 Telegram Drive？](#什么是-telegram-drive)
- [快速开始（5 分钟跑通）](#快速开始5-分钟跑通)
- [登录凭据](#登录凭据)
- [配置说明](#配置说明)
- [启动方式](#启动方式)
- [Web 管理台](#web-管理台)
- [API 认证](#api-认证)
- [相关文档](#相关文档)
- [开源与许可](#开源与许可)

---

## 什么是 Telegram Drive？

利用 Telegram API，让你可以直接在 Telegram 服务器上上传、整理和管理文件。把"收藏夹"和创建的频道当作文件夹，在浏览器中提供熟悉的文件管理器界面。

### 核心功能

- **无限云存储** — 以 Telegram 云基础设施作为你的存储驱动器
- **高性能列表** — 虚拟滚动支持数千个文件
- **流媒体播放** — 无需下载即可在线播放视频/音频
- **PDF 查看器** — 内置 PDF 支持，无限滚动
- **拖拽上传** — 仪表板内 HTML5 拖拽移动文件
- **缩略图预览** — 图片和媒体的嵌入式缩略图
- **文件夹管理** — 创建"文件夹"（私有 Telegram 频道）
- **分享链接** — 直链下载，可选密码保护和过期时间，随时可撤销
- **REST API** — 带 API 密钥认证的开放接口，支持 AI 集成
- **SOCKS5 代理** — 保存后自动重连
- **隐私优先** — API 密钥和数据留在本地，无第三方服务器

---

## 快速开始（5 分钟跑通）

### 第一步：准备工作

你需要准备以下材料：

| 材料 | 获取方式 | 用途 |
|------|----------|------|
| **Telegram Bot Token** | @BotFather 创建机器人 | 上传文件到 Telegram |
| **私有频道 ID** | 创建私有频道并设为管理员 | 存储文件 |
| **ACCESS_PWD** | 自己定义一个强密码 | 登录 Web 管理台 |
| **API_KEY** | 自己生成一个随机密钥 | 外部 API 调用 |

#### 如何创建 Bot 和获取 Token

1. 在 Telegram 搜索 **@BotFather**，发送 `/newbot`
2. 给机器人起名字（比如 `MyDriveBot`）
3. 给机器人起用户名（必须以 `bot` 结尾，比如 `mydrive_bot`）
4. BotFather 会返回 Token，格式：`123456789:AAxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`
5. **保存好这个 Token，不要泄露！**

#### 如何创建私有存储频道

1. Telegram 新建频道 → **私有**
2. 名字随便起（比如 `MyDrive Storage`）
3. 不需要添加任何成员，直接创建
4. 创建完成后，进入频道 → 右上角"..." → **管理频道** → 找到 **链接**（就是频道 ID）
5. 频道 ID 格式为 `-100xxxxxxxxxx`（复制这个数字）
6. 把你的 Bot 添加为频道 **管理员**（管理频道 → 管理员 → 搜索你的 Bot → 添加）

### 第二步：克隆并配置

```bash
# 克隆仓库
git clone https://github.com/lza6/Telegram-Unlimited-storage.git
cd Telegram-Unlimited-storage

# 复制环境变量模板
copy .env.example .env
```

编辑 `.env` 文件，填入你的信息：

```env
# ==================== 必填项 ====================

# Bot Token（@BotFather 给你的）
TG_BOT_TOKEN=你的BotToken

# 私有频道 ID（格式：-100xxxxxxxxxx）
TG_STORAGE_CHANNEL_ID=-1001234567890

# Web 管理台登录密码（改成你自己的强密码）
ACCESS_PWD=你的强密码

# API 密钥（自己生成一个随机字符串，建议 32 位以上）
API_KEY=你的随机API密钥

# ==================== 可选项（一般不动） ====================

# 传输模式：bot = 机器人模式（默认），user = 用户模式
TELEGRAM_TRANSPORT_MODE=bot

# 服务端口（默认 1334）
PORT=1334

# 公网访问地址（本地调试不用改）
BASE_URL=http://localhost:1334
```

### 第三步：启动服务

#### 方式一：Windows 一键启动（推荐）

双击运行 `start.bat`，选菜单选项 1 或 2：

- **选项 1**：本地 Python 直接运行（开发用）
- **选项 2**：Docker 容器运行（生产用）

#### 方式二：命令行启动

```bash
# 本地 Python
cd backend
python -m venv .venv
.venv\Scripts\activate
pip install -r requirements.txt
cd ..
backend\.venv\Scripts\python -m uvicorn app.main:app --app-dir backend --host 127.0.0.1 --port 1334

# Docker
docker compose up -d --build
```

### 第四步：登录 Web 管理台

启动成功后，浏览器打开：

| 地址 | 说明 |
|------|------|
| http://127.0.0.1:1334/ | 首页 |
| http://127.0.0.1:1334/login.html | **登录页** |
| http://127.0.0.1:1334/dashboard.html | 管理面板 |
| http://127.0.0.1:1334/docs.html | API 文档 |

用 `ACCESS_PWD` 登录即可。

---

## 登录凭据

> ⚠️ **重要**：部署前必须修改默认密码！

### 默认凭据（初始密码）

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `ACCESS_PWD` | `change-me-strong-password` | **Web 管理台登录密码** |
| `API_KEY` | `generate-a-long-random-hex-key` | 外部 API 集成密钥 |

### 修改凭据

编辑 `.env` 文件：

```env
# 改成你自己的强密码
ACCESS_PWD=your_new_strong_password_here

# 改成随机生成的字符串（建议用密码管理器生成）
API_KEY=your_new_random_api_key_here
```

修改后**重启服务**生效。

### 生成随机 API_KEY 的方法

```bash
# Windows PowerShell
powershell -Command "[Convert]::ToBase64String((1..32 | ForEach-Object { Get-Random -Minimum 33 -Maximum 127 } | ForEach-Object { [char]$_ }))"

# 或者直接用随机十六进制
powershell -Command " -join ((1..64) | ForEach-Object { '{0:x}' -f (Get-Random -Maximum 16) })"
```

---

## 配置说明

以下是 `.env` 中所有配置项的详细说明：

### 必填配置

| 配置项 | 示例值 | 说明 |
|--------|--------|------|
| `TG_BOT_TOKEN` | `123456789:AAxxx...` | Telegram Bot Token，从 @BotFather 获取 |
| `TG_STORAGE_CHANNEL_ID` | `-1001234567890` | 私有存储频道 ID，Bot 必须是管理员 |
| `ACCESS_PWD` | `MySecretPass123` | Web 管理台登录密码 |
| `API_KEY` | `a1b2c3d4e5f6...` | 外部 API 调用密钥 |

### Telegram 传输模式

| 配置项 | 可选值 | 默认 | 说明 |
|--------|--------|------|------|
| `TELEGRAM_TRANSPORT_MODE` | `bot` / `user` / `auto` | `auto` | `bot`=仅机器人，`user`=用户账号，`auto`=自动选择 |

**机器人模式（推荐）**：
- 只需 Bot Token，无需手机号
- 单文件下载限 20MB
- 配置简单，适合大多数场景

**用户模式**：
- 需要 Telegram API ID 和 Hash（从 my.telegram.org 获取）
- 单文件大小无限制
- 需要首次 Web 登录授权

### 服务配置

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `PORT` | `1334` | 服务端口 |
| `BIND_HOST` | `127.0.0.1` | 监听地址（本地开发用，公网需 TLS 反向代理） |
| `BASE_URL` | `http://localhost:1334` | 公网访问地址，用于生成下载链接 |
| `DATA_DIR` | `./data` | 数据目录（session、SQLite 等） |
| `COMPOSE_FILE` | `docker-compose.yml` | Docker compose 配置文件 |

### 安全配置

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `ACCESS_LOCKOUT_MAX` | `8` | 登录失败锁定次数 |
| `ACCESS_LOCKOUT_SECS` | `300` | 锁定时间（秒） |
| `DOWNLOAD_SIGNING_SECRET` | `replace-with...` | 下载签名密钥（生产环境必改，至少 32 字符） |
| `PUBLIC_FILE_ID_DOWNLOAD` | `false` | 是否允许裸 file_id 下载（建议 false） |
| `MULTI_TENANT_ENABLED` | `true` | 是否启用多租户 |

### 上传配置

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `CHUNK_SIZE_MB` | `10` | 分片大小（MB），Bot 模式建议 ≤10MB |
| `CHUNK_CONCURRENT` | `4` | 同时上传的分片数 |
| `FILES_CONCURRENT` | `2` | 同时上传的文件数 |
| `UPLOAD_QUEUE_BACKEND` | `memory` | 上传队列后端（`memory`=单进程，`redis`=多副本共享） |

### 限流配置

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `RATE_LIMIT_RPM` | `120` | 每 IP 每分钟请求数限制 |
| `RATE_LIMIT_API_RPM` | `300` | 每 API Key 每分钟请求数限制 |
| `BOT_RATE_LIMIT_MS` | `3500` | Bot 上传间隔（毫秒），防止 FloodWait |

### 代理配置

| 配置项 | 说明 |
|--------|------|
| `PROXY_SOCKS5` | SOCKS5 代理地址，格式：`socks5://127.0.0.1:1080` |

### 数据库配置（高级）

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `SAAS_DATABASE_MODE` | `sqlite` | 数据库模式：`sqlite` 或 `postgresql` |
| `METADATA_CACHE_ENABLED` | `true` | 是否启用元数据缓存（单实例推荐 true） |

### 其他配置

| 配置项 | 说明 |
|--------|------|
| `METRICS_ENABLED` | 是否启用 Prometheus 指标（`GET /metrics`） |
| `WEBDAV_ENABLED` | 是否启用 WebDAV（企业接入） |
| `CORS_ORIGINS` | 跨域白名单（逗号分隔，留空禁止跨域） |

---

## 启动方式

### 方式一：start.bat（Windows 推荐）

```bash
# 双击 start.bat 或命令行运行
start.bat

# 指定模式
start.bat server   # 本地 Python
start.bat docker   # Docker
start.bat help    # 帮助
```

### 方式二：本地 Python

```bash
# 1. 进入后端目录
cd backend

# 2. 创建虚拟环境
python -m venv .venv

# 3. 激活虚拟环境
.venv\Scripts\activate

# 4. 安装依赖
pip install -r requirements.txt

# 5. 返回上级目录
cd ..

# 6. 启动服务
backend\.venv\Scripts\python -m uvicorn app.main:app --app-dir backend --host 127.0.0.1 --port 1334
```

### 方式三：Docker

```bash
# 1. 确保 Docker 运行中

# 2. 启动（开发模式，带热重载）
docker compose up -d

# 3. 完整重建
docker compose up -d --build

# 4. 查看日志
docker compose logs -f telegram-drive-api
```

### 方式四：Docker Compose 脚本（Windows PowerShell）

```powershell
# 日常启动
.\scripts\compose-up.ps1

# 带日志
.\scripts\compose-up.ps1 -Logs

# 全量重建（requirements.txt 或 Dockerfile 变更后）
.\scripts\compose-up.ps1 -Rebuild
```

### 启动后验证

```bash
# 进程存活
curl http://127.0.0.1:1334/health/live

# 流量就绪（Telegram 连接成功时 200，未连接时 503）
curl http://127.0.0.1:1334/health/ready

# 获取配置
curl http://127.0.0.1:1334/config
```

---

## Web 管理台

启动后打开浏览器访问：

| 页面 | 地址 | 功能 |
|------|------|------|
| **登录页** | http://127.0.0.1:1334/login.html | 输入 `ACCESS_PWD` 登录 |
| **首页** | http://127.0.0.1:1334/ | 自动跳转登录或仪表板 |
| **仪表板** | http://127.0.0.1:1334/dashboard.html | 服务状态、分片上传 |
| **文件管理** | http://127.0.0.1:1334/files.html | 文件列表、搜索、下载、分享 |
| **分享管理** | http://127.0.0.1:1334/shares.html | 管理分享链接 |
| **设置** | http://127.0.0.1:1334/settings.html | 传输模式、网络代理、域名配置 |
| **Telegram 登录** | http://127.0.0.1:1334/telegram.html | User 模式登录授权 |
| **API 文档** | http://127.0.0.1:1334/docs.html | OpenAPI 交互式文档 |

---

## API 认证

### Web 管理台认证

```bash
# 使用 X-Access-Pwd 请求头
curl -H "X-Access-Pwd: your-password" http://127.0.0.1:1334/api/v1/files
```

### 外部 API 认证

```bash
# 使用 X-API-Key 请求头
curl -H "X-API-Key: your-api-key" http://127.0.0.1:1334/api/v1/folders
```

### API 调用示例

```bash
# 获取文件夹列表
curl -H "X-API-Key: your-api-key" http://127.0.0.1:1334/api/v1/folders

# 获取文件列表
curl -H "X-API-Key: your-api-key" "http://127.0.0.1:1334/api/v1/files?limit=20"

# 下载文件
curl -H "X-API-Key: your-api-key" \
  -o downloaded.zip \
  "http://127.0.0.1:1334/api/v1/files/12345/download?folder_id=999"

# 创建分享链接
curl -X POST -H "X-API-Key: your-api-key" \
  -H "Content-Type: application/json" \
  -d '{"message_id": 123, "password": "optional"}' \
  http://127.0.0.1:1334/api/v1/shares

# tg-disk 兼容上传
curl -X POST "http://127.0.0.1:1334/upload" \
  -F "pwd=your-password" \
  -F "file=@./myfile.zip"
```

---

## 相关文档

| 文档 | 内容 |
|------|------|
| [README-DOCKER.md](README-DOCKER.md) | Docker 部署详细指南 |
| [docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md) | 生产部署与 500 并发指南 |
| [docs/DESKTOP-API.md](docs/DESKTOP-API.md) | REST API 完整说明 |

---

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
