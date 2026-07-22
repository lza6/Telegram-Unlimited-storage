# E2E 检查点登记（增量回归）

> 避免每轮全量 Playwright；**动到某区块才重跑对应用例**。

## 用法

1. 功能闭环后在本表追加一行（日期 + 套件 + 用例范围）
2. 下次改同一文件/域时，先跑「关联 E2E」列
3. 发版前仍建议全量：`npx playwright test` + `E2E_API=1`

## 登记

| 域 | 最后全绿日期 | vitest 锚点 | Playwright 关联 | 备注 |
|----|-------------|-------------|-----------------|------|
| connection gates | 2026-06-12 | `connection.test.ts` | — | download/preview/share |
| keyboard shortcuts | 2026-06-12 | `useKeyboardShortcuts.test.ts` | — | R55 gap |
| Bot preview UI | 2026-06-12 | `FileCard.test.tsx` | — | previewEnabled |
| Bot share UI | 2026-06-12 | `ShareDialog.test.ts`, `FileCard` share | `web-smoke -g share` | R56 |
| **web metadata** | 2026-06-12 | `webMetaPure.test.ts` | `web-metadata.spec.ts` | R57 |
| **web readiness split** | 2026-06-12 | `webPure.test.ts` readiness | `web-smoke` + metadata | R58 |
| Share dialog effect | 2026-06-12 | — | — | shareReady 清 shareFile |

**机器可读登记**：[`tests/mocks/pass-registry.json`](../tests/mocks/pass-registry.json) — 改动 `files` 列时只重跑对应 vitest/e2e。

## 命令速查

```powershell
cd Telegram-Drive/app; npm run test -- --run
cd Telegram-Drive/tests/e2e; npx playwright test web-smoke.spec.ts -g "share"
cd Telegram-Drive/app/src-tauri; cargo test --features headless-server --lib
```
