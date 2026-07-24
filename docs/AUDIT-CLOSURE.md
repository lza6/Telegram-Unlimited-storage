> ⚠️ **归档说明（v5.0）**：本文件记录的是 Rust/Tauri 桌面端时代（grammers + actix-web）的审计与修复历史。桌面端及 Rust 后端已在 v5.0 完整移除，后端迁移至 Python（FastAPI + Telethon）。文中所有 `cargo test --features headless-server` 命令、Tauri 拖放（`onDragDropEvent`）等条目**均已失效**，仅作历史存档保留。当前测试入口见 [CLAUDE.md](../CLAUDE.md)（`cd backend && python -m pytest`）。

---

## 第六十三轮修复（多租户授权、Headless 默认收口与交付证据）

> 详表：[FINAL-AUDIT-R63.md](FINAL-AUDIT-R63.md)

- Bulk Telegram 副作用先做逐资源租户授权；HTTP 辅助上传显式携带 owner。
- Headless API_KEY fail-closed，默认 loopback，Compose 仅发布到 loopback；SQLite 生产副本固定为 1。
- 异步上传落盘、令牌日志脱敏、Web/桌面 in-flight 锁、A11y 与小屏布局完成复验。

# 闭环审计与修复记录

> 2026-06-08 深度审查：对照参考项目与用户「一次调用不出逻辑错误」要求。

## 第六十二轮修复（shares_revoked 精确反馈 + 桌面删除中文确认）

> 详表：[FINAL-AUDIT-R62.md](FINAL-AUDIT-R62.md)

| 项 | 落地 |
|----|------|
| `PurgeIndexResult` | 删除返回 `shares_revoked` 计数 |
| `BulkResponse.shares_revoked` | REST bulk delete JSON |
| `DeleteFileResult` | 桌面 `cmd_delete_file` 结构化返回 |
| 删除 toast/confirm | Web/桌面精确「已撤销 N 条」+ 中文确认框 |

### 测试

- `npm test` → **296 passed**
- `cargo test --features headless-server --lib` → **140 passed**
- Playwright → **52 passed**（2 skipped）

## 第六十一轮修复（删除→分享列表跨页闭环 + 删除 UX）

> 详表：[FINAL-AUDIT-R61.md](FINAL-AUDIT-R61.md)

| 项 | 落地 |
|----|------|
| `bumpSharesInvalidateStorage` | Web 删除后 bump localStorage，跨 Tab 通知 |
| `shares-core` 监听 | `storage` / `td-shares-invalidate` / `visibilitychange` |
| 桌面 Sharing 刷新 | `Dashboard` 派发事件 · `SettingsModal` 监听 |
| 删除 toast/confirm | 明确「相关分享链接已一并撤销」 |

### 测试

- `npm test` → **295 passed**
- `cargo test --features headless-server --lib` → **139 passed**
- Playwright smoke+metadata → **51 passed**（2 skipped）

## 第六十轮修复（删除撤销分享 + 分享错误 UX）

> 详表：[FINAL-AUDIT-R60.md](FINAL-AUDIT-R60.md)

| 项 | 落地 |
|----|------|
| `revoke_shares_for_message_id` | 删除/清索引时自动撤销活跃分享 |
| User delete/bulk | 统一 `purge_file_index_entry`（双表 + 撤分享） |
| `sharePure.ts` / `share-pure.js` | Bot 缺映射等错误的可行动中文 |
| Web HTML | `files.html` / `shares.html` 加载 `share-pure.js` |

### 测试

- `npm test` → **292 passed**
- `cargo test --features headless-server --lib` → **140 passed**
- Playwright smoke+metadata → **49 passed**（2 skipped）

## 第五十九轮修复（Bot 双索引一致性 + 分享校验 + 上传中止）

> 详表：[FINAL-AUDIT-R59.md](FINAL-AUDIT-R59.md)

| 项 | 落地 |
|----|------|
| `purge_file_index_entry` | Bot 删除同时清除 `file_assets` + `bot_file_map` |
| `assert_bot_downloadable` | 分享创建/下载校验 Bot 映射 |
| `cmd_move_files` | Bot 模式显式错误（非 "client not connected"） |
| `upload-core.js` | SSE `failed` 时 `AbortController` 中止分片 |
| `DESKTOP-API.md` | Bot 模式限制与 Local API 前提 |

### 测试

- `npm test` → **288 passed**
- `cargo test --features headless-server --lib` → **138 passed**
- Playwright smoke+metadata → **46 passed**（2 skipped）

## 第五十八轮修复（Web readiness 分离 + 终局审计）

> 详表：[FINAL-AUDIT-R58.md](FINAL-AUDIT-R58.md)

| 项 | 落地 |
|----|------|
| `ensureApiAvailable` / `ensureTransportReady` | DB 变更 vs 传输操作分离 |
| `webPure` readiness 纯函数 | 5 tests；镜像 `web-pure.js` |
| `files-core` | Bot 删除/行内分享不绑传输；下载仍绑传输 |
| `shares-core` | 创建/撤销仅 API gate |
| `page-readiness` | 状态点与 `isWebTransportReady` 一致 |

### 测试

- `npm test` → **288 passed**
- coverage → **96.92% stmts / 86.51% branch**
- Playwright smoke+metadata → **49 passed**（2 skipped）

## 第五十七轮修复（Web 元数据 + Share effect + E2E 登记）

> 详表：[ROUND-57-AUDIT.md](ROUND-57-AUDIT.md) · 计划：[ROUND-57-PLAN.md](ROUND-57-PLAN.md) · E2E 登记：[E2E-CHECKPOINTS.md](E2E-CHECKPOINTS.md)

| 项 | 落地 |
|----|------|
| Share 弹窗 effect | `!shareReady` 清 `shareFile`（修复 R56 遗漏） |
| Web SEO/OG | `webMetaPure.ts` + 各 HTML head + `page-meta.js`（`data-td-path` 绝对 canonical） |
| SharingTab | 警告改绑 `shareReady`，Bot 模式不再误报须 User |
| 增量 mock 登记 | `tests/mocks/pass-registry.json` · `web-metadata-r57` → pass |

### 测试

- `npm test` → **241 passed**
- coverage (lib+hooks) → **96.88% stmts / 86.92% branch**
- Playwright `web-metadata.spec.ts` → **4 passed**

## 第五十六轮修复（Bot 模式分享 parity）

> 详表：[ROUND-56-AUDIT.md](ROUND-56-AUDIT.md) · 计划：[ROUND-56-PLAN.md](ROUND-56-PLAN.md) · E2E 登记：[E2E-CHECKPOINTS.md](E2E-CHECKPOINTS.md)

| 项 | 落地 |
|----|------|
| `canShareFiles` / `shareReady` | 与 download 同 gate；ShareDialog + 文件行分享钮解耦 `sessionOnline` |
| Banner | Bot 文案含「分享」；上传/移动仍须 User |

### 测试

- `npm test` → **237 passed**
- coverage (lib+hooks) → **≥96% stmts / ≥86% branch**
- Playwright share 增量 → **3 passed**
- `cargo test --features headless-server --lib` → **94 passed**

## 第五十五轮修复（桌面 Bot 模式预览/流媒体 parity）

> 详表：[ROUND-55-AUDIT.md](ROUND-55-AUDIT.md) · 计划：[ROUND-55-PLAN.md](ROUND-55-PLAN.md)

### 修复

| 项 | 落地 |
|----|------|
| `local_api.rs` | 共享 REST 下载桥（URL、鉴权、落盘、索引模式检测） |
| `cmd_get_preview` / `cmd_get_thumbnail` | Bot 分支经本地 API 缓存 + base64/路径返回 |
| `stream_media` | 无 GramJS client 时代理 `GET /api/v1/files/{id}/download`（Range 透传） |
| `canPreviewFiles` / `previewReady` | 与 download 同 gate；UI 预览钮/缩略图/翻页不再误绑 `sessionOnline` |
| Banner | Bot 文案含「预览」；上传/分享/移动仍须 User |
| **R55 缺口** | `useEffect(!sessionOnline)` 误关预览 → 改 `!previewReady`；快捷键 Enter/Delete 独立 gate |

### 测试

- `npm test` → **233 passed**；coverage **86.87% branch / 96.8% stmts**
- `cargo test --features headless-server --lib` → **94 passed**
- Playwright 静态 **41** + API **43 passed**
- 缺口详述：[ROUND-55-GAP-FIX.md](ROUND-55-GAP-FIX.md)

## 第五十四轮修复（桌面 Bot 模式下载经本地 REST）

> 详表：[ROUND-54-AUDIT.md](ROUND-54-AUDIT.md) · 计划：[ROUND-54-PLAN.md](ROUND-54-PLAN.md)

### 修复

| 项 | 落地 |
|----|------|
| `download_file_via_local_api` | Bot/索引权威时 `cmd_download_file` → `127.0.0.1` REST 流式落盘 + 进度/取消 |
| `canDownload` 门禁 | `useFileDownload` / `useFileOperations` 与 `canDownloadFiles` 对齐 |
| UI `downloadReady` / `deleteReady` | TopBar / FileCard / ContextMenu 下载与删除钮不再误绑 `sessionOnline` |
| Banner | Bot 文案含「下载」；仍标明预览/上传/移动需 User |

### 测试

- `npm test` → **228 passed**；coverage **86.85% branch / 96.8% stmts**
- `cargo test --features headless-server --lib` → **92 passed**
- Playwright 静态 **41** + API **43 passed**

## 第五十三轮修复（桌面 Bot 全局搜索 parity）

> 详表：[ROUND-53-AUDIT.md](ROUND-53-AUDIT.md)

### 修复

| 项 | 落地 |
|----|------|
| `cmd_search_global` | Bot / authoritative index → `search_file_assets` |
| `shouldRebuildIndexBeforeGlobalSearch` | Bot 跳过 GramJS rebuild |
| Dashboard 搜索 | `serviceReady` 门禁；与列表/删除对齐 |

### 测试

- `npm test` → **221 passed**；coverage **86.81% branch / 96.86% stmts**
- `cargo test --features headless-server --lib` → **92 passed**
- Playwright **43 passed**

## 第五十二轮修复（Bulk succeeded_ids + 桌面 Bot 索引浏览）

> 详表：[ROUND-52-AUDIT.md](ROUND-52-AUDIT.md)

### 修复

| 项 | 落地 |
|----|------|
| `BulkResponse.succeeded_ids` | Bot 部分删除精确 deselect；User 全批返回全部 ID |
| `pickBulkSucceededIds` | Web/TS 优先 API 字段，count 推断回退 |
| 桌面 `cmd_get_files` / `cmd_delete_file` | asset index 权威时走 DB（Bot parity） |
| `isServiceReady` / `isBotIndexReady` | 文件列表与索引删除与 GramJS 解耦 |
| `moveExecution.ts` | drag-drop 与 bulk move 共用 |

### 测试

- `npm test` → **219 passed**；coverage **86.76% branch / 96.84% stmts**
- `cargo test --features headless-server --lib` → **92 passed**
- Playwright 静态 **41** + API **43 passed**

## 第五十一轮修复（Bulk count 与选中态 + 拖放移动 parity）

> 详表：[ROUND-51-AUDIT.md](ROUND-51-AUDIT.md) · 扩展方向：[PRODUCT-EXTENSION-IDEAS.md](PRODUCT-EXTENSION-IDEAS.md)

### 修复

| 项 | 落地 |
|----|------|
| `resolveBulkBatchSucceededIds` | count≠batchSize 时不 deselect；partial toast |
| Web bulk UI | 离线可点击 → `ensureReadyForAction` toast |
| 桌面 drag-drop move | per-group try/catch + prune movedOldIds |
| 部分 bulk move | 仅全成功时关闭 Move 模态 |
| 上传 WS | 失败 fallback 到 status poll |

