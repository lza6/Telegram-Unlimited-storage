# 第五十轮深度审查（部分批量选中 + 离线拖放反馈 + 桌面批量移动 parity）

> 日期：2026-06-12  
> 目标：关闭 R49 文档中遗留的「部分成功仍清空整批选中」、桌面离线拖放静默忽略、桌面 bulk move 单 try/catch 导致部分失败时误清选中等问题。

## 自反驳（修复前）

| 反驳 | 结论 |
|------|------|
| 「R49 批量 toast 已闭环」 | **半真** — API 部分成功时 Web 仍 `clearSelection()`，用户以为失败项也被移走/删除 |
| 「离线时拖放会被 uploadEnabled 挡住」 | **假** — Tauri drop 事件被静默丢弃，用户无任何反馈 |
| 「桌面 bulk move 与 Web 一致」 | **假** — 单组失败整批 catch；`moved>0` 仍 `setSelectedIds([])` |
| 「分享域名 PUT 成功就够了」 | **半真** — 刷新 GET 失败时用户不知道服务端视图是否已同步 |

## 落地修复

| 项 | 文件 / 行为 |
|----|-------------|
| Web 部分批量选中 | `files-core.js`：`bulkDeleteByFolder` / `bulkMoveByFolder` 返回 `succeededIds`；成功项从 `state.selected` / `selectedMeta` 逐条移除 |
| 桌面离线拖放 | `ExternalDropBlocker.tsx` 新增 `onUploadBlocked`；`Dashboard.tsx` 会话未就绪时 toast |
| 桌面 bulk move | `useFileOperations.ts`：按 folder 分组 per-group try/catch；仅 prune 已移动 `oldMessageIds`；部分失败 toast；`moved===0 && !failures` 才 info |
| 分享域名刷新 | `share-domain.js`：PUT 成功但 GET 刷新失败 → info toast |
| 上传 poll 失败 | `upload-core.js`：连续 poll 失败 toast 类型 `'info'` → `'err'` |

## 测试

| 套件 | 结果 |
|------|------|
| `app` vitest | **210 passed**（含 partial bulk move 单测） |
| `cargo test --features headless-server --lib` | **90 passed** |
| Playwright 静态 | **38 passed**, 2 skipped |
| Playwright API (`E2E_API=1`) | **40 passed**（含 R50 静态断言） |

## 已知非缺陷（文档化）

- Web bulk 的 `succeededIds` 在 **count === batchSize** 时 deselect；count 不匹配时保留选中（Bot 模式无 per-id 明细）
- 浏览器 dev 模式下 OS 拖放仍提示使用 Upload 按钮（Tauri API 不可用）
- 真实 Telegram 上传/下载/分享 E2E 仍不跑（烧钱）
- 打包桌面拖放需用户本机手动 QA
- **桌面 Bot 模式** 与 Web Headless 能力不对等 — 见 [PRODUCT-EXTENSION-IDEAS.md](PRODUCT-EXTENSION-IDEAS.md)
