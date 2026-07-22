@echo off
chcp 65001 >nul 2>&1
echo [%date% %time%] %~1
exit /b 0
