# 第四十八轮深度审查（桌面 OS 拖放上传 + Web 批量闭环 + OpenAPI）

> 日期：2026-06-12  
> 目标：补齐 R47 推迟项（Finder 外部拖放）、消除伪实现/死代码、同步 OpenAPI 与 `settings` 响应。

## 自反驳（修复前）

| 反驳 | 结论 |
|------|------|
| 「EmptyState 写拖放 = 已支持拖放」 | **假** — R46 去掉误导文案后，桌面仍无 OS 级 drop 接线 |
| 「ExternalDropBlocker 只挡 drop」 | **真** — 仅提示用 Upload，Finder 用户会认为产品坏了 |
| 「DragDropOverlay 算拖放功能」 | **假** — 从未挂载，纯死代码 |
| 「bulk move 一次失败就全失败」 | **半真** — Web 已按 folder 分 payload，但部分失败需 toast |
| 「OpenAPI 有 settings 就行」 | **假** — 缺 `effective_share_link_base` 字段文档，R47 API 与文档脱节 |
| 「metrics 非 404 即可用」 | **半真** — 5xx 应展示 HTTP 状态而非静默 |

## 落地修复

| 项 | 文件 / 行为 |
|----|-------------|
| Tauri OS 拖放上传 | `ExternalDropBlocker.tsx`：`getCurrentWebview().onDragDropEvent`，`drop` → `onUploadPaths` |
| 上传队列入队 | `useFileUpload.ts`：`enqueueUploadPaths`；`handleManualUpload` 复用 |
| Dashboard 接线 | `Dashboard.tsx`：`onUploadPaths={enqueueUploadPaths}` |
| 空文件夹文案 | `EmptyState.tsx`：桌面可拖文件 + Upload 按钮 |
| 删除死代码 | 移除未使用的 `DragDropOverlay.tsx` |
| Web 批量删除/移动 | `files-core.js`：分 payload 循环 + 部分失败 toast |
| 移动目标校验 | `parseMoveTargetFolderId` + `Number.isNaN` toast |
| Metrics 探测 | `settings-core.js`：非 ok 显示 `HTTP {status}`；修复重复 `else` 语法错误 |
| OpenAPI | `docs/openapi.json`：`SettingsResponse` + `effective_share_link_base` |

## 设计说明（非缺陷）

- `tauri.conf.json` 保持 `dragDropEnabled: false`：避免与仪表盘内 HTML5 文件拖拽（排序/移动）冲突；**外部文件**走 Tauri `onDragDropEvent`，不依赖全局 HTML5 drop。
- 浏览器 Vite 开发：`ExternalDropBlocker` 仍引导 Upload 对话框（无 OS 路径）。
- 拖入**文件夹路径**会被 `enqueueUploadPaths` 过滤，提示用 Upload Folder。

## 测试

| 套件 | 结果 |
|------|------|
| `app` vitest | **209 passed**（+`enqueueUploadPaths`） |
| `cargo test --features headless-server --lib` | **90 passed** |
| Playwright 静态 | **32 passed**, 2 skipped |
| Playwright API (`E2E_API=1`, headless 1334) | **34 passed** |

## 仍有意推迟（需真实 Telegram / 烧钱）

- 真实 Bot/User 上传、下载、分享创建 E2E
- 桌面 Tauri 拖放的手动验收（需在打包应用中拖 Finder 文件）
