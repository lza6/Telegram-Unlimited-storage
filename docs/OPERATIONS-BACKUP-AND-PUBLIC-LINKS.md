# 公开直链与 Telegram 备份运维

## 公开永久直链：采用的语义

可以做“永久直链”，但它必须是本站域名的稳定资源地址，而不是把 Telegram `getFile` 临时 URL 直接返给用户。Telegram CDN/Bot API URL、Bot token、文件路径和访问授权都不能构成一个可承诺永久有效的公开地址。

目标路由将是：

```text
https://<本站域名>/dl/<public_asset_id>/<filename>
```

- 链接在资产未删除、未被管理员下线且本站域名仍运行时保持不变；支持 `Range`，因此浏览器、下载器、视频播放器可直接请求和断点续传。
- 按产品要求，公开资源模式**不做 Referer 防盗链、登录验证、一次性 token、过期时间或下载密码**。拿到链接的人都可访问；这是产品选择，不能与“私密文件”同时成立。
- 后端仍必须保留全局/机器人/频道的调度、HTTP 限速和健康保护；它们保护服务可用性，不是对用户的防盗链限制。
- 现有分享链接仍保留为私有受控模式。W2/W6 将单独增加显式 `public_permanent` 资产状态、删除/下线审计、稳定路由、范围请求和流量计量；在此之前不能把当前分享 token 伪称为永久直链。

## 已落地：加密数据库备份到 Telegram 私有频道

已创建 Windows 无 Docker 的备份/验证工具：

```bat
scripts\native\backup-postgres-to-telegram.bat
scripts\native\backup-postgres-to-telegram-dry-run.bat
python scripts\native\restore_telegram_backup.py --input "<下载的.tdbak或分片目录>" --output "<不存在的输出目录>"
```

备份工具会：

1. 用 `pg_dump --format=custom --compress=9` 导出 PostgreSQL 控制面；
2. 通过 SQLite online backup API 取得当前 SQLite 运行数据库的一致快照；
3. 写入带 SHA-256 清单的 ZIP；
4. 使用 `.env` 中 `BACKUP_ENCRYPTION_KEY` 进行 AES-256-GCM 加密；
5. 以保守的 45 MiB 分片上传到已配置的私有 Telegram 存储频道，并发送完成回执；
6. 明文快照和未加密 ZIP 无论成功、失败或 `--keep-local` 均清除。`--keep-local` 仅保留加密 `.tdbak` 材料用于恢复演练；每个备份的 Telegram message ID、分片哈希、完成状态或回滚失败会写入被 Git 忽略的 `data/backup-state/`。

`BACKUP_ENCRYPTION_KEY` 已在本机 `.env` 生成且被 Git 忽略。下载备份后必须先使用恢复工具校验 SHA-256；恢复工具只解密/验证/导出文件，不会自动覆盖数据库。

本轮真实证据：2026-07-19 已成功创建并上传一个加密备份到已配置存储频道；随后从本地演练副本完成解密和完整性校验。未输出 token、频道 ID、加密密钥或备份内容。

## 不能混淆的事实

- 备份放到 Telegram 不会让 PostgreSQL/SQLite 本身变小。数据库增长要靠 W3 的账本分区、任务/回调/审计保留期、聚合归档、VACUUM/ANALYZE、临时文件 TTL 与配额治理。
- Telegram 不是“无限且无条件”的数据库备份服务：公开 Bot API 单文件上传边界为 50 MB；脚本默认保持在 45 MiB 分片。Local Bot API 的更大边界也不等于无限。
- `.tdbak` 不可丢失 `BACKUP_ENCRYPTION_KEY`；密钥丢失意味着备份不可恢复。应将密钥存到独立密码库/离线恢复材料，不能仅存于这台机器。