# 第十六轮：第一性原理 + TDD 执行计划

## 1. 问题本质

**核心问题：** 用户要求「一次调用不出逻辑错误」——任何 API/UI 路径在 mock 环境下可验证、可回归。

**成功标准：**
- Bot/资产索引路径：`folder_id` 过滤与分页在 SQL 层正确（非内存假分页）
- 搜索 API：资产索引模式尊重 `folder_id`
- 前端纯函数（safeNext / safeHttpUrl / connection 态）有 Vitest 单测
- `cargo test` + `npm test` 全绿，无真实 Telegram 网络调用

## 2. 主要矛盾 vs 次要矛盾

| 类型 | 项 | 说明 |
|------|-----|------|
| **主要** | 资产索引 `folder_id` 分页/搜索 | 直接导致「第 2 页空列表」「搜错文件夹」 |
| **次要** | 拖放离线 UX、health 探针抖动 | 体验/观测，不阻塞单次 API 正确性 |
| **外部硬条件** | Telegram 真实 API 不可在 CI 调用 | 用 SQLite mock + Vitest 纯函数 |
| **内因** | 分页在 SQL 后内存 filter | 可通过 DB scoped 查询消除 |

## 3. 反转条件（若以下成立则本方案需重做）

- 若产品决定废弃 `file_assets` 索引改全走 grammers 实时拉取 → 本轮 DB 修复可降级
- 若 `folder_id=null` 语义改为「全部文件夹」而非「Saved Messages」→ 需改 SQL `IS NULL` 分支

## 4. TDD 清单（先测后实现）

### Rust / DB
- [x] `list_file_assets_scoped` 按 folder 分页，total count 正确
- [x] `search_file_assets` 带 folder 过滤
- [x] `parse_files_folder_scope` 解析边界（空/null/数字）

### TypeScript
- [x] `canTransferFiles('checking') === false`
- [x] `safeNext` 拒绝外部 URL / login 重定向
- [x] `safeHttpUrl` 拒绝 javascript:

### 集成（mock，无网络）
- [x] `cargo test --features headless-server --lib` → **73 passed**
- [x] `npm test` → **35 passed**

## 5. 执行顺序

1. DB 测试 + `list_file_assets_scoped` / `count_file_assets_scoped` / `search_file_assets` 扩展
2. `api_list_files` / `api_search_files` 接线
3. `webPure.ts` + Vitest + `connection.test.ts`
4. 文档：`AUDIT-CLOSURE.md` 第十六轮、`README.md` 摘要
5. 跑通验证命令并记录输出

## 6. 下一阶主要矛盾可能转移

- 上传进度 URL 带 `pwd` → 需后端 short-lived token（需协议变更）
- User 模式 search 仍走 grammers 实时扫描 → 可后续加本地索引
