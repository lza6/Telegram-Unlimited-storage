# Round 45 — 全栈深度审查与静默失败修复

## 自反驳（审查前最强质疑）

| 质疑 | 若成立意味着什么 |
|------|------------------|
| 「R44 后已可上线，再审查是重复劳动」 | **假** — 多轮迭代后易出现「API 已接但失败静默吞掉」的伪闭环 |
| 「空 catch 是故意的非致命设计」 | **半真** — 非致命不等于用户无感知；刷新/搜索失败应可见 |
| 「OpenAPI 写 auto 无所谓」 | **假** — 集成方按文档发 `auto` 会稳定 400 |
| 「桌面外部拖放应支持上传」 | **需产品决策** — 当前 `ExternalDropBlocker` 明确引导到 Upload 按钮，非 stub |
| 「cmd_invalidate_file_index 未调用是 bug」 | **半真** — 桌面用 `invalidateQueries` + rebuild；专用命令为预留，非断链 |

## 审查范围

- **Web**：8 页 HTML + 4 个 `*-core.js` + `api-client` / `telegram-auth`
- **桌面**：Dashboard 全动作 → hooks → Tauri invoke
- **后端**：`route_registry.rs` 39 路由 ↔ `openapi.json`；`api_routes` / `commands` 无 `todo!`/`501`
- **测试**：Vitest 205 + Rust 90 + Playwright 34（含 API）

## 审查结论矩阵

| 链路 | 状态 | 说明 |
|------|------|------|
| Web 登录 `/verify` + health | ✅ 真实 | `login.js` |
| Web 上传 chunk/merge/progress | ✅ 真实 | `upload-core.js` → headless 1334 |
| Web 文件列表/搜索/下载/分享 | ✅ 真实 | `files-core.js` REST |
| Web 批量删除/移动 | ✅ 真实 | `POST /api/v1/files/bulk` + transport 守卫 |
| Web 索引 rebuild（刷新/搜索/设置） | ✅ 已补强 | 失败现 toast（R45） |
| Web Telegram 手机/QR 登录 | ✅ 已补强 | QR 手动轮询错误可见（R45） |
| Web 分享创建/撤销 | ✅ 真实 | `shares-core.js` |
| Web 设置 transport/网络/域名/rebuild | ✅ 真实 | `settings-core.js` |
| 桌面 上传/下载队列 | ✅ 真实 | hooks + `cmd_*` |
| 桌面 删除/移动/搜索/预览 | ✅ 真实 | `useFileOperations` + Dashboard |
| 桌面 分享 | ✅ 真实 | ShareDialog + Settings 列表撤销 |
| 桌面 外部拖放上传 | ⚪ 产品设计 | 阻断并提示用 Upload 按钮 |
| REST rebuild-index（Bot） | ⚪ 模式限制 | `400 NOT_SUPPORTED`，非 stub |
| REST bulk move（Bot） | ⚪ 模式限制 | Web/桌面均有守卫 |
| `cmd_invalidate_file_index` | ⚪ 未接线 | 索引靠 rebuild + query invalidate |
| OpenAPI transport `auto` | ✅ 已修 | enum 仅 `bot`/`user`（R45） |

## R45 修复项

| 文件 | 修复 |
|------|------|
| `webPure.ts` / `web-pure.js` | `rebuildIndexShouldSurfaceBackgroundFailure` + 失败文案 |
| `files-core.js` | rebuild 失败 toast；`loadMoveFolders` 失败 toast |
| `telegram-auth.js` | `#qr-poll` 错误展示 + `stopQrPoll` |
| `searchPure.ts` + `Dashboard.tsx` | 全局搜索前 rebuild 失败 `toast.info` |
| `docs/openapi.json` | 移除无效 `auto` mode |
| `web-smoke.spec.ts` | +3 接线断言 |

## 验证（2026-06-12）

| 命令 | 结果 |
|------|------|
| `npm test` | **205 passed** |
| `cargo test --features headless-server --lib` | **90 passed** |
| E2E 静态 | **32 passed**, 2 skipped |
| E2E API | **34 passed** |

## 反转条件

- 用户要求外部拖放直传 → 需 Tauri dialog + `useFileUpload` 接入 `ExternalDropBlocker`
- Bot 模式也要 rebuild-index → 需产品定义 Bot 索引语义后再开 API
- 索引失败 toast 太吵 → 可改为 banner 一次性，保留 `webPure` 门控

## 下一阶段（R46 候选）

- 桌面 `FileCard` 增加 Share 快捷按钮（现仅右键菜单）
- transport 切换后自动触发 `cmd_invalidate_file_index`（若索引 API 扩展）
- Web `docs.html` 可选 `requireLogin` 或隐藏未登录 logout
