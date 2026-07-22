# 环境变量与首次启动指南

本项目可使用两种 Telegram 传输：Bot 模式适合最快跑通；User 模式适合更高限制和现有个人 Telegram 账号。只需二选一，不要同时猜测。

## 检查本机配置状态

根目录 `.env` 是唯一运行时配置来源，且不应提交、截图或复制到文档。可执行 `start.bat server`：脚本会检查 Headless 所需的 `ACCESS_PWD` 与 `API_KEY`，且不会显示它们的实际值。

选择 Bot 模式时，必须配置 `TG_BOT_TOKEN` 与 `TG_STORAGE_CHANNEL_ID`；选择 User 模式时，必须配置 `TELEGRAM_API_ID` 与 `TELEGRAM_API_HASH` 并在 Web 登录页完成手机验证。两条路线二选一。

## 方案 A：Bot 模式，建议首次跑通使用

你需要提供：

1. `TG_BOT_TOKEN`：在 Telegram 的 `@BotFather` 创建机器人后获得。不是 Bot ID；不要把 token 发到聊天、截图或 Git。
2. `TG_STORAGE_CHANNEL_ID`：专用私有频道的数字 ID，格式通常为 `-100xxxxxxxxxx`。
3. 权限：把机器人加入该私有频道并授予 **管理员** 权限，至少允许发布消息和文件。
4. `ACCESS_PWD`：Web 管理台密码，自行生成长随机值。
5. `API_KEY`：给自动化、AI 工具和 REST 调用使用的独立长随机值。
6. `DOWNLOAD_SIGNING_SECRET`：至少 32 个随机字符，用于预签名下载链接。

Bot 模式最小 `.env` 示例：

```dotenv
TELEGRAM_TRANSPORT_MODE=bot
TG_BOT_TOKEN=从_BotFather_取得的_token
TG_STORAGE_CHANNEL_ID=-1001234567890
ACCESS_PWD=自行生成的长随机管理密码
API_KEY=自行生成的长随机API密钥
DOWNLOAD_SIGNING_SECRET=至少32字符的随机密钥
BIND_HOST=127.0.0.1
PORT=1334
DATA_DIR=./data
STATIC_DIR=./deploy/web
DOCS_DIR=./docs
```

## 方案 B：User 模式，使用 Telegram 用户账号

你需要提供：

1. `TELEGRAM_API_ID` 与 `TELEGRAM_API_HASH`：登录 `my.telegram.org`，在 API development tools 创建应用后取得。它们不是 BotFather Token。
2. `ACCESS_PWD`、`API_KEY` 和 `DOWNLOAD_SIGNING_SECRET`，要求同 Bot 模式。
3. 第一次登录时可访问 Telegram 的手机号、短信或 App 验证码，以及如启用时的两步验证密码。

User 模式最小 `.env` 示例：

```dotenv
TELEGRAM_TRANSPORT_MODE=user
TELEGRAM_API_ID=123456
TELEGRAM_API_HASH=你的API_HASH
ACCESS_PWD=自行生成的长随机管理密码
API_KEY=自行生成的长随机API密钥
DOWNLOAD_SIGNING_SECRET=至少32字符的随机密钥
BIND_HOST=127.0.0.1
PORT=1334
DATA_DIR=./data
STATIC_DIR=./deploy/web
DOCS_DIR=./docs
```

启动 `start.bat server` 后，打开 `http://127.0.0.1:1334/telegram.html`，用 `ACCESS_PWD` 登录管理台，再完成 Telegram 用户登录。成功后 session 会存入 `data/`；它是敏感凭据，不能提交或共享。

## 不需要提供的东西

- 不需要 Telegram Bot 的数字 ID；真正需要的是 `TG_BOT_TOKEN`。
- 不需要频道公开链接；真正需要的是私有频道数字 ID 和机器人管理员权限。
- 不需要把任意真实 token、密码、API Hash 发给我。请只写入本机 `.env`。

## 安全启动规则

- `.env` 被 Git 忽略，仍不可上传网盘或截图。
- `start.bat server` 固定为 `127.0.0.1`；`start.bat docker` 固定使用基础 `docker-compose.yml`，不继承 `.env` 的 `COMPOSE_FILE`。
- 需要公网访问时，用 HTTPS 反向代理把流量转到本机 loopback 端口；不要直接把 API 暴露为明文 HTTP。
- 多租户默认开启。若真的要给多个系统使用，后续还需要按 `data/tenants.json.example` 创建各租户独立 API Key。
