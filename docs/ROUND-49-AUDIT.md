# 第四十九轮深度审查（构建阻断 + 批量 Toast 逻辑 + OpenAPI 契约）

> 日期：2026-06-12  
> 目标：在 R48 拖放闭环后，消除**会误导用户**的 toast 逻辑、**桌面构建阻断**、Web 控制台与 OpenAPI 文档漂移。

## 自反驳（修复前）

| 反驳 | 结论 |
|------|------|
| 「R48 后一次调用不会错」 | **假** — 批量移动 API 全失败时仍弹「已在目标文件夹」 |
| 「部分删除失败 toast 够了」 | **半真** — 全失败时仍弹「已删除 0 条」成功样式 |
| 「ShareDialog 复制失败有 toast」 | **假** — `toast` 未 import，`tsc` 报错 |
| 「Web 用 X-Access-Pwd，OpenAPI 写 ApiKey 无妨」 | **假** — 集成方按文档会 401 |
| 「行内分享 = 分享页」 | **假** — 行内创建永久无密码链接，用户无感知 |

## 落地修复

| 项 | 文件 / 行为 |
|----|-------------|
| ShareDialog 构建 | `ShareDialog.tsx` 补 `import { toast } from 'sonner'` |
| 批量移动 toast | `files-core.js`：`bulkMoveByFolder` 返回 `{ total, failures }`；仅 `total===0 && !failures.length` 时提示「已在目标」 |
| 批量删除 toast | 仅 `total > 0` 显示成功；`total===0 && !failures` →「没有可删除的条目」 |
| 行内分享确认 | `createShare` 前 `confirm` 说明无密码永久链接 |
| 上传部分失败 | `upload-core.js`：`showToast(..., 'err')` + 透传 `TdApi.showToast` type |
| 登录健康检查 | `login.js`：session 自动跳转时 API 不可达显示错误 |
| Settings 健康 | `settings-core.js`：`loadHealth` 失败 toast |
| 分享域名同步 | `SettingsContext.tsx`：`cmd_set_ui_share_domain` 失败 toast |
| OpenAPI | `AccessPwdAuth` 补全 admin 路由；`BulkResponse` / `NetworkResponse` / `SettingsUpdateResponse` |

## 测试

| 套件 | 结果 |
|------|------|
| `app` vitest | **209 passed** |
| `cargo test --features headless-server --lib` | **90 passed** |
| Playwright 静态 | **35 passed**, 2 skipped |
| Playwright API | **37 passed** |

## 已知非缺陷（文档化）

- ~~批量删除/移动**部分成功**时仍清空整批选中项~~ → **R50 已修复**（仅移除 `succeededIds`）
- `tsc --noEmit` 在部分测试 fixture 类型上仍有历史告警（vitest 全绿）
- 真实 Telegram 上传/下载 E2E 仍不跑（烧钱）
