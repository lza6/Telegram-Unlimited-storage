# R53 计划（桌面 Bot 全局搜索 + 搜索门禁对齐）

> 主要矛盾：R52 已让 Bot 模式**列表/删索引**可用，但**全局搜索**仍要求 `sessionOnline` 且 `cmd_search_global` 未走 `asset_index_authoritative`，Bot 无 User 会话时搜索空白。

## TDD 清单

1. RED `shouldRebuildIndexBeforeGlobalSearch` — Bot 模式跳过 GramJS rebuild
2. GREEN `cmd_search_global` — `desktop_uses_asset_index` 时走 `search_file_assets`
3. GREEN `Dashboard` — 搜索 debounce 用 `serviceReady`；Bot 跳过 rebuild
4. VERIFY vitest / cargo / playwright

## 反转条件

- User 模式 + 索引未完成：仍 rebuild + Telegram fallback（现有行为）
- API 未启动的 Bot：搜索仍失败（与列表一致，需启用本地 API）