### 测试

- `npm test` → **215 passed**；coverage **87% branch / 97% stmts**（lib+hooks）
- Playwright 静态 **41** + API **43 passed**

## 第五十轮修复（部分批量选中 + 离线拖放 + 桌面 bulk move parity）

> 详表：[ROUND-50-AUDIT.md](ROUND-50-AUDIT.md)

### 修复

| 项 | 落地 |
|----|------|
| Web bulk delete/move | `succeededIds` 逐条 deselect，部分失败保留未成功选中 |
| 桌面 ExternalDropBlocker | 离线 drop → `onUploadBlocked` toast |
| 桌面 handleBulkMove | per-group try/catch + 仅 prune 已移动 ID |
| share-domain / upload poll | 刷新失败 info；poll 失败 err toast |

### 测试

- `npm test`（`app/`）→ **210 passed**
- `cargo test --features headless-server --lib` → **90 passed**
- Playwright 静态 → **38 passed**；API → **40 passed**

## 第四十九轮修复（构建阻断 + 批量 Toast + OpenAPI 契约）

> 详表：[ROUND-49-AUDIT.md](ROUND-49-AUDIT.md)

### 修复

| 项 | 落地 |
|----|------|
| ShareDialog | 补 `toast` import（修复 tsc 构建阻断） |
| Web bulk move/delete | 失败时不弹误导性成功/信息 toast |
| 行内分享 | 创建永久链接前 confirm |
| upload/login/settings | 部分失败、健康检查、域名同步可见化 |
| OpenAPI | `AccessPwdAuth` + Bulk/Network/Settings PUT schema |

### 测试

- `npm test`（`app/`）→ **209 passed**
- `cargo test --features headless-server --lib` → **90 passed**
- Playwright 静态 → **35 passed**；API → **37 passed**

## 第四十八轮修复（桌面 OS 拖放上传 + Web 批量闭环 + OpenAPI）

> 详表：[ROUND-48-AUDIT.md](ROUND-48-AUDIT.md)

### 修复

| 项 | 落地 |
|----|------|
| Tauri `onDragDropEvent` | Finder 拖文件 → `enqueueUploadPaths` |
| `enqueueUploadPaths` | 与 Upload 对话框共用入队逻辑 |
| `DragDropOverlay.tsx` | 删除未挂载死代码 |
| Web bulk delete/move | 部分失败 toast + 移动目标 NaN 校验 |
| `settings-core` metrics | 非 ok HTTP 状态提示 |
| OpenAPI `SettingsResponse` | 含 `effective_share_link_base` |

### 测试

- `npm test`（`app/`）→ **209 passed**
- `cargo test --features headless-server --lib` → **90 passed**
- Playwright 静态 → **32 passed**；API → **34 passed**

## 第四十七轮修复（桌面模态/文件夹 + Web 上传 + 分享基址 API）

> 详表：[ROUND-47-AUDIT.md](ROUND-47-AUDIT.md)

### 修复

| 项 | 落地 |
|----|------|
| 快捷键/Escape/离线 | Share/Settings 模态与 bulk delete 隔离 |
| 文件夹 Open | 网格/列表 Eye 与右键一致 |
| Bot Move 按钮 | TopBar 前置禁用 |
| Web upload | 空选、progress failed、poll、复制、config |
| `effective_share_link_base` | Headless 与桌面分享链接基址正确展示 |

### 测试

- `npm test` → **208 passed**
- `cargo test --features headless-server --lib` → **90 passed**

## 第四十六轮修复（Share 快捷按钮 + Transport 索引闭环）

> 详表：[ROUND-46-TDD.md](ROUND-46-TDD.md)

### 修复

| 项 | 落地 |
|----|------|
| `FileCard` / `FileListItem` | 悬停 Share 按钮（非文件夹） |
| `SettingsModal` | 切换 transport 后 `cmd_invalidate_file_index` + 回调 |
| `Dashboard` | `onTransportSwitched` 刷新 files/api-health 并清全局搜索 |
| `EmptyState` | 去掉误导性「拖放上传」文案 |
| Web `settings-core.js` | 切换后提示本页「重建文件索引」 |

### 测试

- `npm test` → **208 passed**
- `cargo test --features headless-server --lib` → **90 passed**

### 推迟（非 stub）

- Finder 外部拖放直传 → **R48 已落地**（Tauri `onDragDropEvent`）；打包应用内需手动拖放验收

## 第四十五轮修复（全栈深度审查 + 静默失败可见化）

> 详表：[ROUND-45-AUDIT.md](ROUND-45-AUDIT.md)

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R44 后功能已全接线」 | **假** — 刷新/搜索 rebuild、移动夹加载、QR 轮询失败仍静默 |
| 「空 catch 等于非致命可忽略」 | **半真** — 用户会以为「没文件」而非「索引失败」 |
| 「OpenAPI 写 auto 无影响」 | **假** — 客户端按文档发 `auto` 稳定 400 |
| 「外部拖放应直传」 | **产品设计** — `ExternalDropBlocker` 引导 Upload 按钮，非断链 |

### 修复

| 项 | 落地 |
|----|------|
| `webPure` / `files-core.js` | 后台 rebuild 失败 toast |
| `files-core.js` | `loadMoveFolders` 失败 toast |
| `telegram-auth.js` | QR 手动轮询错误展示 |
| `searchPure` + `Dashboard` | 搜索前 rebuild 失败 toast |
| `docs/openapi.json` | transport mode 去掉无效 `auto` |
| `web-smoke.spec.ts` | +3 接线断言 |

### 测试

- `cargo test --features headless-server --lib` → **90 passed**
- `npm test` → **205 passed**
- Playwright 静态 → **32 passed**, **2 skipped**
- Playwright API → **34 passed**

### 已知非缺陷（文档化）

- Bot 模式 `rebuild-index` / bulk move → 显式 400 + UI 守卫
- `cmd_invalidate_file_index` 未从 UI 调用 → rebuild + query invalidate 已覆盖
- 桌面外部拖放 → 阻断并提示
- FileCard 分享 → 右键菜单（无独立按钮）

### 反转条件

- 外部拖放直传 → 改 `ExternalDropBlocker` + `useFileUpload`
- Bot 也要 rebuild → 需索引语义产品定义

## 第四十四轮修复（TransferQueuePanel + vpn try_read 全面化）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「Upload/Download 已共用 transferUiPure 就够」 | **半真**：JSX 仍双份，按钮/进度逻辑易漂移 |
| 「keep-alive/polling try_read 已够」 | **半真**：其余 helper 仍 `blocking_read`，headless 日志仍可能 panic |
| 「抽组件风险大应继续推迟」 | **半真**：hooks 已 199 测稳，现抽 Panel 风险可控 |

### 修复

| 项 | 落地 |
|----|------|
| `TransferQueuePanel.tsx` | 共享队列 UI |
| `UploadQueue` / `DownloadQueue` | 薄包装 + 原有测试全绿 |
| `vpn_optimizer.rs` | `vpn_config`/`proxy_config` + 零 `blocking_read` |

### 测试

- `cargo test --features headless-server --lib` → **90 passed**
- `npm test` → **201 passed**
- Playwright 静态 → **30 passed**, **2 skipped**
- Playwright API → **32 passed**

### 反转条件

- 队列 UI 需强分化 → 扩展 Panel slot，勿复制 JSX

## 第四十三轮修复（hooks 分支 + settings 手动重建索引）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R42 hooks 已够覆盖」 | **半真**：bulkDownload 成功、move 失败、download folder 入队未测 |
| 「download store 恢复 init 测过即可」 | **半真**：remount 时 `initialized` 重置需对称 upload 证伪 |
| 「manual rebuild 只能 files 刷新」 | **真缺口**：切换 transport 后用户不知去哪重建；settings 缺入口 |

### 修复

| 项 | 落地 |
|----|------|
| `useFileOperations.test.tsx` | +5：bulkDownload/folderDownload/move error/zero |
| `useFileDownload.test.tsx` | remount 恢复 pending |
| `settings.html` / `settings-core.js` | 手动重建索引 + `TdWebPure` manual toast |
| `web-smoke.spec.ts` | +2 settings 接线断言 |

### 测试（mock / 本地 headless，无 Telegram 外网）

- `cargo test --features headless-server --lib` → **89 passed**
- `npm test` → **197 passed**
- `npm run test:coverage` → **97.35%** stmts / **85.84%** branches
- Playwright 静态 → **30 passed**, **2 skipped**
- Playwright API → **32 passed**, **0 skipped**

### 反转条件

- Bot 模式也要 rebuild → 需 API 语义明确后再做
- bulk delete session-lost 未调 onSessionError → R44 补测 ✅（R43 已补）

## 第四十二轮修复（静默索引 + User onboarding + remount 测试 + headless 稳定）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R41 下载/移动已闭环 UX」 | **半真**：索引 rebuild 仍每次 toast；User 模式无引导卡片 |
| 「store 恢复已有单测即够」 | **半真**：仅首 mount 测过，remount 时 pending 被 processing 清空未证伪 |
| 「headless vpn panic 无害」 | **半真**：日志 panic 干扰 CI 判读；keep-alive 在 tokio 内 `blocking_read` 根因明确 |

### 修复

| 项 | 落地 |
|----|------|
| `webPure.ts` / `web-pure.js` | rebuild toast 门控 + Bot/User onboarding 纯函数 |
| `files-core.js` | refresh/search 静默 rebuild |
| `dashboard.html` | User onboarding；`await refreshStatus()` |
| `useFileOperations.test.tsx` | +delete/bulkDelete/空文件夹 |
| `useFileUpload.test.tsx` | remount 恢复 pending |
| `vpn_optimizer.rs` | keep-alive/polling 用 `try_read` |
| `web-smoke.spec.ts` | +4 接线断言 |

### 测试（mock / 本地 headless，无 Telegram 外网）

- `cargo test --features headless-server --lib` → **89 passed**
- `npm test` → **191 passed**
- `npm run test:coverage` → **95.92%** stmts / **85.52%** branches
- Playwright 静态 → **28 passed**, **2 skipped**
- Playwright API → **30 passed**, **0 skipped**

### 反转条件

- settings 手动 rebuild 需 toast → 接 `rebuildIndexIfUser('manual')`
- hooks 分支 <80% → R43 补 bulkDownload 成功路径

### 下一阶段主要矛盾转移信号

- 用户反馈「刷新没提示不知道索引有没有更新」→ 仅在 manual/settings 恢复 toast
- 队列 remount 仍丢任务 → 查 processing 态是否误写 store

## 第四十一轮修复（Web 下载进度 + 桌面 bulk move + 连接轮询测试）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R40 Web 下载已用 blob」 | **半真**：无 `Content-Length` 进度反馈，大文件 UX 与 EXTENSION-UX-R37 不符 |
| 「Web bulk move 已守卫即桌面一致」 | **假**：桌面 `Dashboard` 未读 `transport_mode`，拖放/批量移动 Bot 模式仍可触发 |
| 「connection 30s 轮询已存在即稳」 | **半真**：无 fakeTimers 单测，回归不可证伪 |

### 修复

| 项 | 落地 |
|----|------|
| `downloadPure.ts` / `download-pure.js` | 流式读取 + 百分比标签；`readResponseBlobWithProgress` |
| `files-core.js` | 下载按钮随进度更新文案 |
| `useFileOperations.ts` | `guardBulkMove`；opts 注入 |
| `Dashboard.tsx` | `cmd_get_api_health` → bulk move + drag-drop 守卫 |
| `filesPure.ts` | `bulkMoveBlockedMessage(..., 'desktop')` |
| `useFileOperations.test.tsx` | 4 项守卫/成功路径 |
| `useTelegramConnection.test.tsx` | +30s 轮询再探针 |
| `web-smoke.spec.ts` | 下载进度符号；dashboard API/静态双模式就绪 E2E |

