# R54 计划（桌面 Bot 模式下载经本地 REST 闭环）

> **主要矛盾（决定性）**：R52/R53 已让 Bot 可浏览/搜索/删索引，但**下载**仍要求 GramJS User 会话——`cmd_download_file` 只走 `require_client`，前端 `useFileDownload` 仅用 `canTransfer`。

> **牵引性**：用户期望 Banner「Bot 已就绪」与可点下载按钮一致；队列应经 `127.0.0.1` 本地 API 流式落盘（mock 友好，不烧 Telegram 流量）。

> **阶段性**：本回合交付下载链路；预览/上传/移动仍依赖 User 会话（下阶段 R55）。

## 内外因

| 类型 | 内容 |
|------|------|
| 内因（可改） | Rust `cmd_download_file` 分支、`useFileDownload` 门禁、FileCard/TopBar `downloadEnabled` |
| 外因（硬条件） | 本地 API 须运行且 `X-Access-Pwd` 已配置；Bot 文件实体仍由 headless REST 提供 |
| 外因→内因 | API 未启动 → `apiHealth.ready=false` → `downloadReady=false`（与列表行为一致） |

## 次要矛盾（本回合盯住不主攻）

- 预览仍走 `cmd_get_preview`（需 User）
- `BulkResponse.skipped_ids` 审计字段
- Desktop E2E mock Tauri invoke

## TDD 清单

1. RED `canEnqueueDownload` / `canDownloadFiles` 用例补全
2. RED `useFileDownload` — `canTransfer=false` + `canDownload=true` 可入队
3. GREEN `download_file_via_local_api` + `cmd_download_file` 资产索引分支
4. GREEN `Dashboard` / `TopBar` / `FileExplorer` — `downloadReady` / `deleteReady` 与按钮对齐
5. VERIFY vitest coverage / cargo / playwright

## 反转条件

- User 在线：仍走 GramJS 直传（现有路径不变）
- `desktop_uses_asset_index=false`：不碰 REST 代理
- 本地 API 返回 4xx：队列项 `error`，不静默成功

## 下一阶段矛盾转移信号

- 下载可用后，用户会追问**预览/分享**在 Bot 下的 parity → R55 主攻预览 REST URL
