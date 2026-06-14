# Round 43 — hooks 分支补全 + settings 手动重建索引 + download remount

## 主要矛盾

| 维度 | 内容 |
|------|------|
| **决定性** | R42 后 `useFileOperations` 下载/移动成功路径与错误路径缺证伪；User 模式切换索引重置后无显式 manual rebuild 入口 |
| **牵引性** | 对称 upload remount 测试覆盖 download；settings 页接 `TdWebPure.rebuildIndexShouldToast('manual')` |
| **阶段性** | R43 收口 R42 反转条件；TransferQueuePanel 抽取仍属维护向 P2 |

## 先测后写

1. `handleBulkDownload` 成功入队并清空选择 ✅
2. `handleDownloadFolder` 非空文件夹入队 / 无 queue 报错 ✅
3. 无 queue 时 toast「下载队列不可用」 ✅
4. `handleBulkMove` 失败 toast / moved=0 info ✅
5. bulk delete session-lost → `onSessionError` ✅
6. `useFileDownload` remount 恢复 pending ✅
7. Playwright：settings.html 按钮 + settings-core manual rebuild 接线 ✅

## 实现摘要

- **`useFileOperations.test.tsx`**：+7 用例（15 total）
- **`useFileDownload.test.tsx`**：remount 对称测试
- **`settings.html`**：文件索引块 + `web-pure.js`
- **`settings-core.js`**：`rebuildFileIndexManual()`；切换模式文案指向设置页

## 验证结果（2026-06-12）

| 命令 | 结果 |
|------|------|
| `cargo test --features headless-server --lib` | 89 passed |
| `npm test`（app） | 199 passed |
| `npm run test:coverage` | 97.35% stmts / 85.84% branches；`useFileOperations` 95.45% lines |
| `tests/e2e` `npm test` | 30 passed, 2 skipped |
| `tests/e2e` `npm run test:api` | 32 passed, 0 skipped |

## 反转条件

- 用户反馈 settings rebuild 应 Bot 模式也可用 → 需 API 层支持后再放开
- `useFileOperations` 分支仍 <80% → 补 session error 回调 on bulk delete 路径

## 下一阶段（R44 候选）

- `TransferQueuePanel` 抽取（EXTENSION-UX C）
- headless 其余 `blocking_read` 迁移