### 测试（mock / 本地 headless，无 Telegram 外网）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **182 passed**
- `npm run test:coverage` → **92.46%** stmts / **86.91%** branches
- Playwright 静态（`tests/e2e` `npm test`）→ **24 passed**, **2 skipped**
- Playwright API（`npm run test:api`）→ **26 passed**, **0 skipped**

### 反转条件

- `useFileOperations` 行覆盖 <60% → R42 补 delete/bulkDelete
- dashboard E2E 在真实慢网 flake → 产品 `await refreshStatus()`
- 用户要求全 hooks ≥80% → 扩 `useFileOperations` 删除路径测试

### 下一阶段主要矛盾转移信号

- 报「Bot 模式拖放仍移动」→ 查 `api-health` query 是否 stale
- 报「下载进度卡住」→ 查无 `Content-Length` 时 indeterminate 分支
- 要求桌面队列重启 E2E → R42 Rust/Tauri 集成

## 第四十轮修复（队列持久化 + Sync 成功路径 + Web 就绪 UX）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R39 connection 守卫已测即闭环」 | **假**：`handleSyncFolders` 成功 merge/rebuild、store 恢复 pending 队列仍无单测 |
| 「Playwright dashboard 就绪 E2E 可证伪 UI」 | **假**：`throw new Error('redirect')` 在自动化环境下中断内联脚本，`#tg-status` 永驻「检测中…」 |
| 「mockStore.get 反映 set 后状态」 | **假**：Vitest store mock 无状态，sync 后 `folderIds` 断言失败 |

### 修复

| 项 | 落地 |
|----|------|
| `test-setup.ts` | 有状态 `resetStoreData()`；`set`/`get` 共享 Map |
| `useTelegramConnection.test.tsx` | +sync 成功/无新增、`cmd_check_connection` false → session_lost |
| `useFileUpload/Download.test.tsx` | store 恢复 pending（中文 toast）；zip 失败路径 |
| `dashboard.html` / `upload.html` | `requireLogin()` 正分支包裹初始化，移除 `throw redirect` |
| `web-smoke.spec.ts` | dashboard 就绪：mock API + 显式调用 `TdPageReadiness.refreshUploadReadiness` |

### 测试（mock / 本地 headless，无 Telegram 外网）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **173 passed**
- `npm run test:coverage` → **96.81%** stmts / **87.41%** branches；hooks **96.03%** lines
- Playwright 静态（`tests/e2e` `npm test`）→ **26 passed**, **0 skipped**
- Playwright API（`npm run test:api`，需释放 `:1334`）→ **26 passed**, **0 skipped**（health 200 + rebuild-index 401 实跑）

### 反转条件

- `test:api` 端口占用 → 先结束占用 `:1334` 的 serve/headless 进程
- hooks 分支仍 ~81% → R41 补 zip 未启用、dialog 取消等边角
- Web 下载无 determinate 进度 → 见 `EXTENSION-UX-R37.md`

### 下一阶段主要矛盾转移信号

- 用户报「重启后队列丢失」→ 加 Rust/Tauri store 集成测或 E2E 桌面端
- 要求全 `app/src` ≥80% → 扩 UI 组件测试或收窄 coverage include

## 第三十九轮修复（Connection Hook 测试 + E2E API 管线）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R38 hooks 已闭环传输层」 | **半真**：`useTelegramConnection` 的 online/offline/session 与 requireOnline 守卫仍无 mock 测试 |
| 「Playwright webServer 已自动起静态站」 | **半真**：health/rebuild-index 在纯静态模式下仍 skip；需 headless User 模式 |
| 「connection 纳入 coverage 即达标」 | **假**：`useTelegramConnection.ts` 仅 ~59% lines，sync/create/delete 成功路径未测 |

### 修复

| 项 | 落地 |
|----|------|
| `useTelegramConnection.test.tsx` | 9 项：online/session_lost/offline/store 恢复/sync 守卫/logout/confirm/activeFolder |
| `scripts/e2e-headless-server.ps1` | User 模式 headless（无 Bot getMe），临时 DATA_DIR + ACCESS_PWD=test |
| `playwright.config.cjs` | `E2E_API=1` 起 headless；静态模式用本地 `serve` devDep |
| `tests/e2e/package.json` | `test:api` 脚本；`serve@14.2.4` devDep |
| `vitest.config.ts` | coverage 纳入 `useTelegramConnection.ts` |
| 文档 | `ROUND-39-TDD.md` |

### 测试（mock / 本地 headless，无 Telegram 外网）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **158 passed**
- `npm run test:coverage` → **86.37%** stmts / **84.35%** branches；hooks **77.19%** lines（connection ~59% 拉低）
- Playwright 静态（`npm test` in `tests/e2e`）→ **24 passed**, **2 skipped**
- Playwright API（`npm run test:api`）→ 见下方验证

### 反转条件

- `useTelegramConnection` coverage <50% 且用户报连接 bug → R40 补 sync/create/delete 成功路径
- headless 启动 >300s → 预编译 `cargo build --bin telegram-drive-server`
- Windows `playwright.config.ts` ESM 报错 → 已改 `playwright.config.cjs`

### 下一阶段主要矛盾转移信号

- 用户报「同步文件夹失败」→ 补 `handleSyncFolders` 成功 renderHook + Rust mock DB
- 要求 hooks 整体 ≥85% → 扩 connection 测试或从 coverage include 暂时排除未测 handler

## 第三十八轮修复（Hooks 集成测试层 + classifyDownloadFailure）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R37 纯函数已闭环」 | **半真**：桌面 `useFileUpload`/`useFileDownload` 状态机无 renderHook 覆盖，cancel/retry/dialog 路径可漂移 |
| 「下载错误分类与上传对齐」 | **假**：`useFileDownload` 内联 if/else，未复用可测纯函数 |
| 「Vitest 门槛代表全应用质量」 | **假**：此前 coverage include 不含 hooks |

### 修复

| 项 | 落地 |
|----|------|
| `classifyDownloadFailure` | `downloadPure.ts` + `useFileDownload` 接入 |
| `test-setup.ts` | `vi.hoisted` mock invoke/listen/dialog/store |
| `hookWrapper.tsx` | QueryClient + SettingsProvider 测试包装 |
| `useFileUpload.test.tsx` | 14 项：success/错误分类/session/cancel/retry/dialog/进度 |
| `useFileDownload.test.tsx` | 13 项：queue/bulk/save 取消/cancelAll/进度/session |
| `vitest.config.ts` | coverage 纳入 hooks；门槛 80%（用户最低要求） |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib`（`app/src-tauri`）→ **88 passed**
- `npm test` → **149 passed**
- `npm run test:coverage` → **93.7%** stmts / **85.45%** branches；hooks **88.42%** lines
- Playwright（`serve deploy/web:1334`）→ **24 passed**, **2 skipped**

### 反转条件

- hooks 分支覆盖率仍 ~75% → R39 补 store 恢复、zip 失败、dialog 异常路径
- Playwright 需手动起 serve → 可加 `playwright.config` `webServer` 自动启动

### 下一阶段主要矛盾转移信号

- 用户报「连接状态不对」→ R39 `useTelegramConnection` renderHook
- 要求 E2E API 全绿 → `E2E_API=1` + headless 二进制

## 第三十七轮修复（downloadPure 提取 + 上传错误分类 + webPure 安全收紧）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「R36 Web 下载已闭环」 | **半真**：逻辑在 `files-core.js` 内联，无法 mock 单测，与桌面 `downloadPure` 脱节 |
| 「上传错误处理已稳定」 | **半真**：`useFileUpload` 内 if/else 链，同类错误在 download hook 可能漂移 |
| 「safeNext/safeHttpUrl 已防开放重定向」 | **假**：`not a url %%` 会被解析为同源路径；`:::bad` 可被拼成 href |
| 「纯函数覆盖率已拉满」 | **假**：`uploadPure`/`filesPure` 登录 URL/`webPure` catch 分支未覆盖 |

### 修复

| 项 | 落地 |
|----|------|
| `downloadPure.ts` + `download-pure.js` | Web 下载防重复、按钮态、文件名、toast 纯函数 |
| `files-core.js` | 接入 `TdDownloadPure` |
| `files.html` | script 顺序 `download-pure.js` → `files-core.js` |
| `uploadPure.classifyUploadFailure` | `useFileUpload` 错误分类统一 |
| `webPure.ts` + `web-pure.js` | `safeNext` 必须 `/` 开头；`safeHttpUrl` 必须 `http(s)://` 前缀 |
| 测试 | 新增/扩充 download/upload/files/queue/transfer/web 单测 |
| 文档 | `ROUND-37-TDD.md`、`EXTENSION-UX-R37.md` |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **119 passed**
- `npm run test:coverage` → **97.73%** statements / **93.1%** branches（`src/lib` 语句 **100%**）
- Playwright（`deploy/web` 静态 serve:1334）→ **24 passed**, **2 skipped**（health/rebuild-index 需 Headless API）

### 反转条件

- 若 `safeHttpUrl` 需支持相对 `/api/...` 链接 → 单独 `safeRelativeApiPath`，不放宽 `safeHttpUrl`
- Hooks 层仍无 render 集成测试 → 第三十八轮主攻（见 EXTENSION-UX-R37 B）

### 下一阶段主要矛盾转移信号

- 用户反馈「上传失败文案不对」→ 检查 `classifyUploadFailure` 与 Rust 错误字符串是否对齐
- CI 要求全 app 80% 覆盖率 → 扩展 `vitest.config` include 至 hooks

## 第三十六轮修复（Web 下载防重复 + 批量移动守卫统一 + DownloadQueue 测试对齐）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「Web 批量移动已用 TdFilesPure」 | **半真**：`updateBulkUi` 已用纯函数，但点击处理仍硬编码 `transportMode !== 'user'` 与独立中文文案 |
| 「Web 单文件下载有闭环」 | **半真**：有 `ensureReadyForAction` + blob，但无进行中态，连点会并发拉取 |
| 「DownloadQueue 与 UploadQueue 行为一致」 | **半真**：Clear Finished 逻辑相同，但下载侧未用 `countSuccessTransfers` 且缺对应单测 |

### 修复

| 项 | 落地 |
|----|------|
| `files-core.js` | `downloadingIds` 防重复；按钮「下载中…」+ toast；`finally` 恢复 |
| `files-core.js` | 批量移动点击走 `TdFilesPure.canBulkMoveInTransportMode` / `bulkMoveBlockedMessage` |
| `DownloadQueue.tsx` | `completedCount` → `countSuccessTransfers` |
| `DownloadQueue.test.tsx` | Clear Finished 仅 success 时显示 |
| Playwright | 下载防重复 + 移动守卫静态断言 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **102 passed**
- `npm run test:coverage` → **94.59%** statements / **88%** branches（纯函数门槛通过）

### 反转条件

- Web 下载仍为 blob 全量拉取，大文件无 determinate 进度（EXTENSION-ROADMAP 6.4）
- 桌面端批量移动不读 REST `transport_mode`（Grammers 直连，Bot 仅影响 Headless/Web REST）

