# Round 44 — TransferQueuePanel 抽取 + vpn_optimizer 全面 try_read

## 主要矛盾

| 维度 | 内容 |
|------|------|
| **决定性** | Upload/Download 队列 JSX ~90% 重复，维护双份易漂移；headless 启动时 `blocking_read` 在 tokio 内 panic |
| **牵引性** | `TransferQueuePanel<T>` 统一进度条/取消/重试；`vpn_config()`/`proxy_config()` 统一非阻塞读 |
| **阶段性** | R44 完成 EXTENSION-UX C + R42/43 headless 稳定性余项 |

## 先测后写

1. `TransferQueuePanel` 空队列 null + 进度行渲染 ✅
2. 现有 `UploadQueue` / `DownloadQueue` 测试仍绿（行为不变） ✅
3. `vpn_optimizer` tokio 内 `connect_timeout_secs` 等不 panic ✅
4. 全库无 `blocking_read` 残留 ✅

## 实现摘要

- **`TransferQueuePanel.tsx`**：共享队列 UI（header、Cancel All、Clear Finished、进度、错误行）
- **`UploadQueue.tsx` / `DownloadQueue.tsx`**：薄包装，保留各自 status 指示器与定位 class
- **`TransferQueuePanel.test.tsx`**：+2 冒烟
- **`vpn_optimizer.rs`**：`vpn_config()` / `proxy_config()` 用 `try_read` + default；+1 async 安全测试

## 验证结果（2026-06-12）

| 命令 | 结果 |
|------|------|
| `cargo test --features headless-server --lib` | 90 passed |
| `npm test`（app） | 201 passed |
| `npm run test:coverage` | 97.35%+ stmts（hooks/lib 维持 ≥95%） |
| `tests/e2e` 静态 | 30 passed, 2 skipped |
| `tests/e2e` API | 32 passed, 0 skipped |

## 反转条件

- 上传/下载 UI 需分叉（例如下载专属批量操作）→ 在 Panel 增加 slot，勿回退复制 JSX
- VPN 配置读失败需显式报错而非 default → 记录 warn 并返回 default（当前行为）

## 下一阶段（R45 候选）

- `useTransferQueue` 合并 upload/download hooks（大 refactor，需 characterization tests）
- Web `files.html` 虚拟列表（P3，当前分页够用）
