# E2E headless server for Playwright (no real Telegram traffic).
# User transport mode: health checks local session only (none connected → ready=false, no getMe network).
param(
    [int]$Port = 1334,
    [string]$RepoRoot = ""
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}
$srcTauri = Join-Path $RepoRoot "app\src-tauri"
$dataDir = Join-Path $env:TEMP "td-e2e-headless-$Port"

if (-not (Test-Path $srcTauri)) {
    Write-Error "src-tauri not found at $srcTauri (RepoRoot=$RepoRoot)"
}

if (-not (Test-Path $dataDir)) {
    New-Item -ItemType Directory -Path $dataDir | Out-Null
}

$env:PORT = "$Port"
$env:ACCESS_PWD = if ($env:E2E_ACCESS_PWD) { $env:E2E_ACCESS_PWD } else { "test" }
$env:DATA_DIR = $dataDir
$env:STATIC_DIR = Join-Path $RepoRoot "deploy\web"
$env:DOCS_DIR = Join-Path $RepoRoot "docs"
$env:TELEGRAM_TRANSPORT_MODE = "user"
$env:TELEGRAM_API_ID = "12345"
$env:TELEGRAM_API_HASH = "e2e_dummy_hash_not_for_production"
$env:BIND_HOST = "127.0.0.1"
$env:API_KEY = "e2e-test-api-key"
$env:UPLOAD_QUEUE_BACKEND = "memory"

Push-Location $srcTauri
try {
    Write-Host "[e2e-headless] building telegram-drive-server..."
    cargo build --bin telegram-drive-server --features headless-server --quiet
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Write-Host "[e2e-headless] listening on http://127.0.0.1:$Port (DATA_DIR=$dataDir)"
    cargo run --bin telegram-drive-server --features headless-server --quiet
} finally {
    Pop-Location
}
