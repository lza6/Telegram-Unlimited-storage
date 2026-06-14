# R56 审计 — Bot 模式分享 parity

> 计划：[ROUND-56-PLAN.md](ROUND-56-PLAN.md)

## 主要矛盾

分享按钮与 ShareDialog 误绑 `sessionOnline`，而 `cmd_create_share` 仅写 SQLite，**无需 GramJS**。

## 自我反驳

| 声称 | 反驳 | 处置 |
|------|------|------|
| 「需要 Rust Bot 分支」 | `sharing_core::create_share` 已是 DB-only | **零 Rust 改动** |
| 「分享下载走 Telegram」 | `/d/{token}` 经 `assert_share_download_allowed` + asset index | 已有，R54/R55 流路径覆盖 |
| 「Settings 分享列表也要 gate」 | `cmd_list_shares` 无 GramJS 依赖 | 保持可加载 |

## 落地

| 层 | 变更 |
|----|------|
| `connection.ts` | `canShareFiles` ≡ `canDownloadFiles` |
| Dashboard | `shareReady` / `shareBlockedMessage` / banner |
| ShareDialog | `shareReady` 替代 `sessionOnline` |
| FileExplorer → Card/List/Menu | `shareEnabled` + `shareBlockedTitle` |

## 仍保留（次要）

- 上传 / 移动 / 拖放 → User 会话
- Telegram 公开频道链接复制 → 仍依赖 folder username（非 REST share）

## 反转条件

- API health 掉线 → `shareReady=false`，按钮 disabled + toast
- 多租户无 asset 行 → 分享链接创建成功但下载 403（已有后端校验）

## 下一阶段

- 「能分享不能移动」→ bulk move Bot（R57）

## 验证

| 套件 | 结果 |
|------|------|
| vitest | **237 passed** |
| coverage (lib+hooks) | **96.8% stmts / 86.87% branch** |
| Playwright share 增量 | **3 passed** |
| cargo headless `--lib` | **94 passed** |
