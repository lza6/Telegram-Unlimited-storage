@echo off
chcp 65001 >nul 2>&1
setlocal
cd /d "%~dp0.."
set DOCKER_BUILDKIT=1
set COMPOSE_DOCKER_CLI_BUILD=1
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0dev-build-rust.ps1" %*
exit /b %ERRORLEVEL%
