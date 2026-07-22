@echo off
setlocal EnableExtensions DisableDelayedExpansion
echo Restore is intentionally not parameterized through cmd.exe.
echo Run this exact command from the repository root and replace the two quoted paths:
echo python scripts\native\restore_telegram_backup.py --input "C:\path\to\backup.tdbak-or-directory" --output "C:\path\to\empty-output"
exit /b 2