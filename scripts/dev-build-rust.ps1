# 增量编译 Rust 业务代码（只 build --target builder，复用 cargo cache mount）
# 改 app/src-tauri/src 后优先用这个，比全量 docker build 快一个数量级
param(
    [switch]$NoCache,
    [string]$BuilderImage,
    [string]$RuntimeImage,
    [string]$Container,
    [int]$HostPort = 0,
    [switch]$RebuildRuntime,
    [switch]$UseCompose
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_docker-dev.ps1"
Import-TdDotEnv
Enable-DockerBuildKit
if ($HostPort -le 0) { $HostPort = Get-TdPort }

if (-not $BuilderImage) { $BuilderImage = $script:TD_BUILDER_IMAGE }
if (-not $RuntimeImage) { $RuntimeImage = $script:TD_IMAGE }
if (-not $Container) {
    $Container = if ($UseCompose) { $script:TD_COMPOSE_CONTAINER } else { Get-DevContainer "" }
}

function Restart-ContainerWithBinary {
    param([string]$BinaryPath, [string]$TargetContainer)
    $exists = docker ps -a --filter "name=^${TargetContainer}$" --format "{{.Names}}"
    if (-not $exists) {
        Write-Host "No container '$TargetContainer' — creating ..." -ForegroundColor Yellow
        if (-not (Test-LocalImage $RuntimeImage)) {
            Write-Host "Run dev-build-api.ps1 once for full image" -ForegroundColor Red
            exit 1
        }
        docker @(New-DevContainerRunArgs -Container $TargetContainer -Image $RuntimeImage -HostPort $HostPort) | Out-Null
    }
    docker cp $BinaryPath "${TargetContainer}:/app/telegram-drive-server"
    docker restart $TargetContainer | Out-Null
}

Push-Location $script:TD_ROOT
try {
    $buildArgs = @(
        "build", "--pull=false",
        "--target", "builder",
        "-t", $BuilderImage,
        "--build-arg", "BUILDKIT_INLINE_CACHE=1"
    )
    if (-not $NoCache) {
        $cacheList = [System.Collections.Generic.List[string]]::new()
        $cacheWrap = [ref]$cacheList
        Add-LocalCacheFrom $cacheWrap $BuilderImage
        Add-LocalCacheFrom $cacheWrap $RuntimeImage
        $buildArgs += $cacheList
    } else {
        $buildArgs += "--no-cache"
    }
    $buildArgs += "."

    Write-Host "[1/4] Incremental builder (deps cached, src only) -> $BuilderImage" -ForegroundColor Cyan
    & docker @buildArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    $extract = "td-bin-extract-$PID"
    docker rm -f $extract 2>$null | Out-Null
    docker create --name $extract $BuilderImage | Out-Null
    $localBin = Join-Path $env:TEMP "telegram-drive-server-$PID"
    docker cp "${extract}:/export/telegram-drive-server" $localBin
    docker rm -f $extract | Out-Null

    if ($RebuildRuntime) {
        Write-Host "[2/4] Rebuilding runtime image -> $RuntimeImage" -ForegroundColor Cyan
        $rtArgs = @("build", "--pull=false", "-t", $RuntimeImage, "--cache-from", $BuilderImage, ".")
        if ($NoCache) { $rtArgs += "--no-cache" }
        & docker @rtArgs
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    } else {
        Write-Host "[2/4] Skip runtime image (hot-swap binary into container)" -ForegroundColor DarkGray
    }

    Write-Host "[3/4] Deploy binary -> $Container" -ForegroundColor Cyan
    Restart-ContainerWithBinary -BinaryPath $localBin -TargetContainer $Container
    Remove-Item -Force $localBin -ErrorAction SilentlyContinue

    Write-Host "[4/4] Health check http://127.0.0.1:${HostPort}/api/v1/health" -ForegroundColor Cyan
    if (Wait-Health -HostPort $HostPort) {
        Write-Host "OK http://127.0.0.1:${HostPort}" -ForegroundColor Green
    } else {
        Write-Host "Binary deployed; check: docker logs $Container" -ForegroundColor Yellow
    }
}
finally {
    Pop-Location
}
