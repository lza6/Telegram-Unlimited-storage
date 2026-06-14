# R55 审计（桌面 Bot 模式预览/流媒体 parity）

> 计划：[ROUND-55-PLAN.md](ROUND-55-PLAN.md)

## 主要矛盾（决定性）

R54 后 Bot 用户可下载，但 **预览/缩略图/视频 PDF 流** 仍依赖 GramJS User 会话 → 「能下不能看」。

## 自我反驳（验收前）

| 声称 | 反驳 | 处置 |
|------|------|------|
| 「REST 预览与下载同链路即可」 | 流媒体走 `:14201/stream/*`，非 `cmd_get_preview` | `stream_media` 无 client 时代理本地 API + Range |
| 「改 Dashboard 门禁就够」 | FileCard 预览钮仍 `transferEnabled` | 新增 `previewEnabled` / `previewReady` |
| 「缩略图可继续空返回」 | Bot 下列表无 inline thumb，体验差 | `cmd_get_thumbnail` Bot 分支落盘小图 |
| 「共享 fs 下载逻辑」 | 三处重复 URL/鉴权 | `local_api.rs` 抽取 `LocalApiBridge` + `fetch_file_to_path` |

## 落地

| 层 | 变更 |
|----|------|
| Rust `local_api.rs` | `build_download_url`、`fetch_file_to_path`、`desktop_uses_asset_index` |
| `preview.rs` | Bot 分支：`preview_via_local_api` / `thumbnail_via_local_api` |
| `server.rs` | 无 GramJS client → `stream_via_local_api`（Range 透传） |
| `connection.ts` | `canPreviewFiles`（与 download 同 gate） |
| UI | Dashboard / FileExplorer / FileCard / FileListItem / ContextMenu 拆分 `previewReady` |

## 仍有意保留（次要矛盾）

- 上传 / 分享 / 移动 / 拖放 → 仍须 User 会话
- Bot 大文件缩略图走全量下载（无 Telegram thumb sizes）— 可接受，缓存后复用
- 真实 Telegram 预览 E2E 不在 CI（成本）

## 反转条件

- User 在线 → GramJS 预览/流优先路径不变
- 本地 API 未启 / 无 AccessPwd → `previewReady=false`
- 索引无 `file_assets` 行 → 404 与 download 一致

## 下一阶段信号

- 「能看不能分享」→ ShareDialog Bot 分支（R56）

## 验证（2026-06-12）

| 套件 | 结果 |
|------|------|
| vitest | **233 passed**（含 R55 缺口修复 +3） |
| coverage (lib+hooks) | **96.8% stmts / 86.87% branch** |
| cargo `--features headless-server --lib` | **94 passed** |
| cargo `local_api` tests | **2 passed** |
| Playwright 静态 | **41 passed**, 2 skipped |
| Playwright `E2E_API=1` | **43 passed** |

## R55 缺口修复（深度审查后）

| 问题 | 根因 | 修复 |
|------|------|------|
| Bot 预览一点即关 | `useEffect(!sessionOnline)` 清空 preview | 改为 `!previewReady` 关预览；Share 仍 `!sessionOnline` |
| Enter 无法预览 | 快捷键绑 `transferEnabled` | `previewEnabled` / `deleteEnabled` 独立 gate |
| 缺回归测试 | 无快捷键单测 | `useKeyboardShortcuts.test.ts` |

详见 [ROUND-55-GAP-FIX.md](ROUND-55-GAP-FIX.md)
