@echo off
REM 从 .env 加载环境变量到调用方上下文（本脚本勿使用 setlocal）
set "ENV_FILE=%~1"
if not exist "%ENV_FILE%" exit /b 0

for /f "usebackq eol=# tokens=1,* delims==" %%A in ("%ENV_FILE%") do (
    if not "%%~A"=="" set "%%~A=%%~B"
)
exit /b 0
