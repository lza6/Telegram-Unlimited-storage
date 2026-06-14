# R55 缺口修复（深度审查发现）

## 主要矛盾

R55 后端/UI 门禁已通，但 **Dashboard 副作用仍按 `sessionOnline` 清预览** → Bot 用户一点开就被 effect 关掉（「能下能点不能看」假闭环）。

## 次要矛盾

- 键盘 Enter 仍绑 `transferEnabled=sessionOnline`
- Delete 快捷键未用 `deleteReady`（Bot 可删索引）

## TDD

1. RED `useKeyboardShortcuts` — Enter 用 `previewEnabled`，Delete 用 `deleteEnabled`
2. GREEN Dashboard effect — `!previewReady` 时关预览，Share 仍 `!sessionOnline`
3. VERIFY vitest 全绿

## 反转条件

- `previewReady` 变 false（API 掉线）→ 预览应关闭
- User 上线 → 行为不变
