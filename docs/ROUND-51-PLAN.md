# R51 计划（TDD + 第一性原理）

> 日期：2026-06-12  
> 主要矛盾：**用户一次批量操作后，UI 选中态/列表态必须与后端真实结果一致**（不可假定 HTTP 200 = 全部 ID 成功）。

## 决定性判断

| 层级 | 内容 |
|------|------|
| 主要矛盾 | Bot bulk delete 返回 `count < file_ids.length` 时，R50 仍把整批 ID 从选中移除 → **假成功** |
| 次要矛盾 | 桌面 drag-drop 移动未继承 R50 per-group 容错；离线 bulk 按钮 disabled 无 toast |
| 外部硬条件 | 后端无 per-id 明细；User 模式 bulk 全成全败 |
| 内因 | 前端用「请求成功」代替「每条 ID 成功」 |

## TDD 清单

1. **RED** `resolveBulkBatchSucceededIds` 单测（count 不匹配 → 空 succeededIds + partialBatch）
2. **GREEN** `filesPure.ts` + `files-pure.js` + `files-core.js` 接入
3. **RED** `handleBulkMove` 部分失败时不应 `onSuccess` 关模态
4. **GREEN** `useFileOperations.ts` + 单测
5. **GREEN** `Dashboard.handleDropOnFolder` per-group try/catch + prune movedOldIds
6. **GREEN** `ExternalDropBlocker` 浏览器 drop 离线 toast + 单测
7. **GREEN** Web bulk 按钮点击 guard toast（handler 内检查 serviceReady）
8. **VERIFY** vitest / cargo / playwright

## 反转条件

- 若后端新增 `succeeded_ids[]`，应改为按明细 deselect，本 helper 可保留为 fallback
- 若桌面 Bot 模式产品定位为「仅 Web Headless」，则 Bot 桌面 parity 降为文档化而非实现

## 下一阶段矛盾转移信号

- 桌面 Bot 与用户传输模式 parity 成为用户投诉主因 → 主攻 `sessionOnline` + `cmd_*` bot 路径
