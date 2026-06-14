# Round 47 — 深度审查闭环（桌面 UX + Web 静默失败 + 分享基址）

## 自反驳

| 质疑 | 结论 |
|------|------|
| 「R46 后只剩 Finder 拖放」 | **假** — 快捷键与模态冲突、文件夹 Preview 误导航、Headless 分享基址误导均为真实 bug |
| 「REST 已 invalidate，桌面不必再管索引 UI」 | **半真** — 切换后须清 React Query + 搜索缓存，否则列表与 transport 不一致 |
| 「upload progress failed 很少发生可忽略」 | **假** — 用户会看到卡死在分片进度 |
| 「effective_share_base_url 就是 /d/* 基址」 | **假** — Headless `use_stream_port_for_shares=false` 时实际用 API Host |

## 修复矩阵

### 桌面

| 项 | 文件 |
|----|------|
| Share/Settings 打开时禁用全局快捷键；Escape 先关模态 | `Dashboard.tsx` |
| 会话离线关闭 ShareDialog | `Dashboard.tsx` |
| Bot 模式禁用 TopBar「Move to…」 | `TopBar.tsx`, `Dashboard.tsx` |
| 文件夹 Eye 按钮 → Open（与右键一致） | `FileExplorer.tsx`, `FileCard.tsx`, `FileListItem.tsx` |
| 拖放提示 z-index 高于侧栏 | `ExternalDropBlocker.tsx` |
| 移动失败展示 errMsg | `Dashboard.tsx` |
| 文件列表 error 展示 message | `FileExplorer.tsx` |
| ShareDialog / Settings 复制失败 toast | `ShareDialog.tsx`, `SettingsModal.tsx` |

### Web

| 项 | 文件 |
|----|------|
| 上传：空文件 toast、progress failed、poll 失败、config 失败、复制失败 | `upload-core.js` |
| QR start 网络错误 try/catch | `telegram-auth.js` |
| 分享域名加载失败 info toast | `share-domain.js` |
| 设置面板加载失败 toast + 字段清空 | `settings-core.js` |
| shares folder_id 校验 | `shares-core.js` |
| docs OpenAPI 版本 / curl 复制失败可见 | `docs.html` |
| toast `info` 样式 | `api-client.js`, `admin.css` |
| 分享基址 UI 分离 link vs stream | `settings.html`, `settings-core.js` |

### 后端

| 项 | 文件 |
|----|------|
| `effective_share_link_base`（Headless=API Host，桌面=14201） | `settings_routes.rs`, `server_http.rs`, `desktop_api_server.rs` |

## 验证

| 命令 | 结果 |
|------|------|
| `npm test` | **208 passed** |
| `cargo test --features headless-server --lib` | **90 passed** |

## 仍按设计保留

- 桌面 Finder 外部拖放 → `ExternalDropBlocker` 引导 Upload（R48：Tauri drag-drop 分流）
- `DragDropOverlay.tsx` 未挂载（死代码，可后续删除或接线）

## R48 候选

- 启用 Tauri `onDragDropEvent` 外部路径 → `useFileUpload` 队列
- Web bulk 部分失败分组报告
- 删除或接线 `DragDropOverlay.tsx`
