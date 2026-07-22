@echo off
setlocal EnableExtensions DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
set "PG_BIN=C:\Program Files\PostgreSQL\16\bin"
set "PSQL=%PG_BIN%\psql.exe"
set "ENV_FILE=%ROOT%\.env"
if not exist "%PSQL%" (echo PostgreSQL 16 client tools are required.& exit /b 1)
if not exist "%ENV_FILE%" (echo .env is missing.& exit /b 1)

for /f "usebackq tokens=1,* delims==" %%A in ("%ENV_FILE%") do (
  if "%%A"=="POSTGRES_PASSWORD" set "PGPASSWORD=%%B"
  if "%%A"=="POSTGRES_USER" set "PGUSER=%%B"
  if "%%A"=="POSTGRES_DB" set "PGDATABASE=%%B"
  if "%%A"=="POSTGRES_APP_USER" set "POSTGRES_APP_USER=%%B"
  if "%%A"=="POSTGRES_APP_PASSWORD" set "POSTGRES_APP_PASSWORD=%%B"
)
if not defined PGPASSWORD (echo POSTGRES_PASSWORD is missing from .env.& exit /b 1)
if not defined PGUSER (echo POSTGRES_USER is missing from .env.& exit /b 1)
if not defined PGDATABASE (echo POSTGRES_DB is missing from .env.& exit /b 1)
if not defined POSTGRES_APP_USER (echo POSTGRES_APP_USER is missing from .env.& exit /b 1)
if not defined POSTGRES_APP_PASSWORD (echo POSTGRES_APP_PASSWORD is missing from .env.& exit /b 1)

"%PSQL%" -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 15432 -U "%PGUSER%" -d "%PGDATABASE%" -tAc "SELECT 'POSTGRES_RLS_FLAGS=' || count(*) FROM pg_class WHERE relname IN ('tenants','users','memberships','system_admins','tenant_api_keys','web_sessions','tenant_quotas','assets','transfer_jobs','usage_ledger','webhook_endpoints','webhook_deliveries','audit_events') AND relrowsecurity AND relforcerowsecurity; SELECT 'POSTGRES_APP_ROLE_RESTRICTED=' || ((NOT rolsuper AND NOT rolbypassrls AND NOT rolcreaterole AND NOT rolcreatedb)::int) FROM pg_roles WHERE rolname = '%POSTGRES_APP_USER%';"
set "PGPASSWORD=%POSTGRES_APP_PASSWORD%"
"%PSQL%" -X -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 15432 -U "%POSTGRES_APP_USER%" -d "%PGDATABASE%" -f "%ROOT%\scripts\native\test-postgres-rls.sql"
exit /b %ERRORLEVEL%