# Round 40 — 队列持久化 + Sync 成功路径 + E2E API 验证

## 主要矛盾

| 维度 | 内容 |
|------|------|
| **决定性** | R39 connection 守卫已测，但 **sync 成功路径**、**store 恢复 pending 队列** 仍无单测 → 重启后丢任务风险不可证伪 |
| **牵引性** | 补 `handleSyncFolders` 成功/无新增、upload/download store 恢复、zip 失败；跑通 `E2E_API=1` |
| **阶段性** | R40 收口 hooks 持久化；R41 可选 Web 下载进度、connection 30s 轮询 flake 治理 |

## 先测后写

1. `handleSyncFolders` 发现新文件夹 → merge + rebuild + toast「新增」 ✅
2. `handleSyncFolders` 无新文件夹 → toast「同步完成」 ✅
3. `cmd_check_connection` false → session_lost ✅
4. `useFileUpload` / `useFileDownload` 从 store 恢复 pending + 中文 toast ✅
5. `handleFolderUpload` zip 失败 → error toast，不入队 ✅
6. Playwright `npm run test:api` → health 200 + rebuild-index 401 ✅

## 实现摘要

- **`resetStoreData()`**：Vitest store mock 有状态，`set` 后 `get` 可读（fix sync `folderIds`）
- **Web**：`dashboard.html` / `upload.html` 用 `if (TdApi.requireLogin()) { … }` 替代 `throw redirect`
- **E2E dashboard**：mock `/api/v1/*` 后 `page.evaluate` 调用 `TdPageReadiness.refreshUploadReadiness`

## 验证结果（2026-06-12）

| 命令 | 结果 |
|------|------|
| `cargo test --features headless-server --lib` | 88 passed |
| `npm test`（app） | 173 passed |
| `npm run test:coverage` | 96.81% stmts / 87.41% branches |
| `tests/e2e` `npm test` | 26 passed, 0 skipped |
| `tests/e2e` `npm run test:api` | 26 passed, 0 skipped |

## 反转条件

- E2E API 编译 >5min → CI 预编译 artifact
- `:1334` 被占用 → `test:api` 失败；需先结束旧 serve/headless
- store 恢复与 processItem 竞态 flaky → mock invoke 挂起再断言

## 下一阶段（R41 候选）

- Web 下载 determinate 进度（`ReadableStream` + `downloadPure`）
- `useTelegramConnection` 30s 轮询 flake 治理
- 桌面 bulk move 读取 REST `transport_mode`
