# R53 审计（桌面 Bot 全局搜索 parity）

> 日期：2026-06-12 · 计划：[ROUND-53-PLAN.md](ROUND-53-PLAN.md)

## 主要矛盾

R52 后 Bot 可**浏览/删索引**，但全局搜索仍绑定 `sessionOnline` + GramJS `SearchGlobal` / rebuild — 无 User 会话时搜索恒为空。

## 落地

| 区域 | 变更 |
|------|------|
| `fs.rs` | `search_files_from_asset_index`；`cmd_search_global` 在 `desktop_uses_asset_index` 时走 DB |
| `searchPure.ts` | `shouldRebuildIndexBeforeGlobalSearch` — Bot 跳过 GramJS rebuild |
| `Dashboard.tsx` | 搜索门禁 `serviceReady`；Bot 不触发 `cmd_rebuild_file_index` |
| `PRODUCT-EXTENSION-IDEAS.md` | R52/R53 已落地项标注 |

## 自反驳

1. **索引未同步时 Bot 搜索可能漏文件** — 与 Web Bot 一致（依赖既有 index）；非本轮回范围。
2. **预览/下载仍要 User** — 设计内；搜索命中后点预览仍会 blocked。
3. **API 未启动** — `serviceReady` 为 false，搜索不跑（与 R52 列表一致）。

## 验证

| 套件 | 结果 |
|------|------|
| vitest | **221 passed** |
| coverage | **96.86% stmts / 86.81% branch** |
| cargo headless lib | **92 passed** |
| Playwright | **43 passed** |

## 下一阶段

- Bot 模式**下载**走本地 REST（免 GramJS）
- Bulk `skipped_ids` 审计字段
- 桌面 Dashboard 搜索 E2E（需 mock Tauri invoke）
