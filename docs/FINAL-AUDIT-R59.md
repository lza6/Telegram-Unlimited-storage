# 终局审计 R59 — Bot 双索引 / 分享 / 上传中止

> 2026-06-12 · 承接 R58 Web readiness 分离后的 P1 缺口

## 需求追踪矩阵（节选）

| 需求 | 实现 | 状态 | 证据 |
|------|------|------|------|
| Bot 删除不遗留 ghost 分享下载 | `purge_file_index_entry` | 已闭环 | `file_access.rs` + `fs.rs` + `api_routes.rs`；`purge_file_index_entry_removes_both_tables` |
| 分享创建需可下载 | `assert_share_create_allowed` / `assert_bot_downloadable` | 已闭环 | `share_api_routes.rs` · `commands/sharing.rs` · `bot_share_requires_bot_file_map` |
| 分享下载 Bot 校验 | `assert_share_download_allowed(..., bot_mode)` | 已闭环 | `share_routes.rs` |
| 桌面 Bot 移动明确拒绝 | `desktop_is_bot_mode` + `cmd_move_files` | 已闭环 | `local_api.rs` · `fs.rs` |
| SSE 失败停止分片浪费 | `chunkAbort` + `fetchWithRetry` signal | 已闭环 | `upload-core.js` · Playwright smoke |
| 文档与行为一致 | `DESKTOP-API.md` Bot 节 | 已闭环 | 本节 + README 链接 |
| 真实 Telegram 分享下载 E2E | 需用户密钥 | 未闭环 | CI mock；用户需自测 `/d/{token}` |
| Bot 逻辑移动（仅 DB folder_id） | 未实现 | 不适用/待定 | Telegram `forward_messages` 硬约束 |

## P0/P1 处理

| 级别 | 问题 | 处理 |
|------|------|------|
| P1 | 删 `file_assets` 留 `bot_file_map` → 分享仍可创建但下载 404 | **已修** purge 双表 |
| P1 | 单租户 Bot 分享无 `bot_file_map` 校验 | **已修** bot_mode 分支 |
| P1 | `cmd_move_files` Bot 误报 not connected | **已修** 显式文案 |
| P2 | SSE failed 后分片继续上传 | **已修** AbortController |

## 验证（2026-06-12）

- `npm test -- --run` → 288 passed
- `cargo test --features headless-server --lib` → 138 passed（含 3 个新 file_access 测试）
- `npx playwright test web-smoke.spec.ts web-metadata.spec.ts` → 46 passed, 2 skipped

## 剩余风险

1. **生产 OG/SEO**：仅 localhost 逻辑在 CI 验证；真实域名需部署后人工检查。
2. **Bot 上传路径**：仍依赖 User 会话 + GramJS；非本轮回合范围。
3. **索引不一致历史数据**：旧库可能仅有 `file_assets` 无 `bot_file_map`；创建分享现会 400，需用户 rebuild/sync。
