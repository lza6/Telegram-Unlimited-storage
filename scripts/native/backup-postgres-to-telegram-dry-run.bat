@echo off
setlocal EnableExtensions DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "ROOT=%%~fI"
if not "%~1"=="" (echo This launcher accepts no arguments.& exit /b 2)
python "%ROOT%\scripts\native\backup_postgres_to_telegram.py" --dry-run
exit /b %ERRORLEVEL%