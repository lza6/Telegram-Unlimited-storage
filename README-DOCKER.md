# Telegram Drive — Docker API 服务

面向 **7×24 服务器部署** 的无头 API 网关：业务系统用 HTTP 上传/下载/列举文件，无需阿里云 OSS。架构基于 **Telegram 用户 API（Telethon）**，容量与单文件大小优于 Bot API 方案（如 [tg-disk](https://github.com/Yohann0617/tg-disk)）。

**当前交付线：Web UI + Headless API**。纯 Python 后端（FastAPI + Telethon），无 Rust/Cargo 编译依赖。

**生产 / 500 并发 / 降级方案**：详见 [docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md)（含 Redis 多副本、容量规划、Nginx、运维清单）。

**默认端口：`1334`**（可在 `.env` 中设置 `PORT`，宿主机与容器内一致）。

## 镜像使用

```bash
# 使用预构建镜像（推荐）
docker run -d \
  -p 1334:1334 \
  -v ./data:/data \
  -e TELEGRAM_API_ID=... \
  -e TELEGRAM_API_HASH=... \
  -e ACCESS_PWD=... \
  -e API_KEY=... \
  ghcr.io/lza6/telegram-unlimited-storage:<release-tag>

# 自行构建（纯 Python，无编译，构建快）
docker build -t telegram-drive-server:latest .
```

## 工作流（一条 compose 管前后端）

`.env` 里 `COMPOSE_FILE=docker-compose.yml:docker-compose.dev.yml` 会自动合并 dev 层，**`docker compose up -d --build` 与 `dev.bat` / `up.bat` 同一套栈**（Web/docs Volume + uvicorn --reload 热重载）。

| 场景 | 命令 |
|------|------|
| 日常启动 | `.\dev.bat` 或 `docker compose up -d` |
| 前后端全量更新 | `.\up.bat` 或 `docker compose up -d --build` |
| 依赖 / Dockerfile 变更 | `.\dev.bat -Rebuild` |
| 服务器纯 Release | `.env` 设 `COMPOSE_FILE=docker-compose.yml` 再 `docker compose up -d --build` |

改 `deploy/web`、`docs` 保存即生效。改 Python 代码由 uvicorn --reload 自动重载，通常无需 `docker build`。

无 Docker 裸机调试仍用 `start.bat`，共用 `deploy/web` 与 `data/`。

## 与 tg-disk 对比

| 能力 | tg-disk (Go + Bot) | 本 Docker API |
|------|-------------------|---------------|
| 部署 | docker-compose + BOT_TOKEN | docker-compose + API_ID/HASH |
| Web 管理台 | 上传页 + 密码 | 控制台 + Telegram 登录 + **Swagger 文档页** |
| 上传 API | `POST /upload` + 分片 | 同上 + `POST /api/v1/files`（API Key multipart） |
| 下载 | `GET /d?file_id=` | 同上 + `GET /api/v1/files/{id}/download` + 流媒体 |
| 大文件 | 分片 + fileAll.txt | **用户 API 直传** + **tg-disk 同款分片/merge** |
| 分享链接 | 机器人回复 get | `GET/POST/DELETE /api/v1/shares` + `/d/{token}` |
| QR 登录 | 无 | `POST /api/v1/auth/qr/start` + `GET /api/v1/auth/qr/poll` |
| 文件夹/频道 | 固定 CHAT_ID | 多 `[TD]` 频道 + `GET /api/v1/folders` |
| 开发者文档 | README curl | **`/docs.html` + OpenAPI** |

## 快速开始

1. 复制环境变量：

```powershell
copy .env.example .env
# 首次配置清单、Bot/User 两种模式说明见 docs/ENVIRONMENT-SETUP.md
# 编辑 .env：TELEGRAM_API_ID、TELEGRAM_API_HASH、ACCESS_PWD、API_KEY
# 可选 PORT=1334（默认已是 1334）
```

2. 启动 / 更新：

```powershell
$env:DOCKER_BUILDKIT = "1"
.\dev.bat                        # 日常
docker compose up -d --build     # 全量更新（= .\up.bat）
```

### 加速说明

| 场景 | 命令 | 耗时 |
|------|------|------|
| 只改 `deploy/web` / `docs` | 保存文件即可（Volume 直读） | **秒级** |
| 改 Python 业务代码 `backend/app/*.py` | 保存 → uvicorn --reload 自动重载 | **秒级**（无 docker build） |
| 改 `requirements.txt` / Dockerfile | `.\scripts\dev-up.ps1 -Rebuild` | 较慢（仅此时 rebuild） |
| **生产/交付** 全量镜像 | `docker compose up -d --build` | 完整构建 |

**基础镜像**：`python:3.11-slim-bookworm`，镜像体积小，构建无需编译。

4. 浏览器打开（将 `1334` 换成你的 `PORT`）：

- 管理台：`http://localhost:1334/`
- API 文档：`http://localhost:1334/docs.html`
- 进程存活：`http://localhost:1334/health/live`
- 流量就绪：`http://localhost:1334/health/ready`（Telegram 可用时 200；未连接时 503）
- 兼容快照：`http://localhost:1334/api/v1/health`（始终 200，调用方必须读取 `ready`）

5. **首次** 打开「Telegram 登录」完成手机号验证；会话保存在 `./data/telegram.session`。

## 开发者接入

所有 v1 接口需请求头：

```
X-API-Key: <API_KEY 环境变量>
```

示例：

```bash
curl -H "X-API-Key: $API_KEY" "http://localhost:1334/api/v1/folders"
curl -H "X-API-Key: $API_KEY" "http://localhost:1334/api/v1/files?limit=20"
curl -H "X-API-Key: $API_KEY" -o out.bin \
  "http://localhost:1334/api/v1/files/12345/download?folder_id=999"
```

兼容 tg-disk 的上传（管理密码）：

```bash
curl -X POST "http://localhost:1334/upload" \
  -F "pwd=YOUR_ACCESS_PWD" \
  -F "file=@./app-release.apk"
```

## 测试

服务启动后：

```powershell
.\tests\integration\test-api.ps1 -BaseUrl http://localhost:1334 -AccessPwd YOUR_ACCESS_PWD -ApiKey YOUR_API_KEY
```

## 本地运行（无 Docker）

需安装 Python 3.11+。在仓库根目录：

```powershell
cd backend
python -m venv .venv
.venv\Scripts\activate
pip install -r requirements.txt
cd ..

# 从仓库根启动（.env 自动加载）
.venv\Scripts\python -m uvicorn app.main:app --app-dir backend --host 127.0.0.1 --port 1334
```

或根目录 `start.bat`（菜单驱动，自动读 `.env`）。

## 你需要提供

| 项 | 说明 |
|----|------|
| `TELEGRAM_API_ID` / `TELEGRAM_API_HASH` | [my.telegram.org](https://my.telegram.org) 创建应用 |
| `ACCESS_PWD` | Web 管理台密码 |
| `API_KEY` | 给业务系统用的密钥；未设置时启动日志会生成一次性 key |
| 可选 `PORT` | 默认 **1334**；Docker 映射为 `PORT:PORT` |
| 可选 `BASE_URL` | 公网访问地址，用于返回完整下载 URL |
| 可选 `CORS_ORIGINS` | 跨域白名单（逗号分隔；留空禁止跨域） |
| 可选 `RATE_LIMIT_RPM` / `RATE_LIMIT_API_RPM` | IP / API Key 每分钟限流 |
| 可选 `LOG_FORMAT=json` | 结构化 JSON 日志 |
| 可选代理 | 在 `data/network_settings.json` 配置 SOCKS5 |

## tg-disk 分片接口对齐说明

**路由名与表单字段与 tg-disk 一致**（`POST /upload_chunk`、`POST /merge_chunks`、`GET /d?file_id=`、`POST /verify`、`GET /config`）。

差异（有意保留）：

| 项 | tg-disk (Bot API) | 本服务 (User API) |
|----|-------------------|-------------------|
| `file_id` | Bot `file_id` 字符串 | **Telegram `message_id` 数字字符串** |
| 分片实体 | Bot 上传 blob | 用户 API 上传到 `[TD]` 频道 |
| manifest | `fileAll.txt` 格式相同 | 相同（首行文件名 + 每行 chunk message_id） |
| 鉴权 | `pwd` 表单 / 头 | 同上；v1 另支持 `X-API-Key` |

业务若从 tg-disk 迁移：只需把返回的 `file_id` 当作 **message_id** 使用；分片/合并调用方式不变。
