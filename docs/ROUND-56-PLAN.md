# R56 计划 — Bot 模式分享 parity

## 主要矛盾（决定性）

R55 后 Bot 可预览，但 **分享 UI 仍绑 `sessionOnline`**，而 `cmd_create_share` 仅为 DB 写入，无需 GramJS → 「能看不能分享」。

## 牵引性

FileCard / FileListItem / ContextMenu / ShareDialog / `handleShare` 五处同构门禁需与 `downloadReady` 对齐。

## 阶段性

R54 下载 → R55 预览 → **R56 分享** → R57 移动/上传（仍须 User）。

## 方案（奥卡姆）

**A（推荐）**：`canShareFiles` ≡ `canDownloadFiles`；前端 `shareReady` 解耦，**零 Rust 改动**。

**B**：REST `/api/v1/shares` 经桌面 invoke — 重复链路，否决。

## TDD 清单

1. RED `connection.test` — `canShareFiles` Bot 路径
2. RED `ShareDialog.test` — `shareReady=true` + `sessionOnline=false` 可生成
3. RED `FileCard.test` — `shareEnabled` 无 transfer
4. GREEN 组件 wiring + Dashboard banner
5. VERIFY vitest / cargo / E2E 增量（见 E2E-CHECKPOINTS.md）

## 反转条件

- 本地 API 未就绪 → `shareReady=false`
- 多租户 share 下载需 asset 行 — 已有 `assert_share_download_allowed`

## 下一阶段信号

「能分享不能移动」→ bulk move Bot 分支
