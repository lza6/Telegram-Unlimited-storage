# Single entry: setup (first run) + incremental cargo run + logs + browser + cleanup
param(
    [ValidateSet("run", "release", "setup", "build", "stop")]
    [string]$Mode = "run"
)

$ErrorActionPreference = "Continue"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$Manifest = Join-Path $Root "app\src-tauri"
$FetchMarker = Join-Path $Root "data\.cargo-fetch-done"

function Write-Log {
    param([string]$Message)
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Write-Host "[$ts] $Message"
}

function Import-DotEnv {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return $false }
    Get-Content -LiteralPath $Path -Encoding UTF8 | ForEach-Object {
        $line = $_.Trim()
        if ($line -eq "" -or $line.StartsWith("#")) { return }
        $eq = $line.IndexOf("=")
        if ($eq -lt 1) { return }
        $key = $line.Substring(0, $eq).Trim()
        $val = $line.Substring($eq + 1).Trim()
        if ($key) { Set-Item -Path "env:$key" -Value $val }
    }
    return $true
}

function Resolve-RootPath {
    param([string]$Value, [string]$DefaultRelative)
    if (-not $Value) { return (Join-Path $Root $DefaultRelative) }
    if ([System.IO.Path]::IsPathRooted($Value)) { return $Value }
    $trimmed = $Value.TrimStart('.', '\', '/')
    return (Join-Path $Root $trimmed)
}

function Ensure-CargoPath {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoBin) {
        $env:PATH = "$cargoBin;$env:PATH"
    }
}

function Initialize-Environment {
    param([switch]$ForceFetch)

    Write-Log "========== environment check =========="
    Write-Log "ROOT=$Root"

    Ensure-CargoPath

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Log "[ERROR] cargo not found - install https://rustup.rs/ then reopen CMD"
        return 1
    }

    Write-Log "[OK] $(cargo --version 2>&1)"
    Write-Log "[OK] $(rustc --version 2>&1)"

    if (-not (Test-Path (Join-Path $Root "data"))) {
        New-Item -ItemType Directory -Path (Join-Path $Root "data") -Force | Out-Null
        Write-Log "[OK] created data\"
    }

    $envFile = Join-Path $Root ".env"
    if (-not (Test-Path $envFile)) {
        $example = Join-Path $Root ".env.example"
        if (Test-Path $example) {
            Copy-Item $example $envFile -Force
            Write-Log "[OK] created .env from .env.example"
            Write-Log "[!!] edit .env: ACCESS_PWD, TELEGRAM_API_ID/HASH"
        } else {
            Write-Log "[ERROR] missing .env and .env.example"
            return 1
        }
    } else {
        Write-Log "[OK] .env exists"
    }

    $needFetch = $ForceFetch -or -not (Test-Path $FetchMarker)
    if ($needFetch) {
        Write-Log "[..] cargo fetch (first run or forced)..."
        Push-Location $Manifest
        & cargo fetch
        $fetchRc = $LASTEXITCODE
        Pop-Location
        if ($fetchRc -ne 0) {
            Write-Log "[ERROR] cargo fetch failed (exit $fetchRc)"
            return $fetchRc
        }
        Set-Content -Path $FetchMarker -Value (Get-Date -Format "o") -Encoding UTF8
        Write-Log "[OK] cargo fetch done (marker: data\.cargo-fetch-done)"
    } else {
        Write-Log "[OK] skip cargo fetch (incremental dev; delete data\.cargo-fetch-done to refetch)"
    }

    return 0
}

function Apply-RuntimeEnv {
    if (-not (Import-DotEnv (Join-Path $Root ".env"))) {
        Write-Log "[ERROR] cannot load .env"
        return 1
    }

    $port = 1334
    if ($env:PORT) { [void][int]::TryParse($env:PORT, [ref]$port) }
    $env:PORT = "$port"
    $env:DATA_DIR = Resolve-RootPath $env:DATA_DIR "data"
    $env:STATIC_DIR = Resolve-RootPath $env:STATIC_DIR "deploy\web"
    $env:DOCS_DIR = Resolve-RootPath $env:DOCS_DIR "docs"
    if (-not $env:BIND_HOST) { $env:BIND_HOST = "0.0.0.0" }
    if (-not $env:RUST_LOG) { $env:RUST_LOG = "debug,actix_web=info,actix_server=info,h2=info" }
    $env:RUST_BACKTRACE = "full"
    $env:RUST_LIB_BACKTRACE = "1"
    $env:CARGO_INCREMENTAL = "1"
    $env:CARGO_TERM_VERBOSE = "true"

    if (-not (Test-Path $env:DATA_DIR)) {
        New-Item -ItemType Directory -Path $env:DATA_DIR -Force | Out-Null
    }

    if (-not $env:ACCESS_PWD) {
        Write-Log "[ERROR] ACCESS_PWD not set in .env"
        return 1
    }

    if (-not $env:TELEGRAM_API_ID -or -not $env:TELEGRAM_API_HASH) {
        Write-Log "[ERROR] TELEGRAM_API_ID / TELEGRAM_API_HASH required in .env"
        return 1
    }

    if ($env:TELEGRAM_API_ID -eq "123456" -or $env:TELEGRAM_API_HASH -eq "your_api_hash_here") {
        Write-Log "[WARN] Telegram API placeholder - web admin OK; bind/upload need real credentials"
    }

    return 0
}