## 第三十五轮修复（队列 Clear UX 对齐 + 搜索空态 + Web 传输守卫 + 分享撤销就绪检查）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「上传 Clear Finished 已与下载一致」 | **半真**：逻辑只清 success，但按钮在无 success 时仍显示，误导用户 |
| 「桌面/Web 搜索无结果有统一空态」 | **假**：桌面 FileExplorer 用裸文本；Web 搜索与空文件夹同文案 |
| 「Bot 模式移动禁用逻辑可单测」 | **假**：Web `files-core` 硬编码 `transportMode === 'user'` |
| 「分享撤销未校验服务就绪」 | **真**：`shares-core` 撤销 DELETE 未走 `ensureReadyForAction` |

### 修复

| 项 | 落地 |
|----|------|
| `queuePure.ts` | `countSuccessTransfers` / `hasClearableFinishedTransfers` |
| `UploadQueue.tsx` | 「Clear Finished」仅在 `completedCount > 0` 时显示（与 DownloadQueue 一致） |
| `FileExplorer.tsx` | 全局搜索无结果使用 `EmptyState variant="search"` |
| `filesPure.ts` + `files-pure.js` | `canBulkMoveInTransportMode` / `bulkMoveBlockedMessage` |
| `files-core.js` | 搜索空态独立文案；批量移动 UI 走 `TdFilesPure` 守卫 |
| `shares-core.js` | 撤销前 `ensureReadyForAction` |
| Playwright | 下载 blob、搜索空态、传输守卫、分享撤销 wiring 静态断言 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **101 passed**
- `npm run test:coverage` → **94.43%** statements / **87.5%** branches（纯函数门槛通过）

### 反转条件

- 桌面原生 `cmd_move_files` 不读 REST transport 模式（User 会话直连 Grammers）；Bot 仅影响 Headless/Web REST
- Web 单文件下载仍为 blob 全量拉取，大文件无 determinate 进度条（见 EXTENSION-ROADMAP 6.4 剩余项）

## 第三十四轮修复（传输 UI 纯函数 + 上传 Clear 对齐 + 流媒体缓冲态 + CI 覆盖率）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「上传 Clear Finished 与下载一致」 | **假**：上传会清掉 error/cancelled，无法重试；下载只清 success |
| 「队列 UI 字节格式化可单测」 | **假**：Upload/DownloadQueue 各有一份 `formatBytes` |
| 「流媒体只有 Preparing…」 | **半真**：无 buffering 态与 `onWaiting` 反馈 |
| 「CI 未跑 coverage 门槛」 | **假**：`docker-api.yml` 仅 `npm test` |

### 修复

| 项 | 落地 |
|----|------|
| `transferUiPure.ts` | 字节格式化、进度文案、`deriveStreamUiPhase` / `streamStatusMessage` |
| `connection.ts` | `classifyConnectionStatus`；`useTelegramConnection` 接入 |
| `useFileUpload.ts` | `clearFinished` → `filterClearFinishedTransfers`（与下载一致） |
| `UploadQueue` / `DownloadQueue` | 共用 `transferUiPure` + `hasActiveTransfers` |
| `MediaPlayer.tsx` | buffering overlay；切换文件重置 stream；`onError` 显式报错 |
| `docker-api.yml` | CI 增加 `npm run test:coverage` |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **97 passed**
- `npm run test:coverage` → 纯函数层门槛通过

### 反转条件

- MediaPlayer 缓冲态依赖浏览器 `waiting` 事件，极短片段可能看不到 overlay
- hooks 全库 80% 仍未达成（queue/connection 逻辑已纯函数化）

## 第三十三轮修复（传输队列纯函数 + hooks 接线 + 安装包捆绑 Web + 桌面编译修复）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「cancel/retry 逻辑可单测」 | **假**：内联在 `useFileUpload`/`useFileDownload`，Vitest 无法覆盖 |
| 「DownloadQueue 有 UI 测试」 | **假**：仅 UploadQueue 有 component 测试 |
| 「安装包 8550 有 telegram.html」 | **假**：仅 dev 态 manifest 路径可解析，release 无 bundle |
| 「桌面 `cargo check` 可编译」 | **假**：`DbConnection` 未导入、`from_env` 返回值误当 Arc |

### 修复

| 项 | 落地 |
|----|------|
| `queuePure.ts` | cancel/retry/slots/pending 选择 + Vitest |
| `useFileUpload.ts` / `useFileDownload.ts` | 接入纯函数，行为不变 |
| `DownloadQueue.test.tsx` | 下载队列 UI 冒烟测试 |
| `tauri.conf.json` | `deploy/web/**` → bundle `web/` |
| `server_config.rs` | `resolve_desktop_web_static_dir(resource_dir)` 优先 Tauri 资源 |
| `lib.rs` | `DbConnection` 导入；`from_env` clone 修复；API 传入 `resource_dir` |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **88 passed**
- `npm test` → **87 passed**（+7 queuePure + DownloadQueue）
- `npm run test:coverage` → 纯函数层含 `queuePure` 门槛通过
- `cargo check`（桌面默认 feature）→ 编译通过

### 反转条件

- hooks 本体仍依赖 Tauri invoke，全库 80% 需 render/integration 测试（渐进）
- 流媒体 UX banner（EXTENSION-ROADMAP 6.4）MediaPlayer 已有「Preparing stream…」，未加 determinate 进度

## 第三十二轮修复（桌面 REST 静态 Web + User 登录优先 8550 + 纯函数覆盖率）

### 自反驳（本轮前）

| 反驳 | 结论 |
| 「8550 能打开 telegram.html」 | **假**：`for_desktop_api` 的 `static_dir` 指向 `data_dir`，`desktop_api_server` 无 `actix_files` |
| 「切 User 只依赖 1334」 | **半真**：Settings 只探 Headless health，纯桌面用户未启 Docker 时困惑 |
| 「EXTENSION-ROADMAP 6.2/6.3 已做」 | **假**：文档标 P2/P3 待办 |
| 「全库 80% 覆盖率」 | **假**：hooks 仍无 render 测试 |

### 修复

| 项 | 落地 |
|----|------|
| `server_config.rs` | `resolve_desktop_web_static_dir` / `desktop_static_servable` + Rust 单测 |
| `desktop_api_server.rs` | 解析到 `deploy/web` 时挂载静态（API 路由优先） |
| `filesPure.ts` | `buildDesktopApiTelegramLoginUrl` / `buildTelegramLoginCandidates` |
| `SettingsModal.tsx` | 切 User 后按序探测 8550→1334 的 `telegram.html` 并 `open` |
| `vitest.config.ts` + `package.json` | `@vitest/coverage-v8`；`test:coverage` 纯函数 90% 门槛 |
| `DESKTOP-API.md` / `EXTENSION-ROADMAP.md` | Scheme B + 覆盖率节更新 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **88 passed**（+2 static resolve）
- `npm test` → **76 passed**（+2 login candidates）
- `npm run test:coverage` → 纯函数层门槛通过

### 反转条件

- 正式安装包若未捆绑 `deploy/web`，8550 仍无静态页——**第三十三轮**已通过 Tauri bundle `web/` 缓解；dev 仍用 manifest 路径
- hooks 本体仍依赖 Tauri invoke，全库 80% 需 render/integration 测试（第三十三轮后 queue 逻辑已纯函数化）

## 第三十一轮修复（桌面全局搜索索引重建 + 批量删除保留失败选中）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「桌面/Web 搜索行为一致」 | **假**：Web Enter 前 rebuild-index；桌面 debounce 直接 `cmd_search_global`，索引可能陈旧 |
| 「批量删除部分失败可重试」 | **假**：`setSelectedIds([])` 清空全部，失败项无法一键重删 |
| 「useFileOperations 搜索已接线」 | **假**：`handleGlobalSearch` 导出但 Dashboard 未使用（死代码） |
| 「全库 80% 覆盖率」 | **假**：仍未达成 |

### 修复

| 项 | 落地 |
|----|------|
| `searchPure.ts` | `shouldRebuildIndexForGlobalSearch` / `buildRebuildFolderIds` + Vitest |
| `Dashboard.tsx` | 首次进入全局搜索（≥3 字）时 `cmd_rebuild_file_index`，再搜索 |
| `utils.ts` | `pruneSelectedIdsAfterDelete`；批量删后保留失败项选中 |
| `useFileOperations.ts` | 移除未使用的 `handleGlobalSearch` |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **86 passed**
- `npm test` → **74 passed**（+4 searchPure/pruneSelectedIds）

### 反转条件

- 全局搜索 rebuild 仅在「首次进入搜索态」触发一次，长查询改写不重复 rebuild（与 Web Enter 语义对齐）
- 8550 静态 telegram 登录页 → **第三十二轮已做**（见上）

## 第三十轮修复（move remap 失败即报错 + Web Bot 禁移动 + hooks 纯函数）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「move 后索引 remap 失败用户可知」 | **假**：仅 `log::warn`，前端仍 toast 成功 |
| 「Web Bot 点移动有清晰反馈」 | **假**：REST 400，按钮仍可点，错误晦涩 |
| 「上传完成当前文件夹必刷新」 | **半真**：仅 invalidate `['files', folderId]`，切换 folder 时可能漏刷 |
| 「hooks 可单测」 | **假**：队列构建内联在 hook，Vitest 无法覆盖 |
| 「全库 80% 覆盖率」 | **假**：仍未达成（持续项） |

### 修复

| 项 | 落地 |
|----|------|
| `commands/fs.rs` | remap 失败返回 `Err`，不再静默成功 |
| `api_routes.rs` | bulk move remap 失败 → `MOVE_REMAP_FAILED` |
| `downloadPure.ts` | `buildBulkDownloadItems` + Vitest |
| `uploadPure.ts` | `buildUploadQueueEntries` + Vitest |
| `useFileDownload.ts` / `useFileUpload.ts` | 接入纯函数；上传后 invalidate `['files']` |
| `files-core.js` | Bot 模式禁用/拦截批量移动 + `transportMode` 探测 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **86 passed**
- `npm test` → **70 passed**（+5 downloadPure/uploadPure）

### 反转条件

- remap 失败时 Telegram 消息已移动但接口报错——用户需手动刷新/重建索引（优于静默索引漂移）
- 桌面全局搜索仍不自动 rebuild（需手动 Sync）；Web 搜索 Enter 仍会 rebuild

## 第二十九轮修复（forward 数量校验 + Web 批量移动 + planMoveGroups）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「move 失败时不会删源消息」 | **假**：forward 返回数 < 请求数时仍 delete 源消息，存在数据丢失风险 |
| 「Web bulk move 已闭环」 | **假**：REST `action=move` 有，但 `files.html` 无 UI |
| 「moved 计数可信」 | **假**：`cmd_move_files` 曾固定 `moved=message_ids.len()` 即使 remap 失败 |
| 「Dashboard/Hook 移动逻辑一致」 | **半真**：两处重复 group+skip 逻辑，易漂移 |
| 「全库 80% 覆盖率已达成」 | **假**：hooks 仍几乎无单测（下轮继续） |

### 修复

| 项 | 落地 |
|----|------|
| `commands/fs.rs` | forward 数量校验通过后才 delete；`moved=new_ids.len()` |
| `api_routes.rs` | bulk move 同样校验；返回 `MOVE_FORWARD_MISMATCH` |
| `filesPure.ts` / `files-pure.js` | `buildBulkMovePayloads` + Vitest |
| `utils.ts` | `planMoveGroups` 供桌面 Hook/拖放共用 |
| `useFileOperations.ts` / `Dashboard.tsx` | 改用 `planMoveGroups` |
| `files.html` / `files-core.js` | 批量移动 UI（文件夹下拉 + 移动按钮 + 服务就绪禁用） |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **86 passed**
- `npm test` → **65 passed**（+4 filesPure/planMoveGroups/mergeMovePayloads）

### 反转条件

