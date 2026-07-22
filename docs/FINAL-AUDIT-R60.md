# 终局审计 R60 — 删除/分享链路深度闭环

> 2026-06-12 · 承接 R59「双索引校验」后的上下游断裂

## 最强自我反驳（R59 遗留）

| 攻击 | 危险 | R60 处理 |
|------|------|----------|
| 「删了索引分享还在」 | 用户以为链接有效，下载 403 | `purge_file_index_entry` → `revoke_shares_for_message_id` |
| User 模式删 Telegram 消息只删 `file_assets` | `bot_file_map` 幽灵行 | User delete/bulk 改走 `purge_file_index_entry` |
| API 返回 `NOT_DOWNLOADABLE` 英文 | Web/桌面用户不知下一步 | `sharePure.ts` + `share-pure.js` + ShareDialog 测试 |
| `files.html` 未加载 `share-pure.js` | 行内分享仍显示原始错误 | HTML script 顺序修复 |

## 需求追踪（R60 增量）

| 需求 | 状态 | 证据 |
|------|------|------|
| 删除文件后分享链接失效 | **已闭环** | `sharing_core::revoke_shares_for_message_id`；`purge_file_index_entry` 测试 |
| User/Bot 删除均清双表 | **已闭环** | `fs.rs` · `api_routes.rs` bulk delete |
| 分享失败可行动文案 | **已闭环** | `sharePure.test.ts` · ShareDialog test · Playwright |
| Web 页面加载 share-pure | **已闭环** | `files.html` · `shares.html` · smoke |

## 验证

- `cargo test --features headless-server --lib` → 140 passed（+2 sharing/file_access）
- `npm test -- --run` → 292 passed（+3 sharePure + ShareDialog）
- Playwright smoke+metadata → 49 passed, 2 skipped

## 仍部分闭环

- 真实 Bot 凭证下的 `/d/{token}` 下载（需用户环境）
- 仅有 `file_assets`、无 `bot_file_map` 的历史行：创建分享会 400 + 中文指引，需 rebuild/sync
