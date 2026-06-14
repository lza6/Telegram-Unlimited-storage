# 扩展方向建议（第三十七轮 · 采纳自选）

> 以下为产品/UX 扩展方案，**未在本轮实现**；按需采纳。

## A. Web 大文件下载进度（P1 体验）

- **现状：** blob 全量拉取，仅有「下载中…」无百分比
- **方案：** `fetch` + `ReadableStream` 读 `Content-Length`，`downloadPure.computeDownloadPercent(read, total)` 更新按钮/toast
- **约束：** Vitest mock `Response` body stream，不连真实 API
- **验收：** 用户可见 `45%` 类文案；失败仍 toast 错误

## B. 桌面 Hooks 集成测试层（P1 质量）

- **方案：** `@testing-library/react` `renderHook` + mock `@tauri-apps/api/core` invoke/listen
- **覆盖：** upload cancel/retry/clear、download 并发槽、connection `checking→online`
- **验收：** 不启动 Tauri，CI 全绿

## C. 统一传输队列组件（P2 维护）

- **方案：** `TransferQueuePanel<T>` 抽取 Upload/Download 重复 JSX
- **风险：** 改动面大；建议 hooks 测稳后再做

## D. 小白 onboarding（P2 SaaS）

- **Web：** `dashboard.html` 已有 Bot 三步；可增 User 模式「去 telegram.html 登录」一步卡片
- **桌面：** 首次 `connectionStatus=checking` 时 Sidebar 显示「正在检测会话…」

## E. 性能（P3）

- 虚拟列表已覆盖桌面；Web `files.html` 分页 25 条已够用
- 索引 rebuild 可加「后台静默 rebuild」避免搜索前阻塞 toast
