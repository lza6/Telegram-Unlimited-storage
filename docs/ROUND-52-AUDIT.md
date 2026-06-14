# R52 审计（Bulk succeeded_ids + 桌面 Bot 索引 parity）

> 日期：2026-06-12  
> 计划：[ROUND-52-PLAN.md](ROUND-52-PLAN.md)

## 主要矛盾（决定性）

**批量 API 只返回 `count` 时，前端无法知道哪些 ID 成功** — R51 用保守策略（count≠batchSize 则保留选中）避免误删选中态，但 Bot 多租户部分删除仍体验差。根治：后端返回 `succeeded_ids`，前端优先消费。

## 次要矛盾（盯住）

| 项 | 状态 |
|----|------|
| 桌面 Bot 与 Web Headless 列表/删除 parity | **R52 已补** `cmd_get_files` / `cmd_delete_file` 索引路径 |
| 桌面 Bot 全局搜索 | 仍走 GramJS，需 User 会话 |
| Bot 模式下载/上传 | 仍 gated 于 `transferReady`（设计如此） |
| `moveExecution.ts` 无独立单测 | 逻辑薄，由 `useFileOperations` 集成测覆盖 |

## 落地变更

| 区域 | 变更 |
|------|------|
| `api_routes.rs` | `BulkResponse.succeeded_ids`；Bot delete 逐条 push；User 全批返回全部 ID |
| `openapi.json` | `BulkResponse.succeeded_ids` schema |
| `filesPure.ts` / `files-pure.js` | `pickBulkSucceededIds`（API 优先，count 回退） |
| `files-core.js` | bulk delete/move 使用 `pickBulkSucceededIds` + `res.succeeded_ids` |
| `fs.rs` | Bot/索引权威时 `cmd_get_files` 走 DB；`cmd_delete_file` 索引删除 |
| `connection.ts` | `isServiceReady` / `isBotIndexReady` / `isBotTransportMode` |
| `Dashboard.tsx` | `serviceReady` vs `transferReady`；Bot 横幅；文件列表 enabled 于 serviceReady |
| `useFileOperations.ts` | `guardDelete` + `canIndexDelete`；`executeMoveGroups` 复用 |
| `moveExecution.ts` | drag-drop 与 bulk move 共享执行器 |

## 自反驳（最强反对意见）

1. **「succeeded_ids 是否足够？」** — Bot 多租户 skip 未授权行时现已精确返回；若 DB delete 静默失败仍不计入（与 count 一致）。反转条件：若未来 bulk move 也部分成功，需同样扩展字段。
2. **「桌面 Bot 列表是否真可用？」** — 依赖本地 API 已启动且 `transport_mode=bot`；API 未运行时仍回退 GramJS（需 User 会话）。复盘：启用 API + Bot 后应能无会话浏览。
3. **「guardDelete 会不会误开上传？」** — 删除与传输 guard 分离；上传/下载仍 `canTransfer`。OK。
4. **「moveExecution 无单测」** — 与 R51 拖放逻辑等价，回归靠 hook 测试；下轮可加 mock invoke。

## 验证（实测输出）

| 套件 | 结果 |
|------|------|
| `npm test`（`app/`） | **219 passed** |
| `npm run test:coverage` | **96.84% stmts / 86.76% branch**（lib+hooks） |
| `cargo test --features headless-server --lib` | **92 passed**（+2 BulkResponse） |
| Playwright 静态 | **41 passed**, 2 skipped |
| Playwright `E2E_API=1` | **43 passed** |

## 下一阶段信号（主要矛盾可能转移）

- 桌面 Bot **搜索/下载** 与 Web REST 对齐 → 需 `cmd_search_global` 索引路径或 HTTP 代理
- 后端 bulk **move** 部分成功时的 `succeeded_ids` / 失败明细
- 打包桌面 OS 拖放 + Bot 模式 E2E（当前 mock，无真实 Telegram 花费）

## 已知限制（设计内）

- 真实 Telegram 上传/下载/图片生成 API **未在 CI 调用**（省钱 mock）
- `moveExecution.ts` 未纳入 coverage 阈值目录（invoke 包装）
