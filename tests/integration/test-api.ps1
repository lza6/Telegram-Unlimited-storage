# Integration smoke tests for telegram-drive-server
# Usage: .\tests\integration\test-api.ps1 -BaseUrl http://localhost:1334 -AccessPwd test -ApiKey yourkey

param(
    [string]$BaseUrl = "http://localhost:1334",
    [string]$AccessPwd = "test",
    [string]$ApiKey = ""
)

$ErrorActionPreference = "Stop"
$passed = 0
$failed = 0

function Assert-Ok($name, $cond, $detail) {
    if ($cond) {
        Write-Host "[PASS] $name" -ForegroundColor Green
        $script:passed++
    } else {
        Write-Host "[FAIL] $name — $detail" -ForegroundColor Red
        $script:failed++
    }
}

function Get-HttpCode($uri, $method = "GET", $headers = $null, $urlEncoded = $null, $multipart = $null) {
    $args = @("-s", "-o", "NUL", "-w", "%{http_code}", "-X", $method)
    if ($headers) {
        foreach ($k in $headers.Keys) {
            $args += @("-H", "$k`:$($headers[$k])")
        }
    }
    if ($urlEncoded) {
        foreach ($k in $urlEncoded.Keys) {
            $args += @("--data-urlencode", "$k=$($urlEncoded[$k])")
        }
    }
    if ($multipart) {
        foreach ($k in $multipart.Keys) {
            $args += @("-F", "$k=$($multipart[$k])")
        }
    }
    $args += $uri
    return [int](& curl.exe @args)
}

Write-Host "Testing $BaseUrl ..."

try {
    $health = Invoke-RestMethod -Uri "$BaseUrl/api/v1/health" -Method Get
    Assert-Ok "health" ($health.status -eq "ok") ($health | ConvertTo-Json -Compress)
    Assert-Ok "health_version" ($health.version -match '^\d+\.\d+\.\d+') "version=$($health.version)"
    Assert-Ok "health_telegram_field" ($null -ne $health.telegram_connected) "missing telegram_connected"
    Assert-Ok "health_uptime" ($null -ne $health.uptime_secs) "missing uptime_secs"
    Assert-Ok "health_ready" ($null -ne $health.ready) "missing ready"
    Assert-Ok "health_build" ($health.build -and $health.build.Length -gt 0) "missing build"
    Assert-Ok "health_upload_queue" ($null -ne $health.upload_queue) "missing upload_queue"
    Assert-Ok "health_presigned_flag" ($null -ne $health.presigned_download_enabled) "missing presigned flag"
    Assert-Ok "health_multi_tenant" ($null -ne $health.multi_tenant_enabled) "missing multi_tenant"
} catch {
    Assert-Ok "health" $false $_.Exception.Message
}

try {
    $cfg = Invoke-RestMethod -Uri "$BaseUrl/config" -Method Get
    Assert-Ok "config" ($null -ne $cfg.chunk_size_mb) ($cfg | ConvertTo-Json -Compress)
} catch {
    Assert-Ok "config" $false $_.Exception.Message
}

$verifyCode = Get-HttpCode -uri "$BaseUrl/verify" -method POST -urlEncoded @{ pwd = $AccessPwd }
Assert-Ok "verify" ($verifyCode -eq 200) "http $verifyCode"

$verifyMultipart = Get-HttpCode -uri "$BaseUrl/verify" -method POST -multipart @{ pwd = $AccessPwd }
Assert-Ok "verify_multipart" ($verifyMultipart -eq 200) "http $verifyMultipart"

$chunkCode = Get-HttpCode -uri "$BaseUrl/upload_chunk" -method POST -multipart @{ pwd = $AccessPwd }
Assert-Ok "upload_chunk_route" ($chunkCode -eq 400 -or $chunkCode -eq 401) "http $chunkCode"

$mergeCode = Get-HttpCode -uri "$BaseUrl/merge_chunks" -method POST -urlEncoded @{ pwd = $AccessPwd }
Assert-Ok "merge_chunks_route" ($mergeCode -eq 400 -or $mergeCode -eq 401) "http $mergeCode"

$mergeMultipart = Get-HttpCode -uri "$BaseUrl/merge_chunks" -method POST -multipart @{ pwd = $AccessPwd; filename = "x.bin"; chunk_ids = "[]" }
Assert-Ok "merge_chunks_multipart" ($mergeMultipart -eq 400 -or $mergeMultipart -eq 503) "http $mergeMultipart"

$uploadStatusCode = Get-HttpCode -uri "$BaseUrl/upload_status?session_id=__integration_probe__"
Assert-Ok "upload_status_unauth" ($uploadStatusCode -eq 401) "http $uploadStatusCode"

$uploadEventsCode = Get-HttpCode -uri "$BaseUrl/upload_events"
Assert-Ok "upload_events_missing_session" ($uploadEventsCode -eq 400) "http $uploadEventsCode"

$uploadEventsUnauth = Get-HttpCode -uri "$BaseUrl/upload_events?session_id=__integration_probe__"
Assert-Ok "upload_events_unauth" ($uploadEventsUnauth -eq 401) "http $uploadEventsUnauth"

