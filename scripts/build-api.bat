@echo off

chcp 65001 >nul 2>&1

setlocal EnableDelayedExpansion



set "ROOT=%~dp0.."

for %%I in ("%ROOT%") do set "ROOT=%%~fI"

set "TD_ROOT=%ROOT%"

cd /d "%ROOT%"



call "%ROOT%\scripts\_common.bat"

if exist "%ROOT%\.env" call "%ROOT%\scripts\_load-env.bat" "%ROOT%\.env"



set "DATA_DIR=%ROOT%\data"

set "STATIC_DIR=%ROOT%\deploy\web"

set "EXE=%ROOT%\app\src-tauri\target\release\telegram-drive-server.exe"



cargo --version >nul 2>&1

if errorlevel 1 (

    call "%ROOT%\scripts\_log.bat" "[错误] Cargo 不可用，请先运行 setup.bat"

    exit /b 1

)



call "%ROOT%\scripts\_log.bat" "========== Release 编译（详细日志）=========="



pushd "%ROOT%\app\src-tauri"

cargo build --release --bin telegram-drive-server --features headless-server -vv

set "EXIT_CODE=!ERRORLEVEL!"

popd



if !EXIT_CODE! neq 0 (

    call "%ROOT%\scripts\_log.bat" "[错误] 编译失败 !EXIT_CODE!"

    endlocal & exit /b !EXIT_CODE!

)



call "%ROOT%\scripts\_log.bat" "[OK] %EXE%"

endlocal & exit /b 0
