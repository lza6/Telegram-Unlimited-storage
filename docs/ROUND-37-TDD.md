# 第三十七轮 TDD 方案（决定性 / 牵引性 / 阶段性）

## 1. 主要矛盾（决定性）

**卡点：** 可单测的纯函数层已覆盖主链路，但 **Web 下载状态机** 与 **上传错误分类** 仍散落在 `files-core.js` / `useFileUpload.ts` 中，无法 mock 验证，回归只能靠人工点 UI。

**牵引目标：** 把「一次点击不出逻辑错误」收敛为 **可失败的 Vitest + Playwright 静态断言**，不调用真实 Telegram API。

**阶段性：**

| 阶段 | 交付 | 验证 |
|------|------|------|
| P0 | `downloadPure` Web 下载守卫 + `download-pure.js` 镜像 | Vitest 100% + files-core 引用 |
| P1 | `uploadPure.classifyUploadFailure` + hook 接入 | Vitest + hook 行为不变 |
| P2 | 纯函数覆盖率补洞（files/transfer/queue/web/upload） | `npm run test:coverage` 门槛 |
| P3 | 文档同步 AUDIT / README / EXTENSION | 与代码一致 |
| P3 | Playwright 静态 serve + API 用例 skip | 24 passed / 2 skipped（无烧 API） |

## 2. 次要矛盾（先盯住不主攻）

- Hooks 全量 render 集成测试（需 mock Tauri listen/invoke）→ 第三十八轮
- Web blob 下载 determinate 进度条 → EXTENSION-UX-R37
- 真实 Telegram 转发 1:1 → 用户实网反馈

## 3. 第一性原理校验

| 不可简化事实 | 推论 |
|--------------|------|
| 不能烧 Telegram API | 只测纯函数 + 静态 wiring + Rust lib test |
| 用户要「第一次就对」 | 错误分类、防重复下载必须可单测 |
| 奥卡姆剃刀 | 不新建抽象层，只扩展现有 `*Pure.ts` + `deploy/web/assets/*-pure.js` |

## 4. 用户旅程 & 失败用例（先测后码）

### Web 单文件下载

| # | 场景 | 期望 |
|---|------|------|
| D1 | 同 id 已在 `inFlight` | 跳过，toast 重复提示 |
| D2 | 开始下载 | 按钮文案「下载中…」、disabled |
| D3 | 完成/失败 | 恢复「下载」、从 inFlight 移除 |
| D4 | 文件名缺失 | fallback `download` |

### 上传错误

| # | 场景 | 期望 status |
|---|------|-------------|
| U1 | `Transfer cancelled` | cancelled |
| U2 | `FILE_TOO_BIG` / `2 GB` | error + file_too_big toast |
| U3 | session lost 文案 | error + session_lost |
| U4 | 其他 | error generic |

## 5. 反转条件

- 若 Playwright 无法加载新 `download-pure.js` script 顺序 → 合并进 `files-pure.js`（更少 HTTP 请求）
- 若 `classifyUploadFailure` 与 hook 行为漂移 → 回滚 hook，仅保留纯函数供 Web 复用

## 6. 下一阶段主要矛盾可能转移

- P0 完成后 → **hooks mock 集成测试** 成为新主要矛盾
- 用户实网反馈移动/索引问题 → **REST bulk + rebuild-index** 集成 mock 提升优先级