- forward 数量不一致时现在 **直接报错且不删源**——需在 live User 环境确认 Telegram 始终 1:1 返回
- Web 移动后刷新列表，不做客户端 ID remap（与桌面不同，可接受）
- 全库 80% 覆盖率仍未达成

## 第二十八轮修复（cmd_move_files 回传新 ID + UI remap + 分享/上传细节）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「r27 移动后只需重搜」 | **半真**：旧 id 被 prune，但 **预览/分享仍可能持有 stale id**；未回传 new id |
| 「cmd_move_files 已 remap DB」 | **真**（Rust），但前端 **仍返回 bool**，搜索/ShareDialog 无法接续操作 |
| 「Share 弹窗移动后安全」 | **假**：`closePreviewIfMoved` 未覆盖 `shareFile` |
| 「Web 删后 selectedMeta 干净」 | **假**：`clearSelection` 全清，但部分路径可能遗留；删后应 `forgetFileMeta` |
| 「上传 folder 未就绪仍可点」 | **假**：`page-readiness` 只禁 upload 按钮 |

### 修复

| 项 | 落地 |
|----|------|
| `commands/fs.rs` | `MoveFilesResult`（old/new ids + targetFolderId）+ serde 单测 |
| `utils.ts` | `remapMovedFilesInList` / `remapOpenFileAfterMove` / `mergeMovePayloads` |
| `useFileOperations.ts` + `Dashboard.tsx` | 移动后 remap 搜索/预览/Share，不再仅 prune |
| `SettingsModal.tsx` | 桌面切 transport 提示索引重置 |
| `files-core.js` | 批量删除后 `forgetFileMeta` |
| `page-readiness.js` | 未就绪时禁用 `#upload-folder` |
| `utils.test.ts` | remap 单测 +2 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **86 passed**（含 `move_files_result_serializes_camel_case`）
- `npm test` → **61 passed**

### 反转条件

- forward 返回 id 数量与源 id 不一致时，`remapOpenFileAfterMove` 返回 null（关闭预览）——需 live 验证极端 Telegram 响应

## 第二十七轮修复（移动后 stale message_id + 传输切换索引提示）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「删后搜索列表已闭环」 | **半真**：r26 修了 delete，**move 后旧 message_id 仍留在 searchResults** |
| 「移动后预览安全」 | **半真**：`closePreviewIfMoved` 在 modal 回调里，**未 prune 搜索列表** |
| 「拖放移动与批量移动一致」 | **假**：拖放未调 `handleFilesMoved`，全局搜索留幽灵行 |
| 「move 0 文件有正反馈」 | **假**：仍 toast success “Moved 0 files” |
| 「Web 切 transport 用户知索引失效」 | **假**：`set_file_index_complete(false)` 后端有，前端无提示 |

### 修复

| 项 | 落地 |
|----|------|
| `utils.ts` | `filterFilesExcludingIds` + Vitest |
| `useFileOperations.ts` | `onFilesMoved`；moved=0 时 `toast.info` |
| `Dashboard.tsx` | `handleFilesMoved` prune 搜索/预览；拖放移动接线 |
| `settings-core.js` | 切 transport toast「索引已重置，请刷新重建」 |
| `web-smoke.spec.ts` | `selectedMeta` + 索引重置文案断言 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **86 passed**
- `npm test` → **59 passed**

### 反转条件

- 移动后新 message_id 未回传前端，用户需重新搜索才能操作目标 folder 中的文件（已 toast 说明）
- `cmd_move_files` 仍返回 `bool` 而非新 id 列表——完整 id 映射属后续增强

## 第二十六轮修复（搜索删后 UI + 上传 folder 闭环 + merge_chunks）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「全局搜索删文件后列表即时更新」 | **假**：`onFilesRemoved` 仅关预览，未 prune `searchResults` / `previewContextFiles` |
| 「Web 上传可选 folder」 | **假**：`EXTENSION-ROADMAP` §6.1 标 P2 但未落地；legacy `/upload` 虽支持 `folder_id` 前端未传 |
| 「大文件分片上传尊重 folder」 | **假**：`merge_chunks` 硬编码 `folder_id=None`，与单文件上传不一致 |
| 「桌面切 User 盲目 open 1334」 | **假**：未探测 Headless 是否运行，纯桌面用户得到死链体验 |
| 「全局搜索无结果有反馈」 | **假**：`isGlobalSearch` 传入 FileExplorer 但未用于空态文案 |

### 修复

| 项 | 落地 |
|----|------|
| `Dashboard.tsx` | `handleFilesRemoved` prune 搜索列表与预览上下文 |
| `FileExplorer.tsx` | 全局搜索空结果中文提示 |
| `legacy_routes.rs` | `merge_chunks` 读取并传递 `folder_id` |
| `legacy_form.rs` | `parse_optional_i64_field` + 单测 |
| `upload-core.js` | `folderSelectSelector` → `/upload` + `/merge_chunks` |
| `upload-folder.js` | `GET /api/v1/folders` 填充下拉 |
| `dashboard.html` / `upload.html` | 目标文件夹 UI |
| `uploadPure.ts` | `parseUploadFolderId` + Vitest |
| `SettingsModal.tsx` | 切 User 前先 probe `:1334/health` |
| `utils.test.ts` | `fileBelongsToFolder` / `idsIncludeOpenFile` |
| `web-smoke.spec.ts` | upload folder 静态断言 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **86 passed**
- `npm test` → **58 passed**

### 反转条件

- Bot 模式选 channel 上传仍依赖 Telegram 权限（live）；chunk 路径已传 `folder_id` 但需在用户环境验证大文件
- 全库 80% 覆盖率仍未达成

## 第二十五轮修复（filesPure 可测纯函数 + 桌面 User 登录引导）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「bulk delete / download URL 有单测保障」 | **假**：逻辑内联在 `files-core.js`，Vitest 无法覆盖，仅 Playwright 字符串断言 |
| 「桌面切 User 后用户知道去哪登录」 | **假**：第二十四轮仅有 confirm 文案，无自动打开登录页 |
| 「Peer::Chat 搜索映射已验证」 | **假**：仅 User/Channel 单测，Chat 未覆盖 |
| 「全库已达 80% 覆盖率」 | **假**：49 个 Vitest 仅覆盖部分 utils/组件，hooks 几乎未测 |

### 修复

| 项 | 落地 |
|----|------|
| `app/src/lib/filesPure.ts` | `buildBulkDeletePayloads` / `buildFileDownloadUrl` / `buildTelegramLoginUrl` |
| `deploy/web/assets/files-pure.js` | Web 镜像 + `files.html` 引入 |
| `files-core.js` | 改用 `TdFilesPure.*` |
| `SettingsModal.tsx` | 切 User 后 `shell.open` Headless `:1334/telegram.html` + 备选 toast |
| `commands/utils.rs` | `Peer::Chat` → `None` 单测 |
| `docs/ROUND-25-TDD.md` | 第一性原理 + TDD 清单 + 索引未完成搜索行为表 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **85 passed**
- `npm test` → **54 passed**（+5 filesPure）

### 反转条件 / 下轮信号

- **反转**：若 `files-pure.js` 与 `filesPure.ts` 再次分叉且 Vitest 未扩到 Web 镜像，则本轮测试价值衰减
- **转移**：Web 上传 `folder_id` UI、hooks 层覆盖率、live SearchGlobal 用户反馈

## 第二十四轮修复（SearchGlobal Saved Messages folder_id + 删 folder 清理）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「全局搜索 Saved Messages 可删/预览」 | **假**：SearchGlobal `Peer::User` 被映射为 `Some(user_id)`，与 `folder_id=None` 约定不一致 |
| 「删 folder 后预览仍安全」 | **假**：预览/分享弹窗未在删 folder 后关闭 |
| 「桌面 REST 切 User 有登录引导」 | **假**：第二十三轮加了按钮但无确认与 Web 登录提示 |
| 「Web 搜索前索引已 fresh」 | **假**：Enter 搜索未 rebuild，complete 索引可能陈旧 |

### 修复

| 项 | 落地 |
|----|------|
| `commands/utils.rs` | `telegram_peer_id_to_folder_id`（User/Chat→None，Channel→id）+ 单测 |
| `commands/fs.rs` | `cmd_search_global` Telegram 路径改用统一映射 |
| `Dashboard.tsx` | 删 folder 后关闭预览/分享态 |
| `SettingsModal.tsx` | transport 切换确认 + User 模式 Web 登录提示 |
| `files-core.js` | 搜索 Enter 前先 `rebuildIndexIfUser` |
| `web-smoke.spec.ts` | 搜索 rebuild 静态断言 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **84 passed**
- `npm test` → **49 passed**

## 第二十三轮修复（folder 作用域 + 预览/删除/移动 peer 对齐）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「Web 批量删除安全」 | **假**：未传 `folder_id`，User 模式在 Saved Messages peer 删 channel 文件 → 错删/失败 |
| 「全局搜索删/预览/下载正确」 | **假**：仍用 `activeFolderId`，非当前 folder 的文件全链路 peer 错误 |
| 「移动后预览仍可用」 | **假**：forward 新 message_id，未关闭 Preview/Media/PDF |
| 「跨 folder 批量移动正确」 | **假**：多源 folder 仍单 `sourceFolderId` |
| 「桌面 Settings 无法切 transport」 | **假**：仅展示 health，无切换（Web 有） |
| 「桌面 upload 后 REST list 即时」 | **假**：`cmd_upload_file` 未 invalidate metadata cache |

### 修复

| 项 | 落地 |
|----|------|
| `utils.ts` | `resolveFileFolderId` + `groupIdsBySourceFolder` |
| `useFileOperations.ts` | 删/移按 file.folder_id 分组 |
| `useFileDownload.ts` | 批量下载用各文件 folder_id |
| `PreviewModal` / `MediaPlayer` / `PdfViewer` / `FileCard` | peer 用 `file.folder_id ?? activeFolderId` |
| `Dashboard.tsx` | 下载/拖放移动分组；移动后关闭预览 |
| `files-core.js` | `bulkDeleteByFolder` 按 folder 分组调 bulk API |
| `commands/fs.rs` | 桌面 upload 后 invalidate metadata cache |
| `SettingsModal.tsx` | REST API 运行时 transport 切换按钮 |
| `openapi.json` | bulk 请求体文档化 `folder_id` |
| `utils.test.ts` / `web-smoke.spec.ts` | folder 分组单测 + 静态断言 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **82 passed**
- `npm test` → **49 passed**（+3 folder 分组）

## 第二十二轮修复（索引生命周期 + REST 一致性 + 幽灵行 purge）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「Sync 后索引永远准确」 | **假**：rebuild 只 upsert 不 purge，已删文件/文件夹仍留 ghost 行 |
| 「complete 标志会随结构变化失效」 | **假**：logout/新建/删 folder/切 transport 均不清 complete |
| 「REST list/search/get 规则一致」 | **假**：list 仍 `multi_tenant\|\|Bot`，search/get 用 authoritative |
| 「Web User 批量删除同步索引」 | **假**：User bulk delete 未 `delete_file_asset` |
| 「REST User 上传可搜索」 | **假**：`http_upload` User 路径未 `record_uploaded_file` |
| 「Web 能重建索引」 | **假**：`cmd_rebuild_file_index` 仅 Tauri；Web 无 REST |
| 「移动后弹窗会关」 | **假**：`MoveToFolderModal` 成功未 `onSuccess` 关闭 |
| 「Sync 刚建 folder 必入索引」 | **假**：`handleSyncFolders` 用陈旧 React `folders` state |

### 修复

