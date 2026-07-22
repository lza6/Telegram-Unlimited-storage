@echo off
setlocal EnableExtensions DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
if not "%~1"=="" (
  echo This launcher accepts no arguments. Use backup-postgres-to-telegram-dry-run.bat or backup-postgres-to-telegram-keep-local.bat.
  exit /b 2
)
python "%ROOT%\scripts\native\backup_postgres_to_telegram.py"
exit /b %ERRORLEVEL%