# R57 计划 — Web 元数据 + Share 副作用修复 + Mock 登记

## 主要矛盾（决定性）

1. **R56 遗漏**：`useEffect(!sessionOnline)` 仍清 `shareFile` → Bot 分享弹窗闪关（与 R55 preview 同型 bug）
2. **Web 管理台缺 SEO/OG 元数据** — 社交卡片、canonical、robots 不一致（外部：爬虫/分享预览；内部：可补）

## 牵引性

- 批量移动 **不能** Bot 化 — `cmd_move_files` 硬依赖 GramJS `forward_messages`（**外部硬条件**，R58+ 再议）
- 增量 E2E：用户要求 mock 文件夹登记已测模块

## TDD

1. RED `webMetaPure.test.ts` — robots / OG bundle 一致性
2. RED `web-metadata.spec.ts` — login/docs meta + page-meta.js 绝对 URL
3. GREEN HTML head + `page-meta.js` + Dashboard shareReady effect
4. VERIFY vitest + playwright metadata + share 增量

## 反转条件

- 生产域名部署后需用真实 URL 验 OG（非 localhost）— 由 `page-meta.js` 在运行时填绝对路径

## 下一阶段

- R58：Bot 索引内「逻辑移动」（仅 DB folder_id remap，不 forward）— 需产品确认
