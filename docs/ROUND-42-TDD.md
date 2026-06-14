# Round 42 — 静默索引重建 + User onboarding + 队列 remount + headless 稳定性

## 主要矛盾

| 维度 | 内容 |
|------|------|
| **决定性** | R41 后 Web 刷新/搜索仍弹「索引已重建」toast，干扰高频操作；User 模式缺 onboarding；hooks 删除路径与 remount 恢复缺证伪 |
| **牵引性** | `webPure` 统一 rebuild/onboarding 纯函数；dashboard `await refreshStatus`；`vpn_optimizer` 异步安全读 |
| **阶段性** | R42 收口 EXTENSION-UX P2 + R41 反转条件；R43 可选 TransferQueuePanel 抽取 |

## 先测后写

1. `rebuildIndexShouldToast`：refresh/search 静默，manual 才 toast ✅
2. `shouldShowUserOnboarding` / Bot 卡片互斥逻辑 ✅
3. `useFileOperations`：delete / bulkDelete 部分失败 / 空文件夹下载 ✅
4. `useFileUpload`：unmount 后 remount 从 store 恢复 pending ✅
5. `vpn_optimizer`：`keep_alive_interval_sec` 在 tokio runtime 内不 panic ✅
6. Playwright：+4 静态 smoke（onboarding、silent rebuild、web-pure、files.html 脚本序） ✅

## 实现摘要

- **`webPure.ts` / `web-pure.js`**：`rebuildIndexShouldToast`、`formatRebuildIndexSuccessToast`、`shouldShowBot/UserOnboarding`
- **`files-core.js`**：`rebuildIndexIfUser('refresh'|'search')` + `TdWebPure` toast 门控
- **`files.html`**：加载 `web-pure.js`
- **`dashboard.html`**：User onboarding 卡片；`initDashboard` async + `await refreshStatus`；`TdWebPure` 控制 Bot/User 卡片
- **`connection.ts`**：checking 文案含 verifying session
- **`vpn_optimizer.rs`**：`try_read` 替代 async 上下文的 `blocking_read`（keep-alive / polling）
- **测试**：`useFileOperations` +4、`useFileUpload` remount、`webPure` +4、`web-smoke` +4

## 验证结果（2026-06-12）

| 命令 | 结果 |
|------|------|
| `cargo test --features headless-server --lib` | 89 passed |
| `npm test`（app） | 191 passed |
| `npm run test:coverage` | 95.92% stmts / 85.52% branches；hooks 93.42% lines |
| `tests/e2e` `npm test` | 28 passed, 2 skipped |
| `tests/e2e` `npm run test:api` | 30 passed, 0 skipped |

## 反转条件

- `useFileOperations` 分支仍 ~56% → R43 补 bulkDownload 成功、session error 回调
- 用户要求 settings 页手动 rebuild 带 toast → 调用 `rebuildIndexIfUser('manual')`
- headless 仍见其他 `blocking_read` panic → 逐步改为 `try_read` 或 async API

## 下一阶段（R43 候选）

- `TransferQueuePanel` 抽取（EXTENSION-UX C，维护向）
- settings 页显式「重建索引」按钮（manual toast）
- download remount 对称测试
