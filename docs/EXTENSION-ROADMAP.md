# 扩展方向（可选采纳）

> 第十六轮审计后整理；均可在 mock/本地环境验证，无需真实 Telegram API 调用。

## 1. 上传进度短期 Token（P1）✅ 第十七轮已落地

**问题：** legacy SSE/WS `/upload_events?pwd=` 将管理密码写入 URL（日志/历史泄露风险）。

**方案：** Headless 上传开始时签发 5 分钟 `upload_progress_token`（HMAC），SSE/WS 仅带 token；前端 `upload-core.js` 改 query 参数。

**验收：** Rust 单测 `progress_token_roundtrip`；集成测试 `test-api.ps1` 走 token 路径。

## 2. Bot health 探针去抖（P2）✅ 第十八轮已落地

**问题：** 每次 `GET /health` 打 Telegram live probe，间歇性 `ready=false`。

**方案：** `bot_test_connection_cached` 结果缓存 30s + stale-while-revalidate；连续 3 次失败才判不可用。

**验收：** `bot_probe_cache_*` Rust 单测；health/auth 走缓存路径。

## 3. 桌面拖放离线 UX（P2）✅ 第十七轮已落地

**问题：** 离线时仍可拖文件，drop 时才 toast。

**方案：** `FileCard`/`FileListItem` `draggable={sessionOnline && !isFolder}`；`SidebarItem` `dropEnabled`；Vitest 覆盖。

## 4. User 模式搜索索引（P3）✅ 第二十一轮完整闭环

**问题：** User 模式 search 仍 grammers 扫消息，慢且不可 mock；第十九轮增量索引在 `count>0` 时误把部分索引当完整索引（搜索漏文件、API GET 404）。

**方案：** 桌面 `cmd_upload_file` / 打开文件夹懒索引写入 `file_assets`；**Sync 按钮**（桌面）或 **刷新**（Web `files.html`）调用 rebuild 全量 purge+扫描并设置 `file_index_complete=1`；logout/新建/删 folder/切 transport 自动清 complete；search/list/get 仅在 complete 后走 DB（`asset_index_authoritative`）。

**验收：** `delete_all_file_assets_for_owner`、`asset_index_authoritative_user_requires_complete_flag` 单测；`POST /api/v1/files/rebuild-index`；REST User 上传/删除同步索引。

## 5. Web E2E smoke（P3）✅ 已纳入 CI + 第十九轮增强

**现状：** `docker-api.yml` 在容器 smoke 后跑 Playwright。

**方案：** 新增 dashboard `ready=false` 时上传按钮 disabled；`upload-core` token 静态断言。

**验收：** `tests/e2e/web-smoke.spec.ts`（mock health，无外部 Telegram）。

## 6. 第二十五轮后续（P2–P3）

### 6.1 Web 上传 folder 选择（P2）✅ 第二十六轮已落地

**问题：** `upload.html` / legacy `/upload` 无 folder UI；大文件 `merge_chunks` 忽略 `folder_id`。

**方案：** `upload-folder.js` + `dashboard.html`/`upload.html` 下拉；`upload-core.js` 传 `folder_id` 至 `/upload` 与 `/merge_chunks`；`parseUploadFolderId` Vitest。

**验收：** `web-smoke` 静态断言；`legacy_form::parse_optional_i64_field` Rust 单测。

### 6.2 桌面 User 登录一体化（P2）✅ 第三十二轮已落地

**问题：** 8550 不挂载静态 Web；切 User 后依赖 Headless :1334 浏览器登录，纯桌面用户可能困惑。

**方案 A（小）：** Settings 探测 `telegram.html` 不可达时提示「请在本应用 Auth 界面登录」。

**方案 B（大）：** 桌面 REST 挂载 `deploy/web`（含 `/telegram.html`），`resolve_desktop_web_static_dir` + Settings 优先打开 `:8550/telegram.html`。

**验收：** Rust `resolve_desktop_web_static_dir` 单测；`buildTelegramLoginCandidates` Vitest；[DESKTOP-API.md](DESKTOP-API.md) 静态节。

### 6.3 覆盖率门禁（P3）✅ 第三十二轮已落地（纯函数层）

**问题：** 全库 Vitest 多例，hooks 未测；`@vitest/coverage-v8` 未安装。

**方案：** `npm run test:coverage`；`vitest.config.ts` 对 `src/lib/*Pure.ts` + `src/utils.ts` + `queuePure.ts` 设 90% 门槛；hooks 队列逻辑第三十三轮已提取至 `queuePure.ts`。

### 6.4 视频轮询/流媒体 UX（P3）✅ 第三十四轮部分落地

**问题：** 大文件下载/流媒体等待时，部分路径仅有 queue 状态，无统一「处理中」banner。

**方案：** 桌面 `MediaPlayer` 增加 `waiting`→Buffering overlay + `transferUiPure` 单测；下载/上传队列共用进度文案格式化。

**验收：** `transferUiPure.test.ts`；切换媒体文件重置 stream 状态。

**剩余（第三十六轮部分缓解）：** Web `files-core` 下载有进行中态（`downloadingIds`、按钮「下载中…」、toast），但仍为 blob 全量拉取，无 determinate 进度条；Playwright 未断言 stream 404/503 响应体（仅静态 wiring）。
