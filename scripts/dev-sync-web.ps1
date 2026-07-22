# 仅同步 deploy/web + docs 到运行中的容器（秒级，不编译 Rust）
param(
    [string]$Container
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_docker-dev.ps1"

if (-not $Container) { $Container = Get-DevContainer "" }

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Error "docker not found"
    exit 1
}

$running = docker ps --filter "name=^${Container}$" --format "{{.Names}}"
if (-not $running) {
    Write-Host "Container '$Container' not running. Start with:" -ForegroundColor Yellow
    Write-Host "  docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d"
    exit 1
}

$web = Join-Path $script:TD_ROOT "deploy\web"
$docs = Join-Path $script:TD_ROOT "docs"
docker cp "$web/." "${Container}:/app/deploy/web/"
docker cp "$docs/." "${Container}:/app/docs/"
Write-Host "Synced web -> $Container (no rebuild)" -ForegroundColor Green
