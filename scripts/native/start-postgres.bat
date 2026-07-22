@echo off
setlocal EnableExtensions DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
set "PG_BIN=C:\Program Files\PostgreSQL\16\bin"
set "PG_DATA=%ROOT%\data\postgres"
if not exist "%PG_BIN%\pg_ctl.exe" (echo PostgreSQL 16 is required.& exit /b 1)
if not exist "%PG_DATA%\PG_VERSION" (echo Native PostgreSQL cluster is not initialized. Run the project bootstrap first.& exit /b 1)
"%PG_BIN%\pg_ctl.exe" -D "%PG_DATA%" status >nul 2>&1 && (echo PostgreSQL already running on 127.0.0.1:15432.& exit /b 0)
"%PG_BIN%\pg_ctl.exe" -D "%PG_DATA%" -l "%PG_DATA%\postgres.log" -o "-h 127.0.0.1 -p 15432" start -w -t 60