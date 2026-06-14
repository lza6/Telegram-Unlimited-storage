# Round 39 — Connection Hook + E2E API 管线

## 主要矛盾

| 维度 | 内容 |
|------|------|
| **决定性** | R38 覆盖传输 hooks，但 `useTelegramConnection` 的 checking→online/offline 与 requireOnline 守卫仍无 mock 测试 |
| **牵引性** | renderHook + mock network/confirm/store；Playwright `E2E_API=1` 起 headless（User 模式、无 Bot getMe） |
| **阶段性** | R39 收口 connection + API E2E skip 消除；R40 可选 store 恢复 E2E、Web 下载进度 |

## 用户旅程 & 失败用例

### Connection Hook
1. 网络在线 + `cmd_check_connection` true → `online`
2. invoke 抛错 → `session_lost`
3. 网络离线 → `network_offline`，不 invoke
4. `handleSyncFolders` 非 online → toast + 不扫描
5. `forceLogout` → clean_cache + onLogoutParent
6. `handleLogout` confirm 取消 → 不 logout
7. initStore 恢复 folders / activeFolderId

### E2E API
1. `E2E_API=1` → headless User 模式，health 200，无 Telegram 外网
2. rebuild-index 无 auth → 401
3. 静态 `serve` 模式仍 skip API 用例

## 反转条件

- headless 启动 >120s → 先 `cargo build --bin telegram-drive-server`
- connection hook flaky → 轻量 TestSettings + mock useNetworkStatus
