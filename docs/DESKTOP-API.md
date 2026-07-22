# 桌面可选 REST API（与 Headless 差异）

桌面应用在 **Settings → General → REST API** 中可启用本地 HTTP 服务（默认 `127.0.0.1:8550`）。与 Docker **Headless 统一网关（1334）** 不同，桌面 API 是**子集**，且分享/流媒体端口分离。

## 端口

| 服务 | 默认端口 | 说明 |
|------|----------|------|
| 桌面 REST API | 8550 | Settings 可改；仅绑定 `127.0.0.1` |
| 流媒体 / 分享下载 `/d/*` | 14201 | 与 REST API **不同进程** |
| Headless 统一网关 | 1334 | API + Web + `/d/*` + legacy 上传同端口 |

## 已挂载路由（桌面 REST）

- `/health/live`（仅进程存活，依赖故障不应触发进程重启）
- `/health/ready`（严格流量就绪；Telegram 未连接时返回 503）
- `/api/v1/health`（兼容快照，始终返回 200；必须读取 `ready`）
- `/api/v1/files`、`/search`、`/folders`、上传相关 REST
- `/api/v1/shares`
- `/api/v1/settings`、`/api/v1/network`
- `/api/v1/transport`（需鉴权）
- Telegram User 登录相关 `/api/v1/auth/*`

## 静态 Web（第三十二轮 · Scheme B 子集）

当本机可解析到 `deploy/web`（开发态 `app/src-tauri/../../deploy/web`，或环境变量 `STATIC_DIR`）时，桌面 REST **同端口**挂载静态页，与 API 路由并存（API 优先）：

- `/telegram.html` — User 模式 Telegram 登录（与 Headless 相同页面）
- `/login.html`、`/settings.html`、`/files.html` 等完整 `deploy/web` 树

若打包后找不到 `telegram.html`，Settings 切 User 时会回退 Headless `:1334` 或提示使用**应用内 Auth**。安装包已将 `deploy/web` 捆绑至 Tauri 资源目录 `web/`（第三十三轮）。

**未挂载（仅 Headless 或桌面无静态目录时）**

- `/upload`、`/upload_chunk`、`/merge_chunks`（legacy 分片）
- `/verify`、`/config`（legacy 管理）
- `/metrics`、WebDAV

完整 OpenAPI 描述 Headless；桌面集成请以上表为准。

## 鉴权

启用 API 后自动生成 **Local Access Password**，写入 `app_data_dir/api_settings.json`，在 Settings 中可复制。

```bash
# 方式 1：本地管理密码
curl -H "X-Access-Pwd: <Local Access Password>" http://127.0.0.1:8550/api/v1/files

# 方式 2：API Key（Settings → Generate）
curl -H "X-API-Key: <hex key>" http://127.0.0.1:8550/api/v1/files
```

未生成 API Key 时，仍可用 **X-Access-Pwd**（本地密码）调用所有需鉴权端点。

## 分享链接

- Tauri UI / `cmd_create_share`：链接 base 来自 `ui_settings.share_domain` 或 `127.0.0.1:14201`
- REST `POST /api/v1/shares`：桌面 REST 下链接同样指向 **14201** 的 `/d/{token}`（非 API 端口）
- REST `POST /api/v1/files`（multipart 上传）：返回的 `download_url` 与上述一致，走 **14201**；`api_download_url` 走 REST API 端口（8550）的 `/api/v1/files/{id}/download`（第十三轮）

Tailscale/LAN：在分享域名填 `100.x.x.x:14201`，或反代 `/d/*` 到 14201。

## 数据目录

- SQLite、`network_settings.json`、`ui_settings.json`、`api_settings.json` → `app_data_dir`
- 流媒体 Transport 配置与上述目录一致（第八轮起默认 `app_data_dir`；可选 `DATA_DIR` 覆盖）
- 已启用 API 但缺少 `local_access_pwd` 的旧配置：应用启动/重启 API 时自动补全（第十轮）

## Bot 模式（桌面 + Headless）

- **列表/搜索/删除索引**：走 `file_assets`；Bot 删除仅移除本地索引，**不会**删除 Telegram 频道内消息。
- **下载/预览/分享下载**：需要 `bot_file_map`（含 Telegram `file_id`）。创建分享前会校验该映射存在；删除时会同时清除 `file_assets` 与 `bot_file_map`，并**自动撤销**该 `message_id` 的全部活跃分享链接。
- **批量移动**：需要 User 模式（GramJS `forward_messages`）。Bot 模式下 REST 返回 `NOT_SUPPORTED`；桌面 `cmd_move_files` 返回明确错误文案。
- **桌面 Bot 下载**：需启用本地 REST API 且配置 **Local Access Password**（`local_api` 回环调用 `/api/v1/files/{id}/download`）。未启用 API 时 Bot 下载/预览不可用。

## 桌面 UI 连接态

侧栏状态点：

| 状态 | 含义 |
|------|------|
| 绿 · Telegram session active | 网络可达且 `cmd_check_connection` 通过 |
| 黄 · Session expired | 网络通但 Telegram 会话失效 |
| 红 · No network connection | 无法连接 Telegram DC / 代理不可达 |

会话失效时上传/下载/删除/移动/搜索/分享/预览会禁用或触发自动登出（第十～十二轮）。

## PostgreSQL upload Saga (N-2C)

When `SAAS_DATABASE_MODE=postgres`, `POST /api/v1/files` requires a non-empty `Idempotency-Key` header. The same key and the same semantic upload request returns the persisted result without a second normal transport call. Reusing the key for different content, transport mode or target returns `409 IDEMPOTENCY_CONFLICT`.

Relevant conflict/recovery responses include `UPLOAD_IN_PROGRESS`, `UPLOAD_RECONCILIATION_REQUIRED`, `UPLOAD_COMPENSATION_PENDING`, `UPLOAD_TERMINAL` and `UPLOAD_LEASE_LOST`. Upload bytes are staged under `<DATA_DIR>/saga-staging/`; receipt and compensation records are append-only under `<DATA_DIR>/saga-recovery/`. `POSTGRES_APP_USER` is also the authenticated recovery node identity; deployments must use a distinct database role per recovery node.

This implementation does not claim atomic Telegram/PostgreSQL exactly-once behavior. Real Telegram acceptance and ambiguous-response reconciliation remain separate gates.
