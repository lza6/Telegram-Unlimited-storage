# 产品扩展方向（可选采纳）

> 与 R51 审计并列；**非本轮回交付范围**，供产品/路线图参考。

## 1. 后端 Bulk API 增强（部分已落地 R52）

**已落地**：`succeeded_ids`（Bot 部分删除、User 全批）。

**待选**：`skipped_ids: [{ id, reason }]` 供审计与 UI 提示。

## 2. 桌面 Bot 模式 parity（R52–R55 已落地轻量方案）

**已落地**：`serviceReady` / `isBotIndexReady`；列表、删除、**全局搜索**、**下载**、**预览/缩略图/流媒体**、**分享链接**走 asset index + 本地 REST / DB share。

**待选**：统一 `TransportFacade`；`skipped_ids` 审计字段；bulk move Bot。

## 3. 上传/下载 UX  polish

| 点 | 建议 |
|----|------|
| 上传 WS 断线 | R51 已加 poll fallback；可再加队列行「进度来源：轮询」标签 |
| 大文件合并 | 合并阶段独立 progress（当前仅 chunk） |
| 下载队列 | 失败项一键重试 + 汇总 toast |

## 4. 小白/onboarding

- 首次 Bot 模式：引导「索引 vs Telegram 消息」差异（已有部分 onboarding，可统一到 files 页 banner）
- 传输模式切换后：自动弹出「重建索引」CTA（settings 已有，可联动 files 页）

## 5. 性能与规模

- 文件列表虚拟滚动（桌面已有 virtual；Web files 表可分页 + 虚拟）
- 全局搜索：debounce + 索引 rebuild 进度条（非 silent 时）

## 6. 安全与 SaaS

- 多租户 bulk：审计日志（谁删除了哪些 ID）
- 分享链接：默认过期时间 + 密码策略提示

---

采纳优先级建议：**1 → 2 → 3**；其余按用户反馈迭代。
