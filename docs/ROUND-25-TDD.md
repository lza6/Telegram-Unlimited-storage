# 第二十五轮 TDD 计划 — 可测纯函数 + User 登录引导

> 2026-06-08 · 技能：First Principles Thinking、test-driven-development、javascript-testing-patterns

## 1. 主要矛盾（决定性）

**Web/桌面共用的 folder 作用域逻辑仍内联在 `files-core.js`，Vitest 无法覆盖 → 回归只能靠 Playwright 静态字符串断言，无法证明分组/URL 正确性。**

| 维度 | 判断 |
|------|------|
| 牵引性 | 第二十三～二十四轮修复的 bulk delete / download peer 若再次漂移，无单测会在生产才暴露 |
| 阶段性 | 索引/SearchGlobal 已闭环；本轮聚焦 **测试可观测性** 与 **桌面 User 切换 UX** |

**反转条件**：若 `filesPure` 与 `files-core.js` 双份逻辑再次分叉，则本轮收益为负 —— 必须通过「TS 源 + JS 镜像 + 同一套 Vitest」约束。

**复盘时间**：下一轮审计时检查 `files-pure.js` 是否与 `filesPure.ts` 行为测试一致。

**下一阶段主要矛盾可能转移**：Web 上传 `folder_id` UI、全库 80% 覆盖率（当前仅关键纯函数路径）。

## 2. 次要矛盾（盯住不主攻）

- 桌面 REST（8550）不挂载静态 Web → 无法在本机 API 端口打开 `/telegram.html`
- 全库 Vitest 覆盖率远低于 80%（hooks/组件未测）
- 真实 Telegram SearchGlobal / forward 仍依赖用户 live 验证

## 3. 第一性原理

|  ground truth | 推论 |
|---------------|------|
| `folder_id = null` 表示 Saved Messages | bulk/delete/download 必须按文件索引 peer，不能假定当前 UI folder |
| 烧钱 API 不得进 CI | 纯函数 + mock DB/SQLite + 静态路由契约 |
| 桌面与 Headless 端口分离 | User 登录引导默认打开 Headless `:1334/telegram.html`，并 toast 说明无 Headless 时用桌面主界面登录 |

## 4. 方案对比

| 方案 | 优点 | 缺点 | 建议 |
|------|------|------|------|
| A. 抽取 `filesPure.ts` + `files-pure.js` | 与 `webPure` 模式一致，Vitest 可测 | 需保持双文件同步 | **采用** |
| B. 仅加 Playwright 断言 | 改动小 | 不测行为，只测字符串 | 作为补充保留 |
| C. 桌面 API 挂载静态 Web | 8550 可开 telegram.html | 架构变更大、与 DESKTOP-API 文档冲突 | 不采用 |

## 5. TDD 清单（先红后绿）

### 5.1 `filesPure.ts`

- [ ] `buildBulkDeletePayloads` — 多 folder 分组，Saved Messages 不含 `folder_id`
- [ ] `buildFileDownloadUrl` — 有/无 `folder_id` query
- [ ] `buildTelegramLoginUrl` — base + next 编码

### 5.2 `files-core.js`

- [ ] 使用 `TdFilesPure.buildBulkDeletePayloads` 替代内联 Map
- [ ] 下载 URL 走 `TdFilesPure.buildFileDownloadUrl`

### 5.3 `SettingsModal.tsx`

- [ ] 切 User 成功后 `shell.open('http://127.0.0.1:1334/telegram.html?next=...')`
- [ ] toast 提示 Headless 未运行时的备选（桌面主界面登录）

### 5.4 Rust

- [ ] `telegram_peer_id_to_folder_id(Peer::Chat)` → `None`

### 5.5 回归

- [ ] `cargo test --features headless-server --lib`
- [ ] `npm test`
- [ ] Playwright smoke 增加 `files-pure.js` 断言

## 6. 索引未完成时搜索行为（文档化）

| 路径 | index incomplete | 行为 |
|------|------------------|------|
| Web `GET /api/v1/files/search` | 否 | 走 Telegram 实时扫描（非 authoritative DB） |
| Web search + User | 是（complete） | DB authoritative |
| 桌面 `cmd_search_global` | N/A | 始终 Telegram API；结果 `folder_id` 经 `telegram_peer_id_to_folder_id` |
| Web Enter 搜索 | — | 先 `rebuild-index`（非致命失败则继续） |
