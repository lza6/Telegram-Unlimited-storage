# R52 计划（Bulk succeeded_ids + 桌面 Bot 索引浏览）

> 主要矛盾：**后端只返回 count 时，前端无法知道哪些 ID 成功** → R51 用保守策略保留选中；根治需 API 返回 `succeeded_ids`，并让桌面 Bot 模式能浏览/删索引（与 Web 一致）。

## TDD 清单

1. RED `pickBulkSucceededIds` / Rust `BulkResponse.succeeded_ids` 契约测试
2. GREEN Bot bulk delete 填充 `succeeded_ids`；User 全成全败附带全部 ID
3. GREEN `files-core.js` 优先 `res.succeeded_ids`
4. GREEN `cmd_get_files` / `cmd_delete_file` 在 asset index 权威时走 DB（桌面 Bot）
5. GREEN `connection.ts` `isServiceReady`；Dashboard `serviceReady` vs `transferReady`
6. GREEN `moveExecution.ts` 复用 drag-drop 与 bulk move 逻辑
7. VERIFY vitest / cargo / playwright

## 反转条件

- 若桌面 API 服务未启动，`apiHealth.ready` 不可用 → 回退 `sessionOnline`
- 若 User 模式未完成索引重建，仍走 GramJS 列表（现有行为）