| 项 | 落地 |
|----|------|
| `db.rs` | `delete_all_file_assets_for_owner` / `delete_file_assets_in_folder` |
| `commands/fs.rs` | rebuild 先 purge owner 再扫；`cmd_invalidate_file_index`；create/delete folder 清 complete |
| `commands/auth.rs` | logout 清 `file_index_complete` |
| `auth_routes.rs` | transport 切换清 complete |
| `api_routes.rs` | list 对齐 `uses_asset_index`；User bulk delete 删索引+invalidate cache；`POST rebuild-index` |
| `http_upload.rs` | User 上传写 `file_assets` |
| `useTelegramConnection.ts` | Sync 读 store 防 race；create/delete folder invalidate `['files']` |
| `Dashboard.tsx` | 移动成功关闭弹窗 |
| `files-core.js` | User 刷新重建索引；批量删除确认文案按 transport 区分 |
| `route_registry.rs` / `openapi.json` | 注册 rebuild-index |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **82 passed**
- `npm test` → **46 passed**
- Playwright smoke 新增 `rebuild-index` 鉴权 + `files-core` 接线断言

## 第二十一轮修复（索引完整性标志 + Sync 全量重建 + 部分索引陷阱）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「打开文件夹后全局搜索已完整」 | **假**：懒索引只写当前 folder，`cmd_search_global` 在 `count>0` 时走 DB，漏掉未打开 folder |
| 「Web API get/search 与桌面一致」 | **假**：`uses_asset_index` 在 User 模式 `count>0` 时强制 DB，未索引文件 GET 直接 404 |
| 「Sync 会同步文件索引」 | **假**：`handleSyncFolders` 只扫 channel 列表，不重建 `file_assets` |
| 「删除后跨 folder 缓存正确」 | **假**：delete 仍只 invalidate 当前 `activeFolderId`（move 第二十轮已改全量） |
| 「设置页切 User 有登录引导」 | **假**：`switchMode('user')` 仅 toast，无跳转 `telegram.html` |

### 修复

| 项 | 落地 |
|----|------|
| `db.rs` | `app_meta.file_index_complete` + `is/set_file_index_complete` |
| `file_access.rs` | `asset_index_authoritative`（User 仅 complete 后信任 DB） |
| `api_routes.rs` / `commands/fs.rs` | search/get 改用 authoritative 标志；`cmd_search_global` 对齐 |
| `commands/fs.rs` | `list_document_files_in_folder` + `cmd_rebuild_file_index` |
| `useTelegramConnection.ts` | Sync 后全 folder 重建索引并 invalidate `['files']` |
| `useFileOperations.ts` | delete/bulk delete invalidate 全部 `['files']` |
| `settings-core.js` | 切 User 模式跳转 `/telegram.html?next=/settings.html` |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **81 passed**
- `npm test` → **46 passed**

## 第二十轮修复（移动索引 remap + 懒索引 + 缓存刷新）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「User 索引已覆盖历史文件」 | **假**：仅新上传写索引，打开文件夹不会补历史 |
| 「移动后 search 仍准确」 | **假**：forward 产生新 message_id，索引仍指向已删旧 id |
| 「移动后目标文件夹 UI 正确」 | **假**：只 invalidate 源 folder，目标 folder 缓存陈旧 |
| 「Web API bulk move 索引一致」 | **假**：User 模式 move 未 remap `file_assets` |
| 「telegram 页 Bot 模式有出路」 | **假**：仅提示文案，无返回控制台链接 |

### 修复

| 项 | 落地 |
|----|------|
| `file_access.rs` | `index_file_metadata_list` / `remap_file_assets_after_move` + 单测 |
| `commands/fs.rs` | `cmd_get_files` 懒索引；`cmd_move_files` 捕获 forward 新 id 并 remap |
| `api_routes.rs` | User bulk move 同步 remap 索引 |
| `Dashboard.tsx` / `useFileOperations.ts` | 移动后 invalidate 全部 `['files']` 查询 |
| `telegram-auth.js` | Bot 就绪时链回控制台与设置页 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **79 passed**
- `npm test` → **46 passed**

## 第十九轮修复（User 本地索引 + 列表拖放 + shares 就绪 + E2E）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「User 模式搜索可走 DB」 | **假**：`cmd_search_global` 仍每次打 Telegram SearchGlobal |
| 「桌面 User 上传会写索引」 | **假**：`cmd_upload_file` 未 `record_uploaded_file` |
| 「FileListItem 文件夹拖入已门禁」 | **假**：第十八轮只修了 FileCard，列表视图漏改 |
| 「shares 页创建分享有就绪 UX」 | **假**：未禁用表单；files 页第十八轮已修，shares 未对齐 |
| 「Playwright 未进 CI」 | **假**：`docker-api.yml` 已有；但缺 dashboard 就绪态用例 |
| 「keepalive 探针未去抖」 | **假**：`server_maintenance` 仍裸 `bot_test_connection` |

### 修复

| 项 | 落地 |
|----|------|
| `commands/fs.rs` | 上传后写 `file_assets`；删除同步索引；`cmd_search_global` DB 优先 |
| `api_routes.rs` | User 模式在 `file_assets` 非空时走资产索引 list/search |
| `db.rs` | `count_all_file_assets` + 单测 |
| `FileListItem.tsx` | 文件夹拖入需 `transferEnabled` |
| `shares-core.js` | 未就绪禁用创建表单；30s 刷新就绪态 |
| `server_maintenance.rs` | keepalive 用 `bot_test_connection_cached` |
| `web-smoke.spec.ts` | dashboard 上传按钮 disabled + token 静态断言 |
| `FileListItem.test.tsx` | Vitest 拖放门禁 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **78 passed**
- `npm test` → **46 passed**（含 `FileListItem` 拖放门禁）

## 第十八轮修复（Bot 探针去抖 + 侧栏拖放门禁 + Web files 就绪 UX）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第十七轮后 health.ready 稳定」 | **假**：每次 `GET /health` 仍打 Bot getMe，间歇性 `ready=false` |
| 「侧栏拖放离线已门禁」 | **假**：仅 `handleDropOnFolder` 拦截；`SidebarItem` 仍高亮可 drop |
| 「文件夹卡片拖入离线已禁」 | **假**：`FileCard` 文件夹 `onDragOver` 未查 `transferEnabled` |
| 「Web files 页下载/删除有就绪 UX」 | **假**：服务未就绪时行内按钮仍可点，仅点击后才 toast |
| 「EXTENSION-ROADMAP P2 探针去抖已做」 | **假**：文档有方案，代码未实现 |

### 修复

| 项 | 落地 |
|----|------|
| `telegram_transport.rs` | `BotProbeCache` 30s TTL + stale-while-revalidate；连续 3 次失败才判死 |
| `api_routes.rs` / `auth_routes.rs` | health/auth 改用 `bot_connection_ready` / `bot_test_connection_cached` |
| `SidebarItem.tsx` / `Sidebar.tsx` | `dropEnabled` 离线禁拖入高亮与 drop |
| `FileCard.tsx` | 文件夹拖入高亮/drop 需 `transferEnabled` |
| `files-core.js` | 未就绪禁用下载/分享/批量删除；30s 周期刷新就绪态 |
| `FileCard.test.tsx` / `SidebarItem.test.tsx` | Vitest 覆盖拖放门禁 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **77 passed**
- `npm test` → **44 passed**（含 `FileCard` / `SidebarItem` 拖放门禁）

## 第十七轮修复（搜索 SQL 分页 + 进度 token + 分享/设置闭环）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「Bot list `?search=` 分页正确」 | **假**：第十六轮只加了 folder scope，search 仍在内存 filter，第 2 页仍可能空 |
| 「上传 SSE 不带 pwd 就安全」 | **假**：第十五轮仍用 `pwd=` query，会进 access log |
| 「频道内文件分享能下载」 | **假**：`ShareDialog` 未传 `activeFolderId`，`folder_id` 丢失 |
| 「settings 保存后分享基址即时更新」 | **假**：PUT 响应缺 `effective_share_base_url` |
| 「sessionError 不误杀」 | **假**：`timeout`/`network` 关键词过宽触发 forceLogout |
| 「双队列 UI 不重叠」 | **假**：DownloadQueue 与 UploadQueue 同 `bottom-6` |

### 修复

| 项 | 落地 |
|----|------|
| `db.rs` | `name_contains` SQL LIKE 过滤；删除占位函数；单测 |
| `api_routes.rs` | `api_list_files` 传 `search` 给 scoped DB，移除内存 retain |
| `upload_progress.rs` | HMAC `upload_progress_token`（5min）；`POST /upload_progress_token` |
| `legacy_routes.rs` | `/upload_status` 共用 token/pwd 鉴权 |
| `upload-core.js` | 上传前取 token，SSE/WS/轮询用 `exp+token` |
| `ShareDialog.tsx` / `Dashboard.tsx` | `activeFolderId` 补全分享下载 |
| `sessionError.ts` | 收窄关键词 + Vitest |
| `FileCard` / `FileListItem` | 离线禁拖；文件夹不可拖 |
| `DownloadQueue.tsx` | `bottom-[22rem]` 避免与上传队列重叠 |
| `settings_routes.rs` | PUT 返回 `effective_share_base_url` |
| `settings-core.js` / `share-domain.js` | 保存域名后刷新分享基址 |
| `openapi.json` | upload token 参数、`folder_id` search、集成测试 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → **75 passed**
- `npm test` → **38 passed**（含 `sessionError.test.ts`）

## 第十六轮修复（资产索引 folder 分页/搜索 + 纯函数 TDD）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「Bot 模式 list API 分页正确」 | **假**：SQL OFFSET 后再内存 filter folder → 第 2 页空 |
| 「search API 尊重 folder_id」 | **假**：资产索引路径忽略 query folder |
| 「Web safeNext/safeHttpUrl 有回归测试」 | **假**：仅内联 JS，无 Vitest |
| 「connection checking 态有测试」 | **假**：第十五轮新增态无单测 |

### 修复

| 项 | 落地 |
|----|------|
| `db.rs` | `list_file_assets_scoped` / `count_file_assets_scoped` SQL 级 folder 分页 |
| `db.rs` | `search_file_assets` 增加 folder scope |
| `api_routes.rs` | `api_list_files` / `api_search_files` 接线 scoped DB |
| `webPure.ts` + `web-pure.js` | safeNext / safeHttpUrl / escapeHtml 单源 + Vitest |
| `connection.test.ts` | checking 态不可传输 |
| `docs/ROUND-16-TDD.md` | 第一性原理 + TDD 清单 |

### 测试（mock，无 Telegram 网络）

- `cargo test --features headless-server --lib` → 73 passed（含 folder 分页/搜索单测）
- `npm test` → vitest 全绿

## 第十五轮修复（桌面连接态 + 后端传输同步 + Web 安全/深链）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第十四轮后 mount 不会误传」 | **假**：`connectionStatus` 默认 `online`，首检前可上传 |
| 「预览翻页有门禁」 | **假**：`navigatePreview` 未查 `sessionOnline` |
| 「8550 改传输模式后 14201 分享仍可用」 | **假**：双 `TransportHandle` 内存不同步 |
| 「POST /api/v1/files 会初始化 User client」 | **假**：未调 `ensure_transport_ready` |
| 「分享 API 500 返回 JSON」 | **假**：`share_api_routes` 用 plain text |
| 「settings 展示正确分享基址」 | **假**：`effective_base_url` 指向 API 端口非 14201 |
| 「upload 结果弹窗无 XSS」 | **假**：Markdown/BBCode textarea 未 escape |
| 「telegram 登录后深链保留」 | **假**：硬编码 `/dashboard.html` |

### 修复

