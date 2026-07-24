# Smart update: Web instant / Python reload / full image (as needed)
# Python backend runs uvicorn --reload in dev, so backend edits hot-reload
# automatically; this script only needs to rebuild the image when
# requirements.txt / Dockerfile change.
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

function Invoke-FullRebuild {
    Write-Host "Full image rebuild (requirements.txt / Dockerfile) ..." -ForegroundColor Yellow
    & (Join-Path $scriptDir "compose-up.ps1") -Rebuild
    exit $LASTEXITCODE
}

if ($ForceFull) {
    Invoke-FullRebuild
}

$depsChanged = $false
$backendChanged = $false
$webChanged = $false
try {
    $status = git -C $root status --porcelain 2>$null
    if ($status) {
        $depsChanged    = $status -match 'requirements.*\.txt|Dockerfile|docker-compose'
        $backendChanged = $status -match 'backend/'
        $webChanged     = $status -match 'deploy/web/|docs/'
    } else {
        $depsChanged = $true
    }
} catch {
    $depsChanged = $true
}

if ($depsChanged) {
    Invoke-FullRebuild
}

if ($webChanged -and -not $backendChanged) {
    Write-Host "Web-only changes -> dev-sync-web" -ForegroundColor Cyan
    & (Join-Path $scriptDir "dev-sync-web.ps1") -Container $Container
    exit $LASTEXITCODE
}

if ($backendChanged) {
    Write-Host "Backend changes -> uvicorn --reload hot-reloads in dev; restarting container to be safe" -ForegroundColor Cyan
    if ($UseCompose) {
        docker compose -f (Join-Path $root "docker-compose.yml") -f (Join-Path $root "docker-compose.dev.yml") restart telegram-drive-api
    } elseif ($Container) {
        docker restart $Container
    }
    exit $LASTEXITCODE
}

Write-Host "No detected changes; syncing web anyway" -ForegroundColor DarkGray
& (Join-Path $scriptDir "dev-sync-web.ps1") -Container $Container
