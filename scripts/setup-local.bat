@echo off
chcp 65001 >nul 2>&1
setlocal EnableDelayedExpansion

set "ROOT=%~dp0.."
for %%I in ("%ROOT%") do set "ROOT=%%~fI"
set "TD_ROOT=%ROOT%"
cd /d "%ROOT%"

call "%ROOT%\scripts\_common.bat"

call "%ROOT%\scripts\_log.bat" "========== 本地环境初始化 =========="
call "%ROOT%\scripts\_log.bat" "ROOT=%ROOT%"

if exist "%USERPROFILE%\.cargo\bin" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

set "CARGO_OK=0"
cargo --version >nul 2>&1
if not errorlevel 1 set "CARGO_OK=1"

if "!CARGO_OK!"=="0" (
    call "%ROOT%\scripts\_log.bat" "[错误] 未检测到可用的 Cargo/Rust"
    echo.
    echo 请安装 https://rustup.rs/ 或 winget install Rustlang.Rustup
    echo 安装后重新打开 CMD 再运行 setup.bat
    endlocal & exit /b 1
)

for /f "delims=" %%v in ('cargo --version 2^>nul') do call "%ROOT%\scripts\_log.bat" "[OK] %%v"
for /f "delims=" %%v in ('rustc --version 2^>nul') do call "%ROOT%\scripts\_log.bat" "[OK] %%v"

if not exist "%ROOT%\data" (
    mkdir "%ROOT%\data"
    call "%ROOT%\scripts\_log.bat" "[OK] 已创建 data\"
) else (
    call "%ROOT%\scripts\_log.bat" "[OK] data\ 已存在"
)

if not exist "%ROOT%\.env" (
    if exist "%ROOT%\.env.example" (
        copy /Y "%ROOT%\.env.example" "%ROOT%\.env" >nul
        call "%ROOT%\scripts\_log.bat" "[OK] 已从 .env.example 生成 .env"
        call "%ROOT%\scripts\_log.bat" "[!!] 请编辑 .env 填写 TELEGRAM_API_ID / HASH / ACCESS_PWD / API_KEY"
    ) else (
        call "%ROOT%\scripts\_log.bat" "[警告] 缺少 .env.example"
    )
) else (
    call "%ROOT%\scripts\_log.bat" "[OK] .env 已存在"
)

call "%ROOT%\scripts\_log.bat" "[..] cargo fetch..."
pushd "%ROOT%\app\src-tauri"
cargo fetch
if errorlevel 1 (
    call "%ROOT%\scripts\_log.bat" "[错误] cargo fetch 失败"
    popd
    endlocal & exit /b 1
)
popd
call "%ROOT%\scripts\_log.bat" "[OK] Cargo 依赖已就绪"

echo.
set "LAUNCH_SCRIPT=start.bat"
call "%ROOT%\scripts\_log.bat" "初始化完成 - 下一步双击 !LAUNCH_SCRIPT!"
call "%ROOT%\scripts\_log.bat" "管理台 http://localhost:1334/  本地 start.bat  服务器 dev.bat"
echo.

endlocal & exit /b 0
