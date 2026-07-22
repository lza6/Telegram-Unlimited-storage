# 终局审计 R61 — 删除→分享列表跨页闭环 + 删除 UX

> 2026-06-12 · 第六十一轮

## 反向审判（本轮攻击点）

| 攻击 | 风险 | 处理 |
|------|------|------|
| R60 后端已撤分享，但 Web「分享管理」页仍显示旧链接 | 用户以为链接有效 | `files-core` 删除后 `bumpSharesInvalidateStorage`；`shares-core` 监听 storage / 自定义事件 / visibility |
| 桌面删除后 Settings→Sharing 列表不刷新 | 同上 | `Dashboard` dispatch `td-shares-invalidate`；`SettingsModal` 监听 |
| 删除成功 toast 未说明分享已撤销 | 用户困惑、重复点撤销 | `formatDeleteSuccessToast` + 桌面 `useFileOperations` 中文提示 |
| 删除确认未提示分享连带影响 | 误操作 | `formatBulkDeleteConfirmMessage` 含「相关分享链接将一并撤销」 |

## 需求追踪

| 需求 | 状态 | 证据 |
|------|------|------|
| 删除后分享列表自动更新（Web 跨 Tab） | 已闭环 | `web-pure.js` · `shares-core.js` storage 监听 |
| 删除后分享列表自动更新（Web 同 Tab 切页） | 已闭环 | `visibilitychange` + localStorage stamp |
| 删除后桌面 Sharing 设置页刷新 | 已闭环 | `Dashboard.tsx` · `SettingsModal.tsx` |
| 删除 UX 说明分享撤销 | 已闭环 | `webPure.ts` · `useFileOperations.ts` |
| 测试与文档同步 | 已闭环 | vitest + Playwright smoke · 本文 · AUDIT-CLOSURE |

## 未在本轮解决（已知边界）

- `rebuild-index` 仅重建 `file_assets`，不写入 `bot_file_map`（Bot 映射仍靠 Bot 上传/下载注册）— 已在 R59/R60 文档说明
- 真实 Telegram `/d/{token}` 下载 E2E 仍依赖外部 Bot 凭证

## 验证命令

```powershell
cd Telegram-Drive/app; npm test -- --run          # 295 passed
cd Telegram-Drive/app/src-tauri; cargo test --features headless-server --lib  # 139 passed
cd Telegram-Drive/tests/e2e; npx playwright test web-smoke.spec.ts web-metadata.spec.ts  # 51 passed, 2 skipped
```
