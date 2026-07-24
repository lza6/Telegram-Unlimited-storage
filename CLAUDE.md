# Telegram Drive 开发指南

Telegram Drive 是基于 **Python FastAPI + Telethon + 静态 Web UI** 的跨平台云存储服务，将 Telegram 账号转换为无限云存储。支持 Docker 部署和本地 Python 进程两种模式。

---

## 项目概览

| 层级 | 技术栈 | 位置 |
|------|--------|------|
| **前端** | 静态 HTML/CSS/JS | `deploy/web/` |
| **后端** | Python 3.11 + FastAPI + uvicorn + Telethon | `backend/app/` |
| **数据库** | SQLite（默认）/ PostgreSQL（控制面模式） | `backend/app/storage.py` |
| **部署** | Docker + docker-compose | `deploy/`, `Dockerfile` |

### 核心功能

- 无限云存储（Telegram "Saved Messages" + Channels）
- 分片上传 / 流媒体播放 / PDF 查看
- 分享链接（密码保护 + 过期时间）
- REST API（AI 集成）+ Web 控制台
- SOCKS5 代理支持
- 多租户支持

---

## 开发环境

### 前置依赖

| 依赖 | 版本 | 安装 |
|------|------|------|
| Python | ≥3.11 | [python.org](https://www.python.org/downloads/) |
| Docker（可选） | 最新 | [docker.com](https://www.docker.com/) |

### 验证安装

```bash
python --version   # ≥3.11
pip --version
```

### Telegram API 凭证

从 [my.telegram.org](https://my.telegram.org) 获取 `api_id` 和 `api_hash`（必需）。

---

## 项目结构

```
Telegram-Drive/
├── backend/                    # Python 后端
│   ├── app/
│   │   ├── main.py             # FastAPI 应用入口 + 中间件
│   │   ├── config.py           # 配置（pydantic-settings）
│   │   ├── auth.py             # 认证逻辑
│   │   ├── security.py         # 安全工具（Argon2, HMAC, 令牌）
│   │   ├── storage.py          # SQLite/PostgreSQL 存储层
│   │   ├── telegram_state.py   # Telegram 连接状态
│   │   ├── transfers.py        # 传输管理（上传/下载）
│   │   ├── downloads.py        # 下载逻辑
│   │   ├── links.py            # 分享链接逻辑
│   │   ├── bot_transport.py    # Bot 传输模式
│   │   ├── settings_store.py   # 设置存储
│   │   ├── state.py            # 应用状态
│   │   └── routers/            # API 路由
│   │       ├── auth.py         # 认证路由
│   │       ├── files.py        # 文件操作路由
│   │       ├── health.py       # 健康检查路由
│   │       ├── legacy.py       # 兼容旧接口路由
│   │       ├── settings.py     # 设置路由
│   │       └── shares.py       # 分享路由
│   ├── tests/                  # pytest 测试
│   ├── requirements.txt        # 生产依赖
│   ├── requirements-dev.txt    # 开发依赖（含 pytest）
│   └── pytest.ini              # pytest 配置
├── deploy/                     # Docker 部署配置
│   ├── web/                    # Web 控制台静态页面
│   └── docker-compose.yml
├── docs/                       # 项目文档
│   ├── DESKTOP-API.md          # REST API 说明
│   ├── DEPLOYMENT-PRODUCTION.md # 生产部署指南
│   └── ...
├── tests/                      # 集成测试
├── .env.example                # 环境变量模板
└── CLAUDE.md                   # 本文件
```

---

## 常用命令

### 本地开发

```bash
cd backend
python -m venv .venv
.venv\Scripts\activate        # Windows
# source .venv/bin/activate   # macOS/Linux
pip install -r requirements-dev.txt

# 从仓库根启动（.env 自动加载）
cd ..
.venv\Scripts\python -m uvicorn app.main:app --app-dir backend --host 127.0.0.1 --port 1334 --reload
```

### 测试

```bash
cd backend
.venv\Scripts\python -m pytest           # 运行测试
.venv\Scripts\python -m pytest --cov=app # 带覆盖率
```

### Docker 部署

```bash
docker-compose up -d             # 开发环境
docker-compose -f docker-compose.prod.yml up -d  # 生产环境
```

---

## 编码规范

### Python

#### 文件组织

- 按功能组织，而非文件类型
- 组件文件 `< 800 行`，函数 `< 50 行`
- 使用类型注解

#### 命名约定

| 类型 | 约定 | 示例 |
|------|------|------|
| 函数/变量 | snake_case | `get_file_list`, `peer_cache` |
| 类/类型 | PascalCase | `TelegramState`, `ConnectionState` |
| 常量 | SCREAMING_SNAKE_CASE | `MAX_RETRY_COUNT` |
| 模块 | snake_case | `commands`, `api_routes` |

#### 错误处理

- 使用 `Result<T, E>` 模式（Python: 异常 + try/except）
- 库代码使用具体异常类型
- 应用代码添加上下文信息

```python
# 正确：带上下文的错误传播
try:
    config = load_config(path)
except FileNotFoundError:
    raise ConfigError(f"Config file not found: {path}")
```

---

## 安全要求

### 提交前检查清单

- [ ] 无硬编码密钥（API keys, tokens）
- [ ] 用户输入已验证
- [ ] SQL 使用参数化查询
- [ ] 文件路径已净化（防止路径遍历）
- [ ] 错误消息不泄露内部信息

### 密钥管理

- 所有敏感配置通过 `.env` 或环境变量
- `.env` 已在 `.gitignore` 中
- 启动时验证必需密钥存在

```python
# 正确：从环境变量加载
api_key = os.environ.get("API_KEY")
if not api_key:
    raise ValueError("API_KEY must be set")
```

---

## 测试要求

### 最低覆盖率：80%

| 层级 | 工具 | 位置 |
|------|------|------|
| Python 单元测试 | pytest | `backend/tests/` |
| 集成测试 | PowerShell/curl | `tests/integration/` |

### 测试结构（AAA 模式）

```python
def test_calculate_file_size():
    # Arrange
    file_size = 1024

    # Act
    result = format_file_size(file_size)

    # Assert
    assert result == "1 KB"
```

---

## Git 工作流

### 提交消息格式

```
<类型>: <描述>

<可选正文>
```

类型：`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

### Pull Request 流程

1. 分析完整提交历史（不仅是最新提交）
2. 使用 `git diff main...HEAD` 查看所有变更
3. 确保 CI 通过、无合并冲突
4. 包含测试计划（TODO 列表）

---

## 架构要点

### FastAPI 应用结构

`main.py` 是核心入口：

```python
app = create_app()

@app.on_event("startup")
async def startup():
    # 初始化 TelegramState, Storage, Authenticator 等
    ...
```

### Telegram 连接状态

`TelegramState` 是核心状态结构：

```python
class TelegramState:
    client: Optional[TelegramClient]
    login_token: Optional[str]
    password_token: Optional[str]
    peer_cache: Dict[int, Peer]
    cancelled_transfers: Set[str]
```

**关键注意**：重新连接前必须断开旧连接。

### REST API 端点

| 路径 | 功能 |
|------|------|
| `/api/v1/files` | 文件列表/上传/下载 |
| `/api/v1/shares` | 分享管理 |
| `/api/v1/settings` | 配置管理 |
| `/api/v1/auth/status` | 连接状态 |

API 认证：
- Web 控制台：`X-Access-Pwd`（本地密码）
- 外部集成：`X-API-Key`（Argon2 hash 校验）

---

## 常见问题

### 首次运行

首次需安装 Python 依赖，耗时 1-2 分钟。后续启动将快很多。

### .env 加载

`.env` 文件从仓库根目录加载（`config.py` 使用绝对路径解析）。无论从哪个目录启动 uvicorn，都能正确加载。

---

## Windows 环境特殊注意

- 禁止使用 `.sh` 脚本，改用 `node` 或 PowerShell
- 路径可使用 `\` 或 `/`（两者均可）
- 命令链接：`cmd1; if($?) { cmd2 }` 而非 `cmd1 && cmd2`
- 查找可执行文件：`where.exe` 而非 `which`
- 使用内置 ripgrep (`rg`) 进行搜索

---

## 参考文档

| 文档 | 内容 |
|------|------|
| [README.md](README.md) | 项目简介与安装指南 |
| [README-DOCKER.md](README-DOCKER.md) | Docker 部署快速开始 |
| [docs/DESKTOP-API.md](docs/DESKTOP-API.md) | REST API 详情 |
| [docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md) | 生产部署指南 |

---

*最后更新: 2026-07-24*
*版本: 5.0.0-python*
