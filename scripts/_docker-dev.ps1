# 共享 Docker 开发常量（被 dev-build-*.ps1 / dev-sync-web.ps1 / dev-update.ps1 引用）
$script:TD_ROOT = Split-Path -Parent $PSScriptRoot
$script:TD_IMAGE = "telegram-drive-api:local"
$script:TD_BUILDER_IMAGE = "telegram-drive-api:builder"
$script:TD_COMPOSE_CONTAINER = "telegram-drive-api"
$script:TD_LEGACY_CONTAINER = "td-api-test"
$script:TD_DEFAULT_PORT = 1334

function Import-TdDotEnv {
    $envFile = Join-Path $script:TD_ROOT ".env"
    if (-not (Test-Path $envFile)) { return }
    foreach ($line in Get-Content $envFile -Encoding UTF8) {
        $trimmed = $line.Trim()
        if ($trimmed -eq '' -or $trimmed.StartsWith('#')) { continue }
        $idx = $trimmed.IndexOf('=')
        if ($idx -lt 1) { continue }
        $key = $trimmed.Substring(0, $idx).Trim()
        $val = $trimmed.Substring($idx + 1).Trim()
        if ($val.Length -ge 2 -and $val.StartsWith('"') -and $val.EndsWith('"')) {
            $val = $val.Substring(1, $val.Length - 2)
        }
        Set-Item -Path "Env:$key" -Value $val -ErrorAction SilentlyContinue
    }
}

function Get-TdPort {
    if ($env:PORT) {
        return [int]$env:PORT
    }
    return $script:TD_DEFAULT_PORT
}

function Enable-DockerBuildKit {
    $env:DOCKER_BUILDKIT = "1"
    $env:COMPOSE_DOCKER_CLI_BUILD = "1"
}

function Test-LocalImage([string]$ImageRef) {
    if (-not $ImageRef) { return $false }
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    $null = & docker image inspect $ImageRef 2>&1
    $ok = ($LASTEXITCODE -eq 0)
    $ErrorActionPreference = $prev
    return $ok
}

function Add-LocalCacheFrom([ref]$ArgsList, [string]$ImageRef) {
    if (Test-LocalImage $ImageRef) {
        $ArgsList.Value += "--cache-from", $ImageRef
    }
}

function Get-DevContainer([string]$Preferred) {
    if ($Preferred) {
        $exists = docker ps -a --filter "name=^${Preferred}$" --format "{{.Names}}"
        if ($exists) { return $Preferred }
    }
    foreach ($name in @($script:TD_COMPOSE_CONTAINER, $script:TD_LEGACY_CONTAINER)) {
        $running = docker ps --filter "name=^${name}$" --format "{{.Names}}"
        if ($running) { return $name }
    }
    foreach ($name in @($script:TD_COMPOSE_CONTAINER, $script:TD_LEGACY_CONTAINER)) {
        $exists = docker ps -a --filter "name=^${name}$" --format "{{.Names}}"
        if ($exists) { return $name }
    }
    return $script:TD_COMPOSE_CONTAINER
}

function New-DevContainerRunArgs {
    param(
        [string]$Container,
        [string]$Image,
        [int]$HostPort = 0
    )
    if ($HostPort -le 0) {
        $HostPort = Get-TdPort
    }
    $envFile = Join-Path $script:TD_ROOT ".env"
    $runArgs = @(
        "run", "-d", "--name", $Container,
        "-p", "${HostPort}:${HostPort}",
        "-e", "PORT=$HostPort",
        "-e", "DATA_DIR=/data",
        "-e", "STATIC_DIR=/app/deploy/web",
        "-v", "$($script:TD_ROOT)/data:/data",
        "-v", "$($script:TD_ROOT)/deploy/web:/app/deploy/web:ro",
        "-v", "$($script:TD_ROOT)/docs:/app/docs:ro"
    )
    if (Test-Path $envFile) {
        $runArgs += @("--env-file", $envFile)
    } else {
        $runArgs += @(
            "-e", "TELEGRAM_API_ID=1",
            "-e", "TELEGRAM_API_HASH=dev",
            "-e", "ACCESS_PWD=test",
            "-e", "API_KEY=dev-key"
        )
    }
    $runArgs += $Image
    return $runArgs
}

function Wait-Health([int]$HostPort, [int]$Retries = 25) {
    for ($i = 0; $i -lt $Retries; $i++) {
        Start-Sleep -Seconds 2
        try {
            $h = Invoke-RestMethod -Uri "http://localhost:${HostPort}/api/v1/health" -TimeoutSec 3
            if ($h.status -eq "ok") { return $true }
        } catch { }
    }
    return $false
}
