# REST API 说明（桌面端已移除）

> **注意**：桌面端（Tauri/Rust）及其独立的桌面 REST API（端口 8550 / 流媒体 14201）
> 已在 v5.0 移除。本项目现在只有**一个统一的 Headless API 网关**（默认端口 `1334`），
> 由 Python FastAPI + Telethon 实现，同时服务 Web 控制台与外部集成。

## 统一 API 网关

所有功能通过单一端口（默认 `1334`，可用 `.env` 的 `PORT` 配置）提供：

| 路径 | 功能 |
|------|------|
| `/api/v1/files` | 文件列表 / 上传 / 下载 |
| `/api/v1/shares` | 分享管理（创建 / 列表 / 撤销） |
| `/api/v1/folders` | 文件夹（Telegram 频道）管理 |
| `/api/v1/settings` | 配置管理 |
| `/api/v1/network` | 网络 / 代理配置 |
| `/api/v1/auth/*` | Telegram 登录（手机号 / QR） |
| `/api/v1/health` | 健康快照（始终 200，读取 `ready`） |
| `/health/live` · `/health/ready` | 存活 / 流量就绪探针 |
| `/d/{token}` | 分享下载页 |
| `/docs.html` | OpenAPI 静态文档 |

## 认证

- **Web 控制台**：`X-Access-Pwd`（`.env` 的 `ACCESS_PWD`）
- **外部集成**：`X-API-Key`（`.env` 的 `API_KEY`，Argon2id 校验，兼容旧 SHA-256 自动升级）

## 与旧桌面 API 的差异

| 项 | 旧桌面 REST（已移除） | 统一网关（当前） |
|----|----------------------|------------------|
| 端口 | 8550（API）+ 14201（流媒体） | 单一 `1334` |
| 分享链接指向 | 14201 流媒体端口 | 同端口 `/d/{token}` |
| 启用方式 | Settings 手动开启 | 始终运行 |
| 实现 | Rust / Tauri | Python / FastAPI |

完整部署与接入见 [README-DOCKER.md](../README-DOCKER.md) 与
[DEPLOYMENT-PRODUCTION.md](DEPLOYMENT-PRODUCTION.md)。
