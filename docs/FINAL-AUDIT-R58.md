# 终局闭环总审计（R58）

> 日期：2026-06-12 · 范围：Telegram-Drive 全栈（桌面 Tauri + Headless API + Web 控制台）

---

## 1. 任务目标与上下文重建

### 真实目标
将项目推进到「可交付、可调用、可维护」状态：Bot/User 双模式 parity、Web SEO、增量 E2E 登记、≥80% 测试覆盖、CI 不调真实 Telegram/API。

### 显式需求（摘录）
| 需求 | 来源 |
|------|------|
| Bot 模式下载/预览/分享与 User 解耦 | R54–R56 |
| 无 `sessionOnline` 误门禁 | R55/R56 gap |
| Web 元数据 canonical/OG/robots | R57 + UI 规则 |
| Mock 优先、真实 API 由用户验收 | 全上下文 |
| 增量 E2E pass-registry | R57 |
| 文档与代码同步 | 全上下文 |

### 隐式需求
- 一次调用可跑通（最少步骤）
- 假功能/伪闭环禁止包装为完成
- 新环境按 README 可启动
- 调用方 OpenAPI/示例与行为一致

### 非功能性
兼容性 · 稳定性 · 可排障 · 可验证 · 可维护

---

## 2. 需求追踪矩阵

| 需求 | 实现位置 | 状态 | 证据 | 缺口/动作 |
|------|----------|------|------|-----------|
| Bot 下载 | `local_api.rs`, `connection.ts`, `useFileDownload` | **已闭环** | vitest + cargo 94 | 桌面须启用 Local API + ACCESS_PWD |
| Bot 预览/流 | `preview.rs`, `previewReady` | **已闭环** | R55 audit | 同上 loopback 依赖 |
| Bot 分享创建 | `sharing.rs`, `shareReady`, Web `ensureApiAvailable` | **已闭环** | ShareDialog test, R58 web | 分享下载需 `bot_file_map` |
| Bot 批量删除（Web） | `files-core.js` + API | **已闭环** | R58 分离 gate | — |
| Bot 批量移动 | `fs.rs` forward_messages | **未闭环** | API 拒绝 Bot move | 需 R59 产品决策 |
| Web 分享/撤销无传输门禁 | `api-client.ensureApiAvailable`, `shares-core` | **已闭环** | R58 + webPure 测试 | — |
| Web 下载需传输就绪 | `ensureTransportReady` | **已闭环** | files-core 分离 | — |
| Web 元数据 | `webMetaPure`, `page-meta.js`, E2E | **已闭环** | 4/4 playwright | 生产 URL OG 需用户抽检 |
| 桌面 API 未开时 Bot 下载 | `local_api.rs` | **部分** | 架构约束 | P1：文档强调或 in-process 路径 |
| dual-index file_assets/bot_file_map | `telegram_transport`, `file_access` | **部分** | Rust 审计 | P1：删除/分享校验 |
| ≥80% coverage lib+hooks | vitest | **已闭环** | ~97% stmts | — |
| 增量 E2E | `pass-registry.json` | **已闭环** | 45 smoke + 4 meta | 发版前建议全量 |
| 真实 Telegram E2E | — | **不适用 CI** | mock 策略 | 用户密钥验收 |

---

## 3. 最强自我反驳

| 攻击点 | 危险 | R58 处理 |
|--------|------|----------|
| 「分享已闭环」但 recipient 404 | 高 | 文档标明需 `bot_file_map`；单租户 create 未校验 |
| Web 用 `ensureServiceReady` 卡 DB 操作 | 高 | **已修**：`ensureApiAvailable` + 分离 gate |
| 绿点「已就绪」但上传禁用 | 中 | **已修**：`page-readiness` 用 `isWebTransportReady` |
| 乐观 `serviceReady=true` 首 30s | 中 | **已修**：初始 `false`，先 poll |
| 宣称「无 bug」 | — | **禁止**；见 §8 |

---

## 4. 全量问题清单

