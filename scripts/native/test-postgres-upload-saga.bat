@echo off
setlocal EnableExtensions DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
set "ENV_FILE=%ROOT%\.env"
if not exist "%ENV_FILE%" (echo .env is missing.& exit /b 1)
for /f "usebackq tokens=1,* delims==" %%A in ("%ENV_FILE%") do (
  if "%%A"=="SAAS_DATABASE_MODE" set "SAAS_DATABASE_MODE=%%B"
  if "%%A"=="POSTGRES_HOST" set "POSTGRES_HOST=%%B"
  if "%%A"=="POSTGRES_PORT" set "POSTGRES_PORT=%%B"
  if "%%A"=="POSTGRES_DB" set "POSTGRES_DB=%%B"
  if "%%A"=="POSTGRES_APP_USER" set "POSTGRES_APP_USER=%%B"
  if "%%A"=="POSTGRES_APP_PASSWORD" set "POSTGRES_APP_PASSWORD=%%B"
)
if not defined SAAS_DATABASE_MODE set "SAAS_DATABASE_MODE=postgres"
if not defined POSTGRES_HOST set "POSTGRES_HOST=127.0.0.1"
if not defined POSTGRES_PORT set "POSTGRES_PORT=15432"
if not defined POSTGRES_DB (echo POSTGRES_DB is missing from .env.& exit /b 1)
if not defined POSTGRES_APP_USER (echo POSTGRES_APP_USER is missing from .env.& exit /b 1)
if not defined POSTGRES_APP_PASSWORD (echo POSTGRES_APP_PASSWORD is missing from .env.& exit /b 1)
if not defined CARGO_TARGET_DIR set "CARGO_TARGET_DIR=%TEMP%\telegram-drive-n2c-target"
pushd "%ROOT%\app\src-tauri" || exit /b 1
cargo test --lib --features headless-server upload_saga_ -- --ignored --nocapture
set "RC=%ERRORLEVEL%"
popd
exit /b %RC%
