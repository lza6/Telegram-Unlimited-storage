# 第五十一轮深度审查（Bulk count 与选中态一致性 + 拖放移动 parity）

> 日期：2026-06-12  
> 计划：[ROUND-51-PLAN.md](ROUND-51-PLAN.md)

## 第一性原理：主要矛盾

**决定性矛盾**：一次 bulk 操作后，UI 选中态必须反映后端**真实成功条数**，不能将 HTTP 200 等同于「本批每个 ID 都成功」。

| 原理 | 结论 |
|------|------|
| 不可简化事实 | Bot bulk delete 仅返回 `count`，无 per-id 明细 |
| 奥卡姆剃刀 | 在 count ≠ batchSize 时**不 deselect**，比猜测哪些 ID 成功更安全 |
| 贝叶斯更新 | R50 的 `succeededIds` 在 Bot 部分跳过时产生**假阴性 UI**（R51 修正） |

## 自反驳（最有力反驳观点）

| 反驳 | 裁决 |
|------|------|
| 「R50 已闭环 partial bulk」 | **假** — `count=2/3` 时仍 deselect 3 个 ID |
| 「桌面 drag-drop 与 TopBar 移动一致」 | **假** — 单 try/catch，一组失败导致列表/选中不同步 |
| 「部分成功应关模态让用户继续」 | **半真** — 部分失败时关模态会隐藏剩余选中；改为仅全成功时 `onSuccess` |
| 「disabled 按钮 = 足够反馈」 | **假** — 离线时 bulk 按钮 disabled 无法触发 `ensureReadyForAction` toast |

**反转条件**：后端若新增 `succeeded_ids[]`，应优先按明细 deselect；本 helper 作 fallback。

**下一阶段矛盾转移信号**：用户投诉「桌面 Bot 模式无法管理文件而 Web 可以」→ 主攻 `sessionOnline` 与 `cmd_*` bot 路径 parity。

## 落地修复

| 项 | 文件 |
|----|------|
| `resolveBulkBatchSucceededIds` | `filesPure.ts` + `files-pure.js` + 单测 |
| Web bulk deselect 按 count | `files-core.js` + `partialBatches` toast |
| Web bulk 按钮可点 + ensureReady toast | `updateBulkUi` 仅无选中时 disabled |
| 桌面 drag-drop per-group | `Dashboard.handleDropOnFolder` |
| 部分移动不关模态 | `useFileOperations.handleBulkMove` |
| 浏览器 drop 离线反馈 | `ExternalDropBlocker` + 单测 |
| WS 进度 fallback poll | `upload-core.js` `beginWsStatusPoll` |

## 测试（已实际运行）

| 套件 | 结果 |
|------|------|
| vitest | **215 passed** |
| vitest coverage (app lib/hooks) | **97.09% stmts / 87.17% branch** |
| Playwright 静态 | **41 passed**, 2 skipped |
| Playwright API | **43 passed** |
| cargo `--features headless-server --lib` | **90 passed**（R50 基线，本轮回未改 Rust） |

## 仍有意为之 / 待产品决策

- **桌面 Bot 模式**：Web Headless 可用 bot index bulk delete；桌面 `cmd_*` 仍依赖 User GramJS（见 [PRODUCT-EXTENSION-IDEAS.md](PRODUCT-EXTENSION-IDEAS.md)）
- **count 不匹配时**：保留整批选中（安全侧），用户需刷新后重试
- **真实 Telegram** E2E 不跑（烧钱）
