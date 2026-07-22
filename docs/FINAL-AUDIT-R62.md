# 终局审计 R62 — 删除返回 shares_revoked + 桌面中文确认

> 2026-06-12 · 第六十二轮

## 反向审判

| 攻击 | 风险 | 处理 |
|------|------|------|
| R61 只说「分享已撤销」但无精确数 | 用户无法核对 | `PurgeIndexResult` · API `shares_revoked` · toast 显示「已撤销 N 条」 |
| 桌面删除确认仍是英文 | 体验不一致 | `formatSingleDeleteConfirmMessage` · 中文 title/按钮 |
| Web bulk 未读 API 字段 | 文案与后端不一致 | `files-core` 聚合 `res.shares_revoked` |
| `cmd_delete_file` 仍返回 bool | 桌面无法展示撤分享数 | 改为 `DeleteFileResult { deleted, shares_revoked }` |

## 变更摘要

| 模块 | 变更 |
|------|------|
| `file_access.rs` | `PurgeIndexResult { purged, shares_revoked }` |
| `api_routes.rs` | `BulkResponse.shares_revoked`（Bot/User delete） |
| `commands/fs.rs` | `DeleteFileResult` |
| `webPure.ts` / `web-pure.js` | 精确 toast + 单条删除确认 |
| `files-core.js` | 聚合 `shares_revoked` |
| `useFileOperations.ts` | 中文确认 + 精确 toast + `deleted` 校验 |

## 验证

```powershell
cd Telegram-Drive/app; npm test -- --run          # 296 passed
cd Telegram-Drive/app/src-tauri; cargo test --features headless-server --lib  # 140 passed
cd Telegram-Drive/tests/e2e; npx playwright test web-smoke.spec.ts web-metadata.spec.ts  # 52 passed, 2 skipped
```
