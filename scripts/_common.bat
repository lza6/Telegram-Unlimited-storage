@echo off
chcp 65001 >nul 2>&1
REM Called by other bats only. Do not double-click.

if not defined TD_ROOT (
    set "TD_ROOT=%~dp0.."
    for %%I in ("!TD_ROOT!") do set "TD_ROOT=%%~fI"
)

if exist "%USERPROFILE%\.cargo\bin" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

set "CARGO_NET_GIT_FETCH_WITH_CLI=true"

if not defined RUST_LOG set "RUST_LOG=debug,actix_web=info,actix_server=info,h2=info"
if not defined RUST_BACKTRACE set "RUST_BACKTRACE=full"
if not defined RUST_LIB_BACKTRACE set "RUST_LIB_BACKTRACE=1"

exit /b 0
