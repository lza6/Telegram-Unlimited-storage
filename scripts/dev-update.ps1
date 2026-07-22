# 智能更新：Web 秒级 / Rust 增量 / 全镜像（按需）
param(
    [switch]$ForceFull,
    [string]$Container,
    [switch]$UseCompose
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$scriptDir = $PSScriptRoot

if (-not $Container) {
    . "$scriptDir\_docker-dev.ps1"
    $Container = if ($UseCompose) { $script:TD_COMPOSE_CONTAINER } else { Get-DevContainer "" }
}

if ($ForceFull) {
    & (Join-Path $scriptDir "dev-build-api.ps1") -Container $Container -UseCompose:$UseCompose
    exit $LASTEXITCODE
}

$rustChanged = $false
$webChanged = $false
try {
    $status = git -C $root status --porcelain 2>$null
    if ($status) {
        $rustChanged = $status -match 'app/src-tauri/'
        $webChanged = $status -match 'deploy/web/|docs/'
    } else {
        $rustChanged = $true
    }
} catch {
    $rustChanged = $true
}

if ($webChanged -and -not $rustChanged) {
    Write-Host "Web-only changes -> dev-sync-web" -ForegroundColor Cyan
    & (Join-Path $scriptDir "dev-sync-web.ps1") -Container $Container
    exit $LASTEXITCODE
}

if ($rustChanged) {
    Write-Host "Rust changes -> dev-build-rust (incremental)" -ForegroundColor Cyan
    & (Join-Path $scriptDir "dev-build-rust.ps1") -Container $Container -UseCompose:$UseCompose
    exit $LASTEXITCODE
}

Write-Host "No detected changes; syncing web anyway" -ForegroundColor DarkGray
& (Join-Path $scriptDir "dev-sync-web.ps1") -Container $Container
