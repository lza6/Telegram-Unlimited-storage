# 完整镜像构建 + 部署（改 Dockerfile / Cargo.lock / 首次启动）
param(
    [switch]$NoCache,
    [string]$Image,
    [string]$BuilderImage,
    [string]$Container,
    [int]$HostPort = 0,
    [switch]$UseCompose
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\_docker-dev.ps1"
Import-TdDotEnv
Enable-DockerBuildKit
if ($HostPort -le 0) { $HostPort = Get-TdPort }

if (-not $Image) { $Image = $script:TD_IMAGE }
if (-not $BuilderImage) { $BuilderImage = $script:TD_BUILDER_IMAGE }
if (-not $Container) {
    $Container = if ($UseCompose) { $script:TD_COMPOSE_CONTAINER } else { Get-DevContainer "" }
}

Push-Location $script:TD_ROOT
try {
    $buildArgs = @(
        "build", "--pull=false",
        "-t", $Image,
        "--build-arg", "BUILDKIT_INLINE_CACHE=1"
    )
    if (-not $NoCache) {
        $cacheList = [System.Collections.Generic.List[string]]::new()
        $cacheWrap = [ref]$cacheList
        Add-LocalCacheFrom $cacheWrap $Image
        Add-LocalCacheFrom $cacheWrap $BuilderImage
        $buildArgs += $cacheList
    } else {
        $buildArgs += "--no-cache"
        Write-Host "WARNING: full rebuild (--NoCache)" -ForegroundColor Red
    }
    $buildArgs += "."

    Write-Host "docker build --pull=false (layer cache + cargo mount) -> $Image" -ForegroundColor Cyan
    & docker @buildArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

    # 保留 builder 阶段镜像，供 dev-build-rust 增量 --cache-from
    $builderArgs = @("build", "--pull=false", "--target", "builder", "-t", $BuilderImage, "--cache-from", $Image, ".")
    & docker @builderArgs | Out-Null

    docker ps -a --filter "name=^${Container}$" -q | ForEach-Object { docker rm -f $Container 2>$null | Out-Null }

    Write-Host "Starting container $Container on port $HostPort ..." -ForegroundColor Cyan
    docker @(New-DevContainerRunArgs -Container $Container -Image $Image -HostPort $HostPort) | Out-Null

    if (Wait-Health -HostPort $HostPort) {
        Write-Host "Deployed http://127.0.0.1:${HostPort}" -ForegroundColor Green
    }
    Write-Host "Next Rust edit: .\scripts\dev-build-rust.ps1" -ForegroundColor DarkGray
}
finally {
    Pop-Location
}