function Stop-All {
    Write-Log "[..] stopping telegram-drive-server and helpers..."
    Get-Process -Name "telegram-drive-server" -ErrorAction SilentlyContinue |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -and $_.CommandLine -like "*wait-and-open-browser.ps1*" } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Write-Log "[OK] stop complete"
    return 0
}

function Start-BrowserWaiter {
    param([int]$Port)

    $script = Join-Path $PSScriptRoot "wait-and-open-browser.ps1"
    $cts = [System.Threading.CancellationTokenSource]::new()

    $task = [System.Threading.Tasks.Task]::Run({
        param($s, $p, $token)
        $deadline = (Get-Date).AddSeconds(180)
        $url = "http://localhost:$p/"
        while ((Get-Date) -lt $deadline -and -not $token.IsCancellationRequested) {
            try {
                $r = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 3
                if ($r.StatusCode -ge 200 -and $r.StatusCode -lt 500) {
                    Start-Process $url | Out-Null
                    return
                }
            } catch {
                Start-Sleep -Milliseconds 800
            }
        }
    }, @($script, $Port, $cts.Token))

    return @{ Task = $task; Cts = $cts }
}

Set-Location $Root
Write-Log "========== Telegram Drive (local, single CMD) =========="
Write-Log "MODE=$Mode  (Docker later: dev.bat / sync.bat)"

if ($Mode -eq "stop") {
    exit (Stop-All)
}

$initRc = Initialize-Environment
if ($initRc -ne 0) { exit $initRc }

if ($Mode -eq "setup") {
    Write-Log "setup-only done. Run start.bat again to launch server."
    exit 0
}

$envRc = Apply-RuntimeEnv
if ($envRc -ne 0) { exit $envRc }

$port = [int]$env:PORT
$url = "http://localhost:$port/"
Write-Log "DATA_DIR=$($env:DATA_DIR)"
Write-Log "STATIC_DIR=$($env:STATIC_DIR)"
Write-Log "URL=$url"
Write-Log "RUST_LOG=$($env:RUST_LOG)"

Push-Location $Manifest
$browser = $null
$exitCode = 0

try {
    if ($Mode -eq "build") {
        Write-Log "========== cargo build --release =========="
        & cargo build --release --bin telegram-drive-server --features headless-server
        $exitCode = $LASTEXITCODE
        exit $exitCode
    }

    if ($Mode -ne "run" -and $Mode -ne "release") {
        Write-Log "[ERROR] unknown mode $Mode"
        exit 1
    }

    Write-Log "[..] browser opens when $url is ready (same window, no extra CMD)"
    $browser = Start-BrowserWaiter -Port $port

    Write-Log "========== server log (close window or Ctrl+C = full stop) =========="
    Write-Host ""

    if ($Mode -eq "release") {
        & cargo build --release --bin telegram-drive-server --features headless-server
        $exitCode = $LASTEXITCODE
        if ($exitCode -eq 0) {
            $exe = Join-Path $Manifest "target\release\telegram-drive-server.exe"
            & $exe
            $exitCode = $LASTEXITCODE
        }
    } else {
        & cargo run --bin telegram-drive-server --features headless-server
        $exitCode = $LASTEXITCODE
    }
} catch {
    Write-Log "[ERROR] $_"
    $exitCode = 1
} finally {
    Pop-Location
    Write-Host ""
    Write-Log "[..] cleanup..."

    if ($browser) {
        $browser.Cts.Cancel()
        try { $browser.Task.Wait(2000) } catch { }
        $browser.Cts.Dispose()
    }

    Stop-All | Out-Null
    Write-Log "exited with code $exitCode"
}

exit $exitCode
