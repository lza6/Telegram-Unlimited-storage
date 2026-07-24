# Unified Docker start/update: single compose stack (web/docs Volume + uvicorn --reload)
# Daily:   docker compose up -d
# Rebuild: .\compose-up.ps1 -Build  or  docker compose up -d --build
param(
    [switch]$Build,
    [switch]$Rebuild,
    [switch]$Sync,
    [switch]$Logs,
    [switch]$Release
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_docker-dev.ps1"
Import-TdDotEnv
Enable-DockerBuildKit

function Get-ComposeFileArgs {
    param([switch]$Release)
    if ($Release) { return @("-f", "docker-compose.yml") }
    return @("-f", "docker-compose.yml", "-f", "docker-compose.dev.yml")
}

# For typing `docker compose ...` directly in the shell (Windows uses ;, Linux/macOS uses :)
if ($Release) {
    $env:COMPOSE_FILE = "docker-compose.yml"
} elseif (-not $env:COMPOSE_FILE) {
    $sep = if ($IsWindows -or ($env:OS -match 'Windows')) { ';' } else { ':' }
    $env:COMPOSE_FILE = "docker-compose.yml${sep}docker-compose.dev.yml"
}

$composeFileArgs = Get-ComposeFileArgs -Release:$Release
$imageRef = if ($Release) { $script:TD_IMAGE } else { "telegram-drive-api:dev" }

Push-Location $script:TD_ROOT
try {
    $prevEa = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    docker rm -f td-api-test 2>&1 | Out-Null
    $ErrorActionPreference = $prevEa

    if ($Sync) {
        Write-Host "Sync: incremental — reload .env + volumes + container restart (no image build)" -ForegroundColor Cyan
        docker compose @composeFileArgs up -d
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        docker compose @composeFileArgs restart telegram-drive-api
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } else {
        if ($Rebuild) {
            Write-Host "Rebuilding image (requirements.txt / Dockerfile changed) ..." -ForegroundColor Yellow
            docker compose @composeFileArgs build --pull=false
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        } elseif (-not (Test-LocalImage $imageRef)) {
            Write-Host "First run or missing image — building $imageRef ..." -ForegroundColor Cyan
            docker compose @composeFileArgs build --pull=false
            if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
        }

        if ($Build -or $Rebuild) {
            Write-Host "docker compose up -d --build ..." -ForegroundColor Cyan
            docker compose @composeFileArgs up -d --build --pull=false
        } else {
            Write-Host "docker compose up -d ..." -ForegroundColor Cyan
            docker compose @composeFileArgs up -d
        }
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }

    $port = Get-TdPort
    $mode = if ($Release) { "release" } else { "dev (uvicorn --reload)" }
    Write-Host ""
    Write-Host "Running [$mode] on port $port" -ForegroundColor Green
    Write-Host "  After edits:  .\sync.bat  then browser http://localhost:${port}/  (Ctrl+F5)" -ForegroundColor Green
    Write-Host "  Web/docs:     volume mount (no rebuild)" -ForegroundColor DarkGray
    if (-not $Release) {
        Write-Host "  Python:       uvicorn --reload in container (or sync.bat restart)" -ForegroundColor DarkGray
    }
    Write-Host "  Logs:         docker compose logs -f telegram-drive-api" -ForegroundColor DarkGray
    Write-Host "  Full rebuild: only requirements.txt/Dockerfile -> compose-up.ps1 -Rebuild" -ForegroundColor DarkGray
    Write-Host ""

    if ($Logs) {
        docker compose @composeFileArgs logs -f
    }
}
finally {
    Pop-Location
}
