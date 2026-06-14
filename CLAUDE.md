# Telegram Drive 开发指南

Telegram Drive 是基于 **Tauri v2 + Rust + React** 的跨平台桌面应用，将 Telegram 账号转换为无限云存储。支持桌面端和 Docker Headless API 两种部署模式。

---

## 项目概览

| 层级 | 技术栈 | 位置 |
|------|--------|------|
| **前端** | React 19 + TypeScript + TailwindCSS 4 + Vite 7 | `app/src/` |
| **后端** | Rust + Tauri v2 + Grammers (Telegram Client) | `app/src-tauri/src/` |
| **API 服务** | Actix-web + Redis + SQLite | `app/src-tauri/src/bin/` |
| **部署** | Docker + docker-compose | `deploy/`, `Dockerfile` |

### 核心功能

- 无限云存储（Telegram "Saved Messages" + Channels）
- 分片上传 / 流媒体播放 / PDF 查看
- 分享链接（密码保护 + 过期时间）
- REST API（AI 集成）+ Web 控制台
- SOCKS5 代理 + VPN 优化器
- 自动更新（Tauri Updater）

---

## 开发环境

### 前置依赖

| 依赖 | 版本 | Windows 安装 |
|------|------|--------------|
| Node.js | ≥18 | [nodejs.org](https://nodejs.org/) |
| Rust | stable | `rustup-init.exe` from [rustup.rs](https://rustup.rs/) |
| Visual Studio Build Tools | 最新 | 选择 "Desktop development with C++" 工作负载 |
| WebView2 | 已预装 | Windows 10/11 通常自带 |

### 验证安装

```bash
node --version   # ≥18
rustc --version  # stable
cargo --version
where.exe cargo  # Windows 使用 where.exe 而非 which
```

### Telegram API 凭证

从 [my.telegram.org](https://my.telegram.org) 获取 `api_id` 和 `api_hash`（必需）。

---

## 项目结构

```
Telegram-Drive/
├── app/                        # Tauri 应用主目录
│   ├── src/                    # React 前端
│   │   ├── components/         # UI 组件（Dashboard, AuthWizard 等）
│   │   ├── hooks/              # 自定义 hooks（useFileUpload, useFileDownload 等）
│   │   ├── context/            # React Context（SettingsContext）
│   │   ├── lib/                # 工具函数
│   │   ├── types/              # TypeScript 类型定义
│   │   └── App.tsx             # 入口组件
│   ├── src-tauri/              # Rust 后端
│   │   ├── src/
│   │   │   ├── commands/       # Tauri 命令（auth, fs, sharing 等）
│   │   │   ├── server.rs       # API 服务器核心
│   │   │   ├── api_routes.rs   # REST API 路由
│   │   │   ├── share_routes.rs # 分享功能路由
│   │   │   ├── vpn_optimizer.rs # VPN 网络优化
│   │   │   └── bin/            # Headless Server 入口
│   │   ├── Cargo.toml          # Rust 依赖
│   │   └── tauri.conf.json     # Tauri 配置
│   ├── package.json            # Node 依赖
│   └── tsconfig.json           # TypeScript 配置
├── deploy/                     # Docker 部署配置
│   ├── web/                    # Web 控制台静态页面
│   └── docker-compose.yml
├── docs/                       # 项目文档
│   ├── DESKTOP-API.md          # 桌面 REST API 说明
│   ├── DEPLOYMENT-PRODUCTION.md # 生产部署指南
│   └── ROUND-*.md              # TDD 开发轮次记录
├── tests/                      # 集成测试
├── .env.example                # 环境变量模板
└── CLAUDE.md                   # 本文件
```

---

## 常用命令

### 开发模式

```bash
cd app
npm install                      # 安装依赖
npm run tauri dev                # 启动开发模式（首次编译 5-15 分钟）
```

### 构建

```bash
npm run build                    # 前端构建
npm run tauri build              # 全量构建（生成安装包）
```

### 测试

```bash
npm run test                     # Vitest 单元测试
npm run test:coverage            # 带覆盖率报告

cd app/src-tauri
cargo test                       # Rust 单元测试
cargo clippy -- -D warnings      # Clippy lint（警告视为错误）
cargo fmt -- --check             # 格式检查
```

### Docker 部署

```bash
docker-compose up -d             # 开发环境
docker-compose -f docker-compose.prod.yml up -d  # 生产环境
```

---

## 编码规范

### TypeScript/React

#### 文件组织

- 按功能组织，而非文件类型
- 组件文件 `< 800 行`，函数 `< 50 行`
- 自定义 hooks 使用 `use` 前缀

#### 命名约定

| 类型 | 约定 | 示例 |
|------|------|------|
| 组件 | PascalCase | `FileCard`, `Dashboard` |
| hooks | camelCase + use | `useFileUpload` |
| 函数/变量 | camelCase | `handleUpload`, `fileList` |
| 类型/接口 | PascalCase | `FileItem`, `UploadConfig` |
| 常量 | UPPER_SNAKE_CASE | `MAX_FILE_SIZE` |

#### 状态管理

| 场景 | 工具 |
|------|------|
| 服务端状态 | TanStack Query |
| 全局状态 | React Context |
| 表单状态 | 受控组件 |

#### 不可变性

```typescript
// 正确：返回新对象
const updated = { ...original, field: value };

// 错误：就地修改
original.field = value;
```

### Rust

#### 格式化

```bash
cargo fmt                        # 自动格式化
cargo clippy -- -D warnings      # lint 检查
```

#### 错误处理

- 使用 `Result<T, E>` + `?` 传播错误
- 库代码使用 `thiserror` 定义类型错误
- 应用代码使用 `anyhow` 添加上下文

```rust
// 正确：带上下文的错误传播
use anyhow::Context;

fn load_config(path: &str) -> anyhow::Result<Config> {
    std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {path}"))?
        .parse()
        .with_context(|| format!("failed to parse {path}"))?
}
```

#### 所有权

- 默认借用 (`&T`)，仅在需要存储或消耗时获取所有权
- 函数参数优先 `&str` 而非 `String`，`&[T]` 而非 `Vec<T>`
- 构造函数使用 `impl Into<String>` 接收参数

#### 命名约定

| 类型 | 约定 | 示例 |
|------|------|------|
| 函数/变量 | snake_case | `get_file_list`, `peer_cache` |
| 类型/枚举 | PascalCase | `TelegramState`, `ConnectionState` |
| 常量 | SCREAMING_SNAKE_CASE | `MAX_RETRY_COUNT` |
| 模块 | snake_case | `commands`, `api_routes` |

---

## 安全要求

### 提交前检查清单

- [ ] 无硬编码密钥（API keys, tokens）
- [ ] 用户输入已验证
- [ ] SQL 使用参数化查询（`sqlite` crate 的 bind 参数）
- [ ] 文件路径已净化（防止路径遍历）
- [ ] 错误消息不泄露内部信息

### 密钥管理

- 所有敏感配置通过 `.env` 或环境变量
- `.env` 已在 `.gitignore` 中
- 启动时验证必需密钥存在

```rust
// 正确：从环境变量加载
std::env::var("API_KEY")
    .context("API_KEY must be set")?
```

---

## 测试要求

### 最低覆盖率：80%

| 层级 | 工具 | 位置 |
|------|------|------|
| React 单元测试 | Vitest + Testing Library | `app/src/**/*.test.tsx` |
| Rust 单元测试 | `#[test]` + `#[cfg(test)]` | `app/src-tauri/src/**/*.rs` |
| 集成测试 | cargo test --test | `tests/` |

### 测试结构（AAA 模式）

```typescript
test('calculates file size correctly', () => {
  // Arrange
  const file = { size: 1024 };

  // Act
  const result = formatFileSize(file.size);

  // Assert
  expect(result).toBe('1 KB');
});
```

### Rust 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_runner_shutdown_sends_once() {
        let shutdown = Arc::new(std::sync::Mutex::new(None));
        let (tx, mut rx) = oneshot::channel();
        *shutdown.lock().unwrap() = Some(tx);
        assert!(signal_runner_shutdown(&shutdown));
        rx.try_recv().expect("shutdown signal");
    }
}
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

### Tauri 命令模式

Rust 后端通过 `#[tauri::command]` 暴露给前端：

```rust
#[tauri::command]
async fn cmd_upload_file(
    path: String,
    folder_id: i64,
    state: State<'_, TelegramState>,
) -> Result<UploadResult, String> {
    // 实现...
}
```

前端调用：

```typescript
import { invoke } from '@tauri-apps/api';

const result = await invoke('cmd_upload_file', { 
  path, 
  folderId 
});
```

### Telegram 连接状态

`TelegramState` 是核心状态结构：

```rust
pub struct TelegramState {
    pub client: Arc<Mutex<Option<Client>>>,
    pub login_token: Arc<Mutex<Option<LoginToken>>>,
    pub password_token: Arc<Mutex<Option<PasswordToken>>>,
    pub runner_shutdown: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
    pub peer_cache: Arc<RwLock<HashMap<i64, Peer>>>,
    pub cancelled_transfers: Arc<RwLock<HashSet<String>>>,
}
```

**关键注意**：重新连接前必须 shutdown 旧 runner，否则线程栈耗尽。

### REST API 端点

| 路径 | 功能 |
|------|------|
| `/api/v1/files` | 文件列表/上传/下载 |
| `/api/v1/shares` | 分享管理 |
| `/api/v1/settings` | 配置管理 |
| `/api/v1/auth/status` | 连接状态 |

API 认证：
- 桌面端：`X-Access-Pwd`（本地密码）
- 外部集成：`X-API-Key`（Argon2 hash 校验）

---

## 常见问题

### 构建失败：`linker 'link.exe' not found`

安装 Visual Studio Build Tools，选择 "Desktop development with C++"。

### 首次编译时间长

首次需编译 300+ Rust crates，耗时 5-15 分钟。后续构建将快很多。

### NPM 漏洞警告

通常与构建工具和 dev dependencies 相关，可选运行 `npm audit fix`。

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
| [docs/DESKTOP-API.md](docs/DESKTOP-API.md) | 桌面 REST API 详情 |
| [docs/DEPLOYMENT-PRODUCTION.md](docs/DEPLOYMENT-PRODUCTION.md) | 生产部署指南 |
| [docs/ROUND-39-TDD.md](docs/ROUND-39-TDD.md) | 最新 TDD 方案 |

---

*最后更新: 2026-06-12*
*版本: 4.0.0-beta*