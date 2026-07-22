@echo off
setlocal EnableExtensions DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
set "ENV_FILE=%ROOT%\.env"
set "PSQL=C:\Program Files\PostgreSQL\16\bin\psql.exe"
for /f "usebackq tokens=1,* delims==" %%A in ("%ENV_FILE%") do (
  if "%%A"=="POSTGRES_PASSWORD" set "PGPASSWORD=%%B"
  if "%%A"=="POSTGRES_USER" set "PGUSER=%%B"
  if "%%A"=="POSTGRES_HOST" set "PGHOST=%%B"
  if "%%A"=="POSTGRES_PORT" set "PGPORT=%%B"
  if "%%A"=="POSTGRES_DB" set "PGDATABASE=%%B"
  if "%%A"=="SAGA_NODE_ID" set "SAGA_NODE_ID=%%B"
  if "%%A"=="SAGA_NODE_TOKEN" set "SAGA_NODE_TOKEN=%%B"
)
if not defined SAGA_NODE_TOKEN (echo SAGA_NODE_TOKEN is missing.& exit /b 1)
"%PSQL%" -X -v ON_ERROR_STOP=1 -f "%ROOT%\scripts\native\test-postgres-scheduler.sql"
exit /b %ERRORLEVEL%