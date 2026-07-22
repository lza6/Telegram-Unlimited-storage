@echo off
setlocal EnableExtensions DisableDelayedExpansion
chcp 65001 >nul
title Telegram Drive Windows 启动器

set "ROOT=%~dp0"
pushd "%ROOT%" >nul 2>&1 || (
  echo [错误] 无法进入项目目录：%ROOT%
  exit /b 1
)

set "ACTION=%~1"
set "RC=0"
if /i "%ACTION%"=="" goto menu
if /i "%ACTION%"=="desktop" goto desktop
if /i "%ACTION%"=="server" goto server
if /i "%ACTION%"=="docker" goto docker
if /i "%ACTION%"=="install" goto install
if /i "%ACTION%"=="help" goto help

echo [错误] 未知参数：%ACTION%
set "RC=1"
goto help

:menu
echo.
echo ========================================
echo          Telegram Drive 启动器
echo ========================================
echo [1] 桌面开发模式（Tauri）
echo [2] Headless API 服务（本机）
echo [3] Docker Compose 服务
echo [4] 只安装前端依赖
echo [0] 退出
echo.
set /p "CHOICE=请输入选项："
if "%CHOICE%"=="1" goto desktop
if "%CHOICE%"=="2" goto server
if "%CHOICE%"=="3" goto docker
if "%CHOICE%"=="4" goto install
if "%CHOICE%"=="0" goto end
echo [错误] 请输入 0 到 4。
goto menu

:load_env
if not exist ".env" (
  echo [提示] 未发现 .env；可从 .env.example 复制后填写 Telegram 与访问凭据。
  exit /b 0
)
for /f "usebackq eol=# tokens=1,* delims==" %%A in (".env") do (
  if not "%%A"=="" set "%%A=%%B"
)
exit /b 0

:require_node
where node >nul 2>&1 || (
  echo [错误] 未找到 Node.js。请安装 Node.js 18 或更高版本后重试。
  exit /b 1
)
where npm.cmd >nul 2>&1 || (
  echo [错误] 未找到 npm.cmd。请修复 Node.js 安装或 PATH。
  exit /b 1
)
exit /b 0

:require_cargo
where cargo >nul 2>&1 || (
  echo [错误] 未找到 Rust cargo。请安装 Rust stable 并重新打开 CMD。
  exit /b 1
)
exit /b 0

:install
call :require_node || goto failed
if not exist "app\package-lock.json" (
  echo [错误] 缺少 app\package-lock.json，拒绝使用 npm install 破坏锁文件一致性。
  goto failed
)
echo [信息] 正在通过 npm ci 安装前端依赖……
pushd "app" || goto failed
call npm.cmd ci
set "RC=%ERRORLEVEL%"
popd
if not "%RC%"=="0" goto failed
echo [完成] 前端依赖安装完成。
goto end

:desktop
call :require_node || goto failed
if not exist "app\node_modules" call :install || goto failed
echo [信息] 启动 Tauri 桌面开发模式。首次 Rust 编译可能需要数分钟。
pushd "app" || goto failed
call npm.cmd run tauri dev
set "RC=%ERRORLEVEL%"
popd
if not "%RC%"=="0" goto failed
goto end

:server
call :load_env
if not defined ACCESS_PWD (
  echo [错误] Headless 服务需要 ACCESS_PWD；请在 .env 中配置。
  goto failed
)
if not defined API_KEY (
  echo [错误] Headless 服务需要 API_KEY；请在 .env 中配置，不会自动生成或打印明文密钥。
  goto failed
)
call :require_cargo || goto failed
if not defined DATA_DIR set "DATA_DIR=%ROOT%data"
if not defined STATIC_DIR set "STATIC_DIR=%ROOT%deploy\web"
if not defined DOCS_DIR set "DOCS_DIR=%ROOT%docs"
set "BIND_HOST=127.0.0.1"
if not defined PORT set "PORT=1334"
echo [信息] 启动 Headless API：http://%BIND_HOST%:%PORT%
echo [信息] 凭据不会显示在窗口中。按 Ctrl+C 可安全停止服务。
pushd "app\src-tauri" || goto failed
cargo run --bin telegram-drive-server --features headless-server
set "RC=%ERRORLEVEL%"
popd
if not "%RC%"=="0" goto failed
goto end

:docker
where docker >nul 2>&1 || (
  echo [错误] 未找到 Docker CLI。请安装并启动 Docker Desktop。
  goto failed
)
call :load_env
if not defined API_KEY (
  echo [错误] Docker Headless 服务需要 API_KEY；请在 .env 中配置。
  goto failed
)
if not defined ACCESS_PWD (
  echo [错误] Docker Headless 服务需要 ACCESS_PWD；请在 .env 中配置。
  goto failed
)
echo [信息] 启动基础 Docker Compose。服务仅发布到本机 127.0.0.1，公网访问请配置 TLS 反向代理。
echo [信息] 此模式忽略 .env 中的 COMPOSE_FILE，避免误启用开发挂载或公网端口。
docker compose -f docker-compose.yml up --build
if errorlevel 1 goto failed
goto end

:help
echo.
echo 用法：
echo   start.bat desktop   启动 Tauri 桌面开发模式
echo   start.bat server    启动本机 Headless API 服务
echo   start.bat docker    启动 Docker Compose 服务
echo   start.bat install   仅执行 npm ci
echo   start.bat           显示交互菜单
goto end

:failed
echo.
echo [失败] 启动未完成，请根据上方错误修复环境或配置。
set "RC=1"

:end
popd >nul 2>&1
endlocal & exit /b %RC%
