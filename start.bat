@echo off
setlocal EnableExtensions DisableDelayedExpansion
chcp 65001 >nul
title Telegram Drive Windows Launcher (Python)

set "ROOT=%~dp0"
pushd "%ROOT%" >nul 2>&1 || (
  echo [ERROR] Cannot enter project dir: %ROOT%
  exit /b 1
)

set "ACTION=%~1"
set "RC=0"
if /i "%ACTION%"=="" goto menu
if /i "%ACTION%"=="server" goto server
if /i "%ACTION%"=="docker" goto docker
if /i "%ACTION%"=="help" goto help

echo [ERROR] Unknown argument: %ACTION%
set "RC=1"
goto help

:menu
echo.
echo ========================================
echo      Telegram Drive Launcher (Python)
echo ========================================
echo [1] Headless API server (local Python)
echo [2] Docker Compose service
echo [0] Exit
echo.
set /p "CHOICE=Select option: "
if "%CHOICE%"=="1" goto server
if "%CHOICE%"=="2" goto docker
if "%CHOICE%"=="0" goto end
echo [ERROR] Enter 0, 1 or 2.
goto menu

:load_env
if not exist ".env" (
  echo [INFO] No .env found; copy .env.example and fill in Telegram + access credentials.
  exit /b 0
)
for /f "usebackq eol=# tokens=1,* delims==" %%A in (".env") do (
  if not "%%A"=="" set "%%A=%%B"
)
exit /b 0

:require_python
set "PY="
where python >nul 2>&1 && set "PY=python"
if not defined PY (
  echo [ERROR] Python not found. Install Python 3.11+ and reopen the terminal.
  exit /b 1
)
exit /b 0

:server
call :load_env
if not defined ACCESS_PWD (
  echo [ERROR] Headless server requires ACCESS_PWD; set it in .env.
  goto failed
)
if not defined API_KEY (
  echo [ERROR] Headless server requires API_KEY; set it in .env. No plaintext key is generated or printed.
  goto failed
)
call :require_python || goto failed
if not defined DATA_DIR set "DATA_DIR=%ROOT%data"
if not defined STATIC_DIR set "STATIC_DIR=%ROOT%deploy\web"
if not defined DOCS_DIR set "DOCS_DIR=%ROOT%docs"
set "BIND_HOST=127.0.0.1"
if not defined PORT set "PORT=1334"
echo [INFO] Starting Headless API: http://%BIND_HOST%:%PORT%
echo [INFO] Credentials are not shown. Press Ctrl+C to stop safely.
pushd "backend" || goto failed
%PY% -m uvicorn app.main:app --host %BIND_HOST% --port %PORT%
set "RC=%ERRORLEVEL%"
popd
if not "%RC%"=="0" goto failed
goto end

:docker
where docker >nul 2>&1 || (
  echo [ERROR] Docker CLI not found. Install and start Docker Desktop.
  goto failed
)
call :load_env
if not defined API_KEY (
  echo [ERROR] Docker headless service requires API_KEY; set it in .env.
  goto failed
)
if not defined ACCESS_PWD (
  echo [ERROR] Docker headless service requires ACCESS_PWD; set it in .env.
  goto failed
)
echo [INFO] Starting base Docker Compose. Service binds to 127.0.0.1 only; use a TLS reverse proxy for public access.
docker compose -f docker-compose.yml up --build
if errorlevel 1 goto failed
goto end

:help
echo.
echo Usage:
echo   start.bat server   Start local Python headless API server
echo   start.bat docker   Start Docker Compose service
echo   start.bat          Show interactive menu
goto end

:failed
echo.
echo [FAILED] Startup did not complete; fix the environment or config per the errors above.
set "RC=1"

:end
popd >nul 2>&1
endlocal & exit /b %RC%