| 项 | 落地 |
|----|------|
| `connection.ts` / `useTelegramConnection` | 新增 `checking` 态；首检前 `canTransfer=false` |
| `Dashboard.tsx` | 预览翻页门禁；离线关闭预览；搜索离线不 spam；横幅按态文案；键盘快捷键拆分 |
| `useKeyboardShortcuts.ts` | `transferEnabled` 与导航快捷键分离 |
| `ShareDialog.tsx` | 会话错误 → `onSessionError` / `forceLogout` |
| `SettingsModal.tsx` | `sessionOnline` 默认 `false` |
| `telegram_transport.rs` | `active_mode()` 从磁盘重载（8550/14201 同步） |
| `api_routes.rs` | `api_upload_file` 前 `ensure_transport_ready` |
| `share_api_routes.rs` | 500 统一 JSON `error` |
| `settings_routes.rs` | 新增 `effective_share_base_url` |
| Web `upload-core.js` | XSS 修复、`safeHttpUrl`、toast 替代 alert |
| Web `telegram-auth.js` | `safeNext` + `afterAuthSuccess`；`fetchAuthStatus` |
| Web `settings.html` / `settings-core.js` | 展示分享链接基址 |

### 有意保留

| 项 | 说明 |
|----|------|
| 上传进度 legacy `pwd=` query | 仍兼容旧客户端；Web 默认走 HMAC token |
| Web `shares` revoke 无 Telegram 门禁 | 本地 DB 操作 |
| `docs.html` 公开浏览 | OpenAPI 静态文档；调用 API 仍需鉴权 |

## 第十四轮修复（Web 就绪 UX 统一 + 登录深链 + 队列恢复门禁）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「upload.html 与 dashboard 上传门禁一致」 | **假**：无 `#service-banner`、无周期 `refreshStatus`、按钮默认可点 |
| 「telegram.html 登录跳转正确」 | **假**：`requireLogin('/login.html?next=...')` 导致双重 `?next=` |
| 「settings health 与 ensureServiceReady 一致」 | **假**：`loadHealth()` 裸 `fetch`，无 `X-Access-Pwd` |
| 「离线时恢复的上传/下载队列不会自动跑」 | **假**：scheduler `useEffect` 未检查 `canTransfer` |
| 「离线仍拉文件列表/bandwidth」 | **假**：React Query `enabled: !!store` 未加 `sessionOnline` |
| 「Settings REST health 与侧栏会话一致」 | **假**：health 失败静默；未展示 App session 态 |
| 「login/index 深链安全」 | **假**：`?next=` 开放重定向；index 硬跳丢失 deep link |
| 「docs 复制 curl 有 toast」 | **假**：页面无 `#toast` 元素 |

### 修复

| 项 | 落地 |
|----|------|
| `page-readiness.js` | 抽取 dashboard/upload 共用 `refreshUploadReadiness()`（auth + health + ensureServiceReady + 横幅/按钮） |
| `upload.html` | 服务状态区 + 默认禁用上传 + 60s 轮询 |
| `dashboard.html` | 改用 `TdPageReadiness` + `TdApi.requireLogin()` |
| `telegram-auth.js` | 修复双重 `?next=` |
| `login.js` | `safeNext()` 防开放重定向；已登录自动跳转 |
| `index.html` | 保留 pathname 的 `?next=` 深链 |
| `settings-core.js` | `loadHealth()` → `TdApi.fetchHealth()` |
| `docs.html` | 补 `#toast` |
| `useFileUpload` / `useFileDownload` | `processItem` + scheduler 前置 `canTransfer` |
| `Dashboard.tsx` | `cmd_get_files` / `cmd_get_bandwidth` 需 `sessionOnline` |
| `SettingsModal.tsx` | App session 行 + REST health 错误可见 |

### 有意保留

| 项 | 说明 |
|----|------|
| Web `shares` revoke | 本地 DB 操作，离线可撤销（与桌面 Settings 分享 Tab 一致） |
| 桌面 REST 子集 | 无 legacy `/upload` 分片、无 `/metrics`、无 WebDAV |

## 第十三轮修复（编译阻断 + health 对齐 + URL 拆分 + Settings 接线）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第十二轮后 Settings 能改代理/VPN」 | **假**：`SettingsModal` proxy/VPN `useEffect` 语法损坏，TS 无法编译 |
| 「health.ready 与 auth.connected 一致」 | **假**：User 模式 health 仅查 client 存在 |
| 「桌面 REST 上传两个 URL 都对」 | **假**：`download_url` 走 14201 但 `api_download_url` 仍指向 14201 → 404 |
| 「files-core 与 ensureServiceReady 一致」 | **假**：banner 仍只读 health，User 未授权时误显示就绪 |
| 「route_registry 反映桌面子集」 | **假**：仅 Headless 全量列表 |

### 修复

| 项 | 落地 |
|----|------|
| `SettingsModal.tsx` | 修复 proxy/VPN `useEffect`；接收 `sessionOnline`；离线时跳过 `cmd_reconnect_telegram` |
| `Dashboard.tsx` | 向 SettingsModal 传递 `sessionOnline` / `transferBlockedMessage` |
| `user_telegram_connected()` | User 模式 health `ready` 与 auth 对齐（`get_me()`） |
| `api_upload_file` | `share_base`（14201 `/d/*`）与 `api_base`（8550 REST 下载）分离 |
| `cmd_apply_proxy_settings` | 校验 proxy_type，仅允许 SOCKS5 |
| `settings_routes` merge_proxy | 不再每次 PATCH 强制覆盖 `proxy_type` |
| `route_registry.rs` | `HEADLESS_ROUTES` + `DESKTOP_API_ROUTES` + 子集测试 |
| Web `files-core` / `shares-core` | banner 统一委托 `TdApi.ensureServiceReady()` |
| Web `dashboard.html` | 上传不可用横幅 + 禁用上传按钮 |
| Web `settings-core.js` | 传输模式切换 contextual confirm |

## 第十二轮修复（深层门禁 + 后端链路 + Web 登录恢复）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第十一轮已拦住所有传输操作」 | **假**：ShareDialog、右键菜单、预览/流媒体、文件夹 Sync/Create/Delete、键盘 Enter 仍可触发 `invoke` |
| 「Web ensureServiceReady 足够」 | **假**：User 模式 `health.ready` 仅检查 client 存在，`auth/status.connected` 才代表已授权 |
| 「桌面 REST 上传 download_url 可用」 | **假**：`POST /api/v1/files` 用 API 端口拼 `/d/*`，实际下载在 **14201** |
| 「输错验证码可重试」 | **假**：`phone_sign_in` 失败后 `login_token` 被消耗，UI 无恢复路径 |

### 修复

| 项 | 落地 |
|----|------|
| 桌面 UI 深层门禁 | ShareDialog、FileCard/FileListItem/ContextMenu、handlePreview/handleShare、键盘快捷键、Sidebar Create Folder |
| `useTelegramConnection` | Sync/Create/Delete Folder 前置 `requireOnline()` |
| 上传/下载 retry | `retryItem` 增加 `guardTransfer()` |
| `api_upload_file` | 桌面 `use_stream_port_for_shares` 时用 `share_link_base`（14201）生成 `download_url` |
| `phone_sign_in` | 验证码错误时恢复 `login_token` |
| Web `ensureServiceReady` | User 模式额外校验 `auth/status.connected` |
| Web `telegram-auth.js` | 验证码失败回退手机号步骤；QR 轮询错误可见 |
| Web `shares-core.js` | 服务横幅 + 创建分享前 `ensureReadyForAction()`；域名保存 `.catch` |

## 第十一轮修复（传输门禁 + Web ready 检查）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第十轮后会话离线时 UI 会拦住操作」 | **假**：TopBar 批量下载/删除、FileExplorer 上传、拖放移动仍可在 `session_lost` 时触发 |
| 「Web 下载/分享前会检查服务就绪」 | **假**：`files-core.js` 直接调 API，未查 `health.ready` |
| 「Web 上传前会检查 ready」 | **假**：`upload-core.js` 点击上传未校验 |
| 「拖放移动失败会触发登出」 | **假**：`handleDropOnFolder` 仅 toast，无 `onSessionError` |

### 修复

| 项 | 落地 |
|----|------|
| `canTransferFiles()` | 统一 `connectionStatus === 'online'` 判断 |
| 桌面 hooks | `useFileUpload` / `useFileDownload` / `useFileOperations` 增加 `canTransfer` 前置守卫 |
| TopBar / FileExplorer / EmptyState | 会话离线时禁用上传、批量、下载文件夹等按钮 + tooltip |
| Dashboard | 连接态横幅；拖放移动前置拦截 + `isSessionLostError` → `forceLogout` |
| Web `api-client.js` | `fetchHealth()` / `ensureServiceReady()`（`/api/v1/health` + `ready`） |
| Web `files-core.js` | 页顶服务横幅；下载/分享/批量删除前 `ensureReadyForAction()` |
| Web `upload-core.js` | 开始上传前 `ensureServiceReady()` |

## 第十轮修复（连接态 UX + 会话错误全链路 + API 迁移）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第九轮后侧边栏状态准确」 | **半假**：仍写 Connected to Telegram，网络通但会话死时误导 |
| 「删除/移动失败会触发登出」 | **假**：`forceLogout` 仅接上传/下载，删文件/搜索/移动不走 |
| 「老用户开 API 有 local_access_pwd」 | **假**：第九轮前启用的实例 json 无字段 → 重启仍 401 |
| 「Settings 同步失败用户知道」 | **假**：proxy/VPN `catch` 静默吞掉 |

### 修复

| 项 | 落地 |
|----|------|
| `connectionStatus` | `online` / `session_lost` / `network_offline`；侧栏文案 + 黄/红/绿点 |
| Sync 按钮 | 仅 `online` 时可点；会话失效时禁用并 tooltip |
| `useFileOperations` | 删除/移动/搜索会话错误 → `onSessionError` → `forceLogout` |
| `prepare_settings_for_runtime` | 启动/重启 API 时补全缺失 `local_access_pwd` 并持久化 |
| SettingsModal / SettingsContext | proxy/VPN/持久化失败 toast |
| `sharing.rs` | 密码 trim 空串 → None |
| Web settings | 分享域名注明 Headless :1334 vs 桌面 :14201 |

## 第九轮修复（桌面鉴权 + 会话断链 + Web 代理密码）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「开 API 就能 curl，没 Key 也行」 | **假**：`for_desktop_api.access_pwd` 为空 → 仅 `NO_KEY_CONFIGURED`，必须先生成 Key |
| 「forceLogout 已实现」 | **假**：导出但上传/下载失败从不调用 → 会话死了用户仍困在 Dashboard |
| 「API 绿点 = 已启动」 | **假**：`restart_api_server` 异步，`running` 在 bind 前返回 false |
| 「Web 改代理密码」 | **假**：表单无 password 字段，PATCH 永远带不出新密码 |

### 修复

| 项 | 落地 |
|----|------|
| `local_access_pwd` | 首次启用 API 自动生成；`for_desktop_api` 注入 `X-Access-Pwd`；Settings 可复制/轮换 |
| `wait_for_api_server` | `cmd_update_api_settings` / 轮换 Key 后最多等 3s 再报 running |
| `forceLogout` 接线 | 上传/下载 `isSessionLostError` → 自动登出 |
| Web `settings.html` | SOCKS5 密码字段（留空不修改） |
| `docs/DESKTOP-API.md` | Headless 全量 vs 桌面子集、端口、鉴权 |
| `openapi.json` | description 指向 DESKTOP-API |

## 第八轮修复（DATA_DIR 分裂 + 分享端口 + 连接态）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「第七轮后桌面 REST 分享能用」 | **假**：`POST /api/v1/shares` 仍用 API 端口拼 `/d/*`，实际下载在 **14201** |
| 「流媒体 Transport 与 DB 同目录」 | **假**：14201 线程读 `DATA_DIR`/temp，DB 在 `app_data_dir` |
| 「侧边栏 Connected = Telegram 在线」 | **假**：仅 `cmd_is_network_available`（TCP 探测 DC） |
| 「开密码保护但没填密码会拦住」 | **假**：ShareDialog 静默传 `null` |

