# R54 审计（桌面 Bot 模式下载经本地 REST）

> 计划：[ROUND-54-PLAN.md](ROUND-54-PLAN.md)

## 主要矛盾（已闭合）

Bot 可浏览/搜索/删索引，但下载仍锁在 GramJS User 会话 → 用户看到「Bot 已就绪」却无法点下载。

## 落地项

| 层 | 变更 |
|----|------|
| Rust `download_file_via_local_api` | `desktop_uses_asset_index` 时 HTTP GET `127.0.0.1:{port}/api/v1/files/{id}/download` + `X-Access-Pwd`，流式写盘 + `download-progress` + 取消 |
| `cmd_download_file` | 注入 `db_pool`；索引权威时早返回 REST 路径 |
| `canDownloadFiles` / `canEnqueueDownload` | 已有纯函数；测试补全 |
| `useFileDownload` / `useFileOperations` | `canDownload` 门禁；批量/文件夹下载走 `guardDownload` |
| UI | `downloadReady` / `deleteReady` 分离 `sessionOnline`；TopBar / FileCard / ContextMenu 下载钮与删除钮对齐后端能力 |
| Banner | Bot 文案更新为含「下载」 |

## 自反驳（最强反对观点）

| 声称 | 反驳 | 处置 |
|------|------|------|
| 「Bot 下载已完全 parity」 | **假**：预览/分享/上传/移动/缩略图仍要 User；`cmd_get_preview` 未走 REST | 文档标明 R55；Banner 已写清 |
| 「REST 代理等于零成本」 | **半真**：仍经本地 headless 拉 Telegram Bot API（用户自测时）；CI 不测真实网络 | 符合「mock 优先」策略 |
| 「UI 删除在 R52 已可用」 | **假**：R52 仅 hook 层 `canIndexDelete`；FileCard 仍 `transferEnabled` 禁删 | R54 用 `deleteReady` 修复 |
| 「无 User 时缩略图可用」 | **假**：`FileCard` 缩略图仍 `transferEnabled` 门禁 | 已知次要矛盾 |

## 反转条件

- User 在线 → GramJS 直传路径不变
- 本地 API 停服 → `downloadReady=false`，与列表一致
- `ACCESS_PWD` 空 → Rust 返回明确错误，队列项 `error`

## 下一阶段信号

- 用户反馈「能下不能看」→ 预览 REST URL / `cmd_get_preview` Bot 分支（R55）

## 验证（2026-06-12）

| 套件 | 结果 |
|------|------|
| vitest | **228 passed** |
| coverage (lib+hooks) | **96.8% stmts / 86.85% branch** |
| cargo `--features headless-server --lib` | **92 passed** |
| Playwright static | **41 passed**, 2 skipped |
| Playwright `E2E_API=1` | **43 passed** |
