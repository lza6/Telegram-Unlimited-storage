# Round 41 — Web 下载进度 + 桌面 bulk move 守卫 + 连接轮询测试

## 主要矛盾

| 维度 | 内容 |
|------|------|
| **决定性** | R40 后 Web 下载仍无 determinate 进度；桌面 bulk move 未读 REST `transport_mode`，与 Web 行为不一致 |
| **牵引性** | `downloadPure` 流式进度 + `files-core` 按钮标签；`Dashboard` 接 `cmd_get_api_health`；`useFileOperations` 守卫单测 |
| **阶段性** | R41 补齐 EXTENSION-UX-R37 P1；R42 可选 onboarding 卡片、索引静默重建 |

## 先测后写

1. `computeDownloadPercent` / `consumeStreamWithProgress` / `readResponseBlobWithProgress` 单测 ✅
2. `files-core.js` 使用 `TdDownloadPure.readResponseBlobWithProgress` 更新按钮文案 ✅
3. `useFileOperations`：`canBulkMove=false` 阻止 `handleBulkMove`；允许时调用 `cmd_move_files` ✅
4. `useTelegramConnection`：fakeTimers 验证 30s 再探 `cmd_check_connection` ✅
5. Playwright：download-pure 镜像新符号；dashboard 就绪 E2E 在静态 + API 模式均通过 ✅

## 实现摘要

- **`downloadPure.ts` / `download-pure.js`**：`computeDownloadPercent`、`formatDownloadProgressLabel`、`deriveWebDownloadButtonState(percent)`、`consumeStreamWithProgress`、`readResponseBlobWithProgress`
- **`files-core.js`**：下载走流式 blob + 按钮百分比标签
- **`useFileOperations.ts`**：`guardBulkMove()`；opts `canBulkMove` / `bulkMoveBlockedMessage`
- **`Dashboard.tsx`**：`useQuery(['api-health'])` → `cmd_get_api_health`；bulk move + 拖放移动同守卫
- **`filesPure.ts`**：`bulkMoveBlockedMessage(mode, 'desktop'|'web')` 分表面文案
- **`web-smoke.spec.ts`**：下载进度符号断言；dashboard 用 `**/api/v1/*` 路由 + 显式 `refreshUploadReadiness`

## 验证结果（2026-06-12）

| 命令 | 结果 |
|------|------|
| `cargo test --features headless-server --lib` | 88 passed |
| `npm test`（app） | 182 passed |
| `npm run test:coverage` | 92.46% stmts / 86.91% branches |
| `tests/e2e` `npm test` | 24 passed, 2 skipped |
| `tests/e2e` `npm run test:api` | 26 passed, 0 skipped |

## 反转条件

- `useFileOperations.ts` coverage ~55% → R42 补 delete/bulkDelete 成功路径
- API 模式 dashboard E2E 再 flake → 产品侧 `await refreshStatus()` 或统一 readiness 模块
- `vpn_optimizer.rs` headless panic → 日志噪音；不影响 E2E 断言

## 下一阶段（R42 候选）

- User 模式 dashboard onboarding 卡片默认可关闭
- Web 索引静默重建（刷新不弹 toast）
- 桌面端 store 队列重启 E2E
