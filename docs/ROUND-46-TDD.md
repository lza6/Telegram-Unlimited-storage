# Round 46 — Share 快捷按钮 + Transport 切换索引闭环

## 自反驳

| 质疑 | 结论 |
|------|------|
| 「右键菜单已有 Share，卡片按钮多余」 | **半真** — 右键是隐藏入口；网格/列表 hover 按钮与 Preview/Download 一致 |
| 「REST 切换已 invalidate，桌面不必再 invoke」 | **半真** — 双写 `cmd_invalidate_file_index` 保证 Tauri 直连与 REST 同库；并刷新 React Query |
| 「EmptyState 写拖放没问题」 | **假** — `dragDropEnabled: false` + `ExternalDropBlocker` 阻断外部拖放，文案误导 |
| 「应在本轮启用 Finder 拖放」 | **推迟** — 需 `dragDropEnabled: true` 与内部 move 拖放冲突，单独立项 |

## 落地

| 项 | 文件 |
|----|------|
| 网格卡片 Share 按钮 | `FileCard.tsx` |
| 列表行 Share 按钮 | `FileListItem.tsx` |
| FileExplorer 接线 | `FileExplorer.tsx` |
| Transport 切换 → invalidate + 清搜索 | `SettingsModal.tsx`, `Dashboard.tsx` |
| EmptyState 文案修正 | `EmptyState.tsx` |
| Web 设置切换提示 | `settings-core.js` |
| 单测 +4 | `FileCard.test.tsx`, `FileListItem.test.tsx` |

## 验证

```text
npm test          → 208 passed
cargo test --features headless-server --lib → 90 passed
```

## 下一阶段（R47）

- Tauri `dragDropEnabled` + 区分外部路径上传 vs 内部 file-id 移动
- Web `docs.html` 版本拉取失败可见化（minor）
