# R55 计划（桌面 Bot 模式预览/流媒体 parity）

> **主要矛盾**：R54 下载已通，但预览仍 `sessionOnline` 门禁 + `cmd_get_preview` / `/stream/*` 仅 GramJS → 「能下不能看」。

> **牵引性**：Bot 用户首要操作是「点开看图/播视频」，与 Banner「可浏览」语义一致。

> **阶段性**：本回合交付预览/缩略图/流媒体 REST 代理 + UI 门禁；分享/上传/移动仍 User（R56+）。

## TDD 清单

1. RED `canPreviewFiles` / `canOpenPreview` — 与 download 同 gate
2. RED FileCard — `previewEnabled` 在 Bot 下可点
3. GREEN `local_api.rs` — 共享 fetch + `desktop_uses_asset_index`
4. GREEN `cmd_get_preview` / `cmd_get_thumbnail` — REST 落盘 + 现有 base64/路径返回
5. GREEN `stream_media` — 无 GramJS client 时代理本地 API（Range 透传）
6. GREEN Dashboard — `previewReady` 替换 preview 路径上的 `sessionOnline`
7. VERIFY vitest / cargo / playwright

## 反转条件

- User 在线 → GramJS 预览/流路径不变（优先或并列）
- 本地 API 未启 → `previewReady=false`
- 索引无 `file_assets` 行 → 预览仍可能 404（与 download 一致）

## 下一阶段信号

- 「能看不能分享」→ ShareDialog Bot 分支（R56）