### P0（阻塞 — 本轮已处理或已文档化）
| 问题 | 处理 |
|------|------|
| Web 分享/ Bot 删除被传输 gate 挡住 | R58 分离 readiness |
| Share 弹窗 `sessionOnline` effect | R57 已改 `shareReady` |
| E2E metadata canonical 重写 | R57 `data-td-path` |

### P1（高 — 待后续）
| 问题 | 影响 | 建议 |
|------|------|------|
| 桌面 Bot 下载依赖 Local API 环回 | API 关则下载失败 | README/DESKTOP-API 强调；或 in-process bot download |
| `file_assets` ≠ `bot_file_map` | 列表有、下载/分享失败 | 上传/删除双表同步；share create 校验 |
| `cmd_move_files` 无 Bot 路径 | 桌面 Bot 移动报错 | 显式错误文案或 DB-only move |
| upload-core SSE 失败不 abort chunks | 浪费带宽 | AbortController（未在本轮实现） |

### P2（中）
- settings 网络面板失败无 inline 错误
- files/shares 页无 tg-dot 状态头
- 生产 OG 社交卡片未验

### P3（增强）
- Bot 逻辑移动（DB folder_id）
- 全量 Playwright + E2E_API=1 发版门禁

---

## 5. R58 实际修复

| 模块 | 变更 |
|------|------|
| `api-client.js` | `ensureApiAvailable`, `ensureTransportReady` |
| `webPure.ts` / `web-pure.js` | `isWebTransportReady`, `isWebDbMutationReady`, `bulkDeleteRequiresTransport` |
| `files-core.js` | transport vs API gate；Bot 删除/分享 decouple |
| `shares-core.js` | 仅 API gate；撤销按钮 disabled 态 |
| `page-readiness.js` | 状态点与上传就绪一致 |
| `dashboard.html` / `upload.html` | script 顺序 web-pure 先于 page-readiness |
| `SharingTab` | `shareReady` 文案 |

---

## 6. 验证结果

| 项 | 命令/结果 |
|----|-----------|
| vitest | **288 passed**（含 webPure readiness +5） |
| coverage lib+hooks | **≥96% stmts / ≥86% branch** |
| cargo `--features headless-server --lib` | **94 passed** |
| Playwright smoke+metadata | **45 passed, 2 skipped, 4 metadata passed** |
| 真实 Telegram | **未跑**（刻意 mock） |

---

## 7. 文档同步

- 本文 `FINAL-AUDIT-R58.md`
- `AUDIT-CLOSURE.md` 第五十八轮（待追加）
- `pass-registry.json` → `web-readiness-r58`
- `README.md` R57/R58 链接

---

## 8. 剩余真实风险与边界

1. **分享 recipient 下载**：需用户真实环境验证 `/d/{token}` + Bot 存储。
2. **桌面 Bot 无 Local API**：下载/预览 loopback 失败 — 配置项非 bug，但易踩坑。
3. **双索引不一致**：历史数据可能导致「看得见下不了」— 需 re-upload 或 rebuild（User）。
4. **不能宣称**：无 bug / 可直接上线 / 100% E2E 真网。

---

## 9. 最终完成度结论

| 维度 | 结论 |
|------|------|
| **已真实闭环** | 桌面 Bot 下载/预览/分享 UI gate；Web 元数据；Web DB 变更 gate；测试矩阵；文档审计链 |
| **部分闭环** | 分享下载数据一致性；桌面 API-off Bot 路径；Bot 移动 |
| **仍受阻** | 真实 Telegram 字节流、生产 OG URL |
| **一次调用标准** | **Headless + Web 管理台 + mock CI** 可达；**完整 Telegram 字节流** 需用户密钥最后一步 |

**反转条件**：用户报告 Bot 分享 404 / 下载失败 → 查 `bot_file_map` 与 Local API 配置。

**下一阶段主要矛盾**：双索引一致性 + 桌面 Bot in-process 下载（若要不依赖 Local API）。
