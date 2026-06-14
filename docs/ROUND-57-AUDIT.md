# R57 审计 — Web 元数据 + Share 副作用 + Mock 登记

## 主要矛盾（已解）

| 矛盾 | 类型 | 处理 |
|------|------|------|
| `useEffect(!sessionOnline)` 误关分享弹窗 | 内部结构 | 改为 `!shareReady`（与 R55 preview 同型） |
| 管理台 HTML 缺 SEO/OG/canonical | 内部可改 | `webMetaPure.ts` + 各页 head + `page-meta.js` |
| E2E 对 `files.html` 用浏览器 goto 被鉴权重定向 | 测试策略 | 静态 meta 改 `request.get` |
| canonical 用 `location.pathname` 遇 `/login` 重写 | 外因→内因 | `data-td-path` 显式 canonical（对齐 `webMetaPure`） |

## 次要矛盾（盯住未主攻）

- **Bulk move Bot 化**：`cmd_move_files` 依赖 GramJS `forward_messages` — 外部硬条件，R58 需产品决策（仅 DB folder_id remap）。
- **生产 OG 卡片**：localhost E2E 已验绝对 URL 逻辑；真实域名需用户部署后用公开 URL 抽检（反转条件）。
- **SharingTab 文案**：曾误称「须 User 才能创建分享」→ 已改 `shareReady` gate。

## 交付清单

| 区域 | 文件/行为 |
|------|-----------|
| 纯函数契约 | `app/src/lib/webMetaPure.ts` + 4 tests |
| 运行时绝对 URL | `deploy/web/assets/page-meta.js` + `data-td-path` |
| HTML head | `deploy/web/*.html`（robots、OG、twitter、canonical 占位） |
| Dashboard | `shareReady` effect deps；SettingsModal 传 `shareReady` |
| SharingTab | 警告绑定 `shareReady` 非 `sessionOnline` |
| 增量 E2E | `tests/mocks/pass-registry.json`、`docs/E2E-CHECKPOINTS.md` |
| E2E | `tests/e2e/web-metadata.spec.ts` **4 passed** |

## 验证（2026-06-12）

```
npm test              → 241 passed
npm run test:coverage → 96.88% stmts / 86.92% branch (lib+hooks)
playwright web-metadata.spec.ts → 4 passed
```

## 自我反驳（最强）

1. **仍可能错**：Rust 生产静态路由若与 `serve` 重写规则不一致，需保证 `data-td-path` 与 OpenAPI 文档路径一致 — 已用 `webMetaPure` 单源。
2. **未 mock 真实 Telegram 分享下载**：recipient `/d/{token}` 流式下载需用户密钥实测 — CI 故意不调真实 API。
3. **index.html** 未加完整 OG（仅入口重定向）— 可接受，`noindex` 即可。

## 下一阶段信号

- 若启用「Bot 逻辑移动」→ 主要矛盾转移至 `fs.rs` + TopBar `bulkMoveAllowed`。
- 若新增 SPA 路由页 → 须同步 `webMetaPure` + HTML head + pass-registry 条目。