### 修复

| 项 | 落地 |
|----|------|
| 流媒体 `data_dir` | 默认 `app_data_dir`；仅显式 `DATA_DIR` 时覆盖 |
| `ShareApiState.use_stream_port_for_shares` | 桌面 REST 分享链接走 `share_base_url_from_data_dir(..., 14201)`；Headless 仍用 `effective_base_url` |
| `cmd_get_stream_info` | 读 `ui_settings.share_domain`，与分享链接策略一致 |
| `useTelegramConnection` | `cmd_check_connection` + 网络可达，30s 轮询 |
| ShareDialog | 开启密码保护但未填密码 → 阻止提交 |
| MediaPlayer | 流初始化失败显示错误，不再无限 Preparing |
| Web `shares.html` | 补密码、有效期字段并传 API |
| `sharing_core` 空 base | 回退 `127.0.0.1:14201` 非 localhost |

### 仍有意保留

| 项 | 说明 |
|----|------|
| 桌面 REST 子集 | 无 `/upload`、`/metrics`、WebDAV（OpenAPI 描述 Headless 全量面） |
| 桌面 API 鉴权 | 启用后自动生成 Local Access Password（`X-Access-Pwd`）或 API Key |
| Headless vs 桌面分享域名端口 | Web 文案 `:1334`（统一网关）；桌面 Settings 填 `:14201` |

## 第七轮修复（P0 桌面 API + 分享域名 + 鉴权统一）

### 自反驳（本轮前）

| 反驳 | 结论 |
|------|------|
| 「桌面开 REST API 就能用」 | **假**：`restart_api_server` 只挂 `configure_api`，缺 DB/Transport/UploadGate → `/health`、`/files` 500 |
| 「ShareDialog 改域名就够了」 | **假**：DB 入库仍写 `localhost:14201`，换设备复制链接仍错 |
| 「transport GET 公开无所谓」 | **假**：泄露 bot/user 配置态；POST 还用明文 api_key 比对 |
| 「重置设置只清 UI」 | **假**：`network_settings.json` 仍保留旧 VPN/代理 |

### 服务端 / 桌面

| 项 | 修复 |
|----|------|
| 桌面 REST API | 新增 `desktop_api_server.rs` + `ServerConfig::for_desktop_api`；完整注入 Db、Transport、UploadGate、Share/Settings/Auth 路由 |
| 分享链接入库 | `sharing.rs` 读 `ui_settings.json` → `share_base_url_from_data_dir`（含 `host:14201`） |
| `GET/POST /api/v1/transport` | 统一 `require_admin_or_api_key`（hash 验证 API Key） |
| `POST /api/v1/shares` | `message_id > 0` + 非空文件名校验 |
| `require_admin_or_api_key` | 抽到 `admin_routes.rs`，settings/transport 共用 |

### Web / 桌面 UI

| 项 | 修复 |
|----|------|
| `shares-core.js` | 拒绝 `message_id <= 0` |
| `resetSettings` | 同步重置 `network_settings.json`（proxy/vpn）+ 清空 `ui_settings.share_domain` |
| `test-api.sh` | health version 不再硬编码 `2.x` |

### 仍有意保留的架构差异（非 bug）

| 项 | 说明 |
|----|------|
| 桌面 `network_settings.json` 路径 | `app_data_dir`；Headless 用 `DATA_DIR` — 同机双部署需分别配置 |
| 分享域名含端口 | Tailscale/LAN 访问流媒体需填 `host:14201` 或反代到 `/d/*` |

## 第六轮修复（自反驳审计 — P1 断链）

### 服务端

| 项 | 修复 |
|----|------|
| `PUT /api/v1/network` | 改为 **Patch 合并**（仅更新传入字段，不再重置 VPN 全量） |
| 代理校验 | 启用代理时 host 不能为空（Rust + Web 双重校验） |
| Headless `auto_detect_vpn` | `telegram-drive-server.rs` 启动时调用 `maybe_auto_enable_vpn_on_startup` |
| settings 鉴权 | 同时接受 `X-Access-Pwd` 与 `X-API-Key` |
| 桌面分享域名 | `cmd_get/set_ui_share_domain` 写入 `ui_settings.json`，与 Web 同源 |

### Web / 桌面

| 项 | 修复 |
|----|------|
| `settings.html` | 代理 host/port/username 可编辑；PATCH 保存 |
| `files-core.js` | 用 `Map` 存元数据，修复 data-attribute 编码问题 |
| `upload-core.js` | 上传结果模态 **escapeHtml** 防 XSS |
| `login.js` | 移除 `?pwd=` URL 自动登录（安全风险） |
| 侧栏 | Telegram 链接统一为 `/telegram.html` + `data-nav` |
| TopBar「Start」 | 点击返回 Saved Messages 根目录 |

## 第五轮修复（服务端 settings + Web 功能补全）

### 服务端 API

| 端点 | 落地 |
|------|------|
| `GET/PUT /api/v1/settings` | `ui_settings.json` 分享域名 + 分片配置只读 |
| `GET/PUT /api/v1/network` | Headless 读写 `network_settings.json`（热更新） |
| `effective_base_url` | 分享/上传链接生成优先 `share_domain` > `BASE_URL` > Host |

### Web 管理台

| 项 | 落地 |
|----|------|
| `share-domain.js` | 服务端 + localStorage 双写；上传/分享/列表共用 |
| `files.html` | 行内「下载」「分享」（调 REST API） |
| `shares.html` | 创建分享表单 + `POST /api/v1/shares` |
| `settings.html` | 分享域名、分片只读、VPN/代理开关（Headless） |
| `telegram.html` | 统一侧栏 + `TdApi` 鉴权 |

## 第四轮修复（VPN 真实接线 + Web 全站侧栏 + auto-detect）

### Rust / Headless

| 项 | 落地 |
|----|------|
| HTTP 下载/上传限速 | `ThrottledReader` + `throttle_transfer_bytes` 接入 `http_download` / `http_upload` / `commands/fs` |
| Headless chunk 大小 | 所有 `download_message_stream` / `download_manifest_stream` 传入 `net_config`，使用 `chunk_size_bytes()` |
| Peer cache 上限 | `resolve_peer_with_limit` + `trim_peer_cache` |
| Keep-alive 后台任务 | 桌面 `lib.rs` + headless `telegram-drive-server.rs` 读取 `keep_alive_interval_sec` |
| 流媒体 server | `start_server` 注入 `net_config` / `AdminState` / `TransportHandle` |
| `auth_routes` 管理端点 | phone/qr 需 `X-Access-Pwd`（`require_admin_access`） |
| `auto_detect_vpn` | 启动时检测 VPN 网卡并自动启用 optimizer；Settings 开关联动 `vpnMode` |

### Web 管理台 (`deploy/web`)

| 项 | 落地 |
|----|------|
| `upload.html` | 统一侧栏 + `TdApi` 鉴权 + `api-client.js` |
| `docs.html` | 统一侧栏 + 退出登录 |
| `telegram-auth.js` | `authFetch` + `X-Access-Pwd` |
| 全站侧栏 | dashboard / files / shares / settings / upload / docs 导航一致 |

### 桌面端

| 项 | 落地 |
|----|------|
| Adaptive polling | `useNetworkStatus` → `cmd_get_polling_interval_ms` |
| Auto-detect VPN | `SettingsContext` 启动检测 + Settings 开关开启时自动 `vpnMode` |

## 第三轮修复（Web 管理台 + 桌面设置 + VPN 接线）

### Web 管理台 (`deploy/web`)

| 项 | 落地 |
|----|------|
| `check_auth` 支持 `X-Access-Pwd` | Web 用登录密码调 `/api/v1/*`，与 `transport/mode` 一致 |
| `assets/api-client.js` | 统一鉴权、401 重定向、侧栏初始化 |
| `files.html` | 列表、搜索、分页、批量 delete（Bot 删索引） |
| `shares.html` | 列表、复制、撤销、域名覆盖（localStorage） |
| `settings.html` | 传输模式切换、`/metrics` 入口、健康状态 |
| 侧栏导航 | dashboard / files / shares / settings / upload / docs 全站一致 |

### 桌面端 (Tauri + React)

| 项 | 落地 |
|----|------|
| `globalDomain` | 写入 `settings.json`；Settings / ShareDialog 共用 |
| 启动加载 `network_settings.json` | `SettingsContext` 调用 `cmd_get_network_config` 合并 |
| 代理 SOCKS5 真实生效 | 移除 MTProto 假选项；变更后 `cmd_reconnect_telegram` |
| `autoUpdate` UI 开关 | Settings → Updates |
| VPN chunk 大小 | `cmd_download_file` 使用 `net_config.chunk_size_bytes()` |
| Peer cache 上限 | `resolve_peer` 扩展后 trim 至 2000 |
| 死代码 | 删除未引用的 `useFileDrop.ts` |

## 已修复（第一、二轮）

### Web 管理台

| 问题 | 修复 |
|------|------|
| 未登录点「Telegram 登录」死链 | `login.html` → `?next=/telegram.html` |
| Bot 模式仍显示 User 登录 | `user_configured` 控制 UI |
| QR 无图、无自动轮询 | qrcodejs + 2.5s poll |
| 上传进度 SSE 无鉴权 | `pwd` 参数 + `/upload_status` 降级 |

### Rust API

| 端点 | 修复 |
|------|------|
| Bot 模式 files/search/folders/bulk | 走 `file_assets` 索引 |
| 分享多租户 | `revoke_share_for_owner` |
| 大文件 Bot 下载 | `download_degradation` 引导页 |

### 桌面端

| 问题 | 修复 |
|------|------|
| 并发上传/下载 | `maxConcurrent*` 真正并行 |
| 批量下载 | `queueBulkDownload` |
| Rename 假按钮 | 已移除 |

## 刻意保留的设计（非 bug）

| 项 | 说明 |
|----|------|
| 桌面外部 OS 拖放 | Tauri `dragDropEnabled: false`；应用内 Upload + 区域拖放可用 |
| Bot bulk move | 需 User 模式（Telegram 转发） |
| Web 分享域名 | 服务端 `ui_settings.json` + 桌面 `cmd_set_ui_share_domain` 同步；仍可与 `settings.json.globalDomain` 并存 |
| Headless VPN UI | Web settings 可开关 proxy/vpn；完整参数可手改 `network_settings.json` |

## Web 控制台页面

| 路径 | 功能 |
|------|------|
| `/dashboard.html` | 状态 + 分片上传 |
| `/files.html` | 文件列表 / 搜索 / 批量删除 |
| `/shares.html` | 分享链接管理 |
| `/settings.html` | 传输模式、Metrics |
| `/upload.html` | tg-disk 兼容上传 |
| `/docs.html` | OpenAPI 静态文档 |

鉴权：登录密码 → `X-Access-Pwd`；外部集成 → `X-API-Key`。

## 验证命令

```powershell
cd app\src-tauri
cargo test --features headless-server --lib   # 68 passed (2026-06-08 第十轮)

# 存储回归（需运行中的服务）
..\scripts\storage-regression.ps1
```

## 后续迭代（非阻塞）

- PostgreSQL 多副本元数据
- Pentaract 式 Bot usage 表
- 桌面与 Headless 网络配置单文件同步（需明确 DATA_DIR = app_data_dir 策略）

详见 [DEPLOYMENT-PRODUCTION.md](DEPLOYMENT-PRODUCTION.md)。
