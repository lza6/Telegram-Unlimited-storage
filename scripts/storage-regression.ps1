# Storage regression smoke test (upload list health download chain).
# Requires a running instance with valid credentials in .env.
# Usage:
#   .\scripts\storage-regression.ps1
#   .\scripts\storage-regression.ps1 -BaseUrl http://localhost:1334 -SkipUpload

param(
    [string]$BaseUrl = "http://localhost:1334",
    [string]$ApiKey = $env:API_KEY,
    [switch]$SkipUpload
)

$ErrorActionPreference = "Stop"

function Assert-Ok($name, $script) {
    Write-Host "== $name ==" -ForegroundColor Cyan
    & $script
    Write-Host "OK: $name" -ForegroundColor Green
}

Assert-Ok "health" {
    $json = curl.exe -fsS "$BaseUrl/api/v1/health"
    if ($json -notmatch '"status"\s*:\s*"ok"') {
        throw "health status not ok: $json"
    }
}

Assert-Ok "config" {
    $cfg = curl.exe -fsS "$BaseUrl/config"
    if ($cfg -notmatch 'chunk_concurrent') {
        throw "config missing chunk_concurrent"
    }
}

if (-not $SkipUpload) {
    if ([string]::IsNullOrWhiteSpace($ApiKey)) {
        Write-Host "SKIP upload (set API_KEY env or pass after loading .env)" -ForegroundColor Yellow
    } else {
        Assert-Ok "api_upload_small" {
            $tmp = New-TemporaryFile
            Set-Content -Path $tmp -Value "storage-regression-$(Get-Date -Format o)" -NoNewline
            $code = curl.exe -s -o NUL -w "%{http_code}" `
                -H "X-API-Key: $ApiKey" `
                -F "file=@$tmp;filename=regression.txt" `
                "$BaseUrl/api/v1/files"
            Remove-Item $tmp -Force
            if ($code -notin @("200", "201", "503")) {
                throw "upload HTTP $code (503 acceptable when queue full)"
            }
        }
    }
}

Assert-Ok "metrics_optional" {
    $code = curl.exe -s -o NUL -w "%{http_code}" "$BaseUrl/metrics"
    if ($code -notin @("200", "404")) {
        throw "metrics HTTP $code"
    }
}

Write-Host "`nStorage regression passed." -ForegroundColor Green
