# Round 38 — Hooks 集成测试层（TDD）

## 主要矛盾（决定性 → 牵引性 → 阶段性）

| 维度 | 判断 |
|------|------|
| **决定性** | R37 纯函数 100% 可测，但 `useFileUpload` / `useFileDownload` 的 invoke/listen/状态机无 renderHook 覆盖 → **生产 bug 仍可能只在真机出现** |
| **牵引性** | mock Tauri + `renderHook` 验证：pending→success、错误分类、cancel/retry/clear、canTransfer 守卫、progress 事件 |
| **阶段性** | R38 收口桌面 Hooks；R39 可选 connection hook + E2E_API Headless 管线 |

## 用户旅程 & 失败用例（先测后写）

### Upload
1. 队列 pending → invoke 成功 → status `success`，invalidate files
2. invoke `FILE_TOO_BIG` → status `error` + 专用 toast
3. invoke session lost → `onSessionError` 回调
4. `canTransfer()=false` → 不处理 pending + retry toast
5. `upload-progress` 事件 → progress/bytes 更新
6. cancelItem(uploading) → `cmd_cancel_transfer`
7. clearFinished → 移除 success 项

### Download
1. queueDownload → pending 入队
2. save 对话框取消 → 项移除
3. invoke 成功 → success + toast
4. `Transfer cancelled` → cancelled
5. session lost → onSessionError
6. classifyDownloadFailure 与 upload 对齐（纯函数单测）

## 执行顺序

1. `classifyDownloadFailure` 提取（downloadPure）
2. test-setup 补 listen/dialog/store mock
3. `hookWrapper` + `useFileUpload.test.tsx` + `useFileDownload.test.tsx`
4. vitest coverage 纳入 hooks
5. cargo + npm + coverage + playwright

## 反转条件

- 若 renderHook 与 SettingsProvider 加载竞态导致 flaky → 改用轻量 TestSettingsContext
- 若 hooks coverage 拉低全局门槛 → hooks 单独 threshold 或分 job

## 下一阶段转移信号

- 用户报「连接状态卡在 checking」→ R39 `useTelegramConnection` 集成测
- CI 要求 E2E API 全绿 → 增加 `E2E_API=1` + headless 启动脚本