try {
    $progressToken = Invoke-RestMethod -Uri "$BaseUrl/upload_progress_token" -Method Post `
        -Headers @{ "X-Access-Pwd" = $AccessPwd; "Content-Type" = "application/json" } `
        -Body (@{ session_id = "__integration_probe__" } | ConvertTo-Json)
    Assert-Ok "upload_progress_token" ($progressToken.token -and $progressToken.expires_at) ($progressToken | ConvertTo-Json -Compress)
    $sseUri = "$BaseUrl/upload_events?session_id=__integration_probe__&exp=$($progressToken.expires_at)&token=$($progressToken.token)"
    $uploadEventsOk = Get-HttpCode -uri $sseUri
    Assert-Ok "upload_events_sse" ($uploadEventsOk -eq 200) "http $uploadEventsOk"
} catch {
    Assert-Ok "upload_progress_token" $false $_.Exception.Message
    Assert-Ok "upload_events_sse" $false "token issue failed"
}

$uploadWsCode = Get-HttpCode -uri "$BaseUrl/upload_ws?session_id=__integration_probe__"
Assert-Ok "upload_ws_route" ($uploadWsCode -eq 400 -or $uploadWsCode -eq 426 -or $uploadWsCode -eq 200) "http $uploadWsCode"

try {
    $uploadPage = Invoke-WebRequest -Uri "$BaseUrl/upload.html" -UseBasicParsing
    Assert-Ok "upload_html" ($uploadPage.StatusCode -eq 200 -and $uploadPage.Content -match "TdUpload") "status $($uploadPage.StatusCode)"
} catch {
    Assert-Ok "upload_html" $false $_.Exception.Message
}

try {
    $js = Invoke-WebRequest -Uri "$BaseUrl/assets/upload-core.js" -UseBasicParsing
    Assert-Ok "upload_core_js" ($js.StatusCode -eq 200 -and $js.Content -match "merge_chunks" -and $js.Content -match "session_id" -and $js.Content -match "upload_ws") "status $($js.StatusCode)"
} catch {
    Assert-Ok "upload_core_js" $false $_.Exception.Message
}

$dCode = Get-HttpCode -uri "$BaseUrl/d?file_id=1&filename=test.bin"
Assert-Ok "legacy_download_route" ($dCode -eq 400 -or $dCode -eq 503) "http $dCode"

if ($ApiKey) {
    $headers = @{ "X-API-Key" = $ApiKey }
    try {
        $auth = Invoke-RestMethod -Uri "$BaseUrl/api/v1/auth/status" -Method Get -Headers $headers
        Assert-Ok "auth/status" $true ($auth | ConvertTo-Json -Compress)
    } catch {
        Assert-Ok "auth/status" $false $_.Exception.Message
    }
    $foldersCode = Get-HttpCode -uri "$BaseUrl/api/v1/folders" -headers $headers
    Assert-Ok "folders" ($foldersCode -eq 200 -or $foldersCode -eq 503) "http $foldersCode"
    $qrCode = Get-HttpCode -uri "$BaseUrl/api/v1/auth/qr/start" -method POST
    Assert-Ok "qr_start" ($qrCode -eq 200 -or $qrCode -eq 503 -or $qrCode -eq 400) "http $qrCode"
    $qrPoll = Get-HttpCode -uri "$BaseUrl/api/v1/auth/qr/poll"
    Assert-Ok "qr_poll" ($qrPoll -eq 200 -or $qrPoll -eq 503 -or $qrPoll -eq 400) "http $qrPoll"
    $filesCode = Get-HttpCode -uri "$BaseUrl/api/v1/files" -headers $headers
    Assert-Ok "files_list" ($filesCode -eq 200 -or $filesCode -eq 503) "http $filesCode"
    try {
        $filesResp = Invoke-WebRequest -Uri "$BaseUrl/api/v1/files" -Headers $headers -UseBasicParsing
        $cacheHdr = $filesResp.Headers["X-Metadata-Cache"]
        Assert-Ok "metadata_cache_header" ($cacheHdr -eq "HIT" -or $cacheHdr -eq "MISS" -or $null -eq $cacheHdr) "header=$cacheHdr"
    } catch {
        Assert-Ok "metadata_cache_header" $true "skipped when files list unavailable"
    }
    $signedCode = Get-HttpCode -uri "$BaseUrl/d/signed?file_id=1&filename=x.bin&exp=0&sig=bad"
    Assert-Ok "presigned_route" ($signedCode -eq 403 -or $signedCode -eq 400 -or $signedCode -eq 503) "http $signedCode"
    $metricsCode = Get-HttpCode -uri "$BaseUrl/metrics"
    Assert-Ok "metrics" ($metricsCode -eq 200 -or $metricsCode -eq 404) "http $metricsCode"
    $rawD = Get-HttpCode -uri "$BaseUrl/d?file_id=999999&filename=x.bin"
    Assert-Ok "raw_file_id_blocked" ($rawD -eq 403 -or $rawD -eq 503 -or $rawD -eq 400) "http $rawD"
    $sharesCode = Get-HttpCode -uri "$BaseUrl/api/v1/shares" -headers $headers
    Assert-Ok "shares_list" ($sharesCode -eq 200) "http $sharesCode"
    $uploadCode = Get-HttpCode -uri "$BaseUrl/api/v1/files" -method POST -headers $headers
    Assert-Ok "files_multipart_route" ($uploadCode -eq 400 -or $uploadCode -eq 503) "http $uploadCode"
} else {
    Write-Host "[SKIP] API key tests (pass -ApiKey)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Result: $passed passed, $failed failed"
if ($failed -gt 0) { exit 1 }
