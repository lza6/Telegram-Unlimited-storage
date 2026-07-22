# Share + presigned download smoke (no real Telegram upload required)
param(
    [string]$BaseUrl = "http://localhost:1334",
    [string]$AccessPwd = "test",
    [string]$ApiKey = ""
)

$ErrorActionPreference = "Stop"
if (-not $ApiKey) {
    Write-Host "[SKIP] share-download — pass -ApiKey" -ForegroundColor Yellow
    exit 0
}

$headers = @{ "X-API-Key" = $ApiKey }
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

Write-Host "Share/download tests on $BaseUrl ..."

try {
    $shares = Invoke-RestMethod -Uri "$BaseUrl/api/v1/shares" -Headers $headers -Method Get
    Assert-Ok "shares_list" ($null -ne $shares) "ok"
} catch {
    Assert-Ok "shares_list" $false $_.Exception.Message
}

$raw = & curl.exe -s -o NUL -w "%{http_code}" "$BaseUrl/d?file_id=999&filename=x.bin"
Assert-Ok "raw_file_id_blocked" ($raw -eq "403" -or $raw -eq "400" -or $raw -eq "503") "http $raw"

$badSig = & curl.exe -s -o NUL -w "%{http_code}" "$BaseUrl/d/signed?file_id=1&filename=x.bin&exp=0&sig=deadbeef"
Assert-Ok "presigned_bad_sig" ($badSig -eq "403" -or $badSig -eq "400") "http $badSig"

Write-Host "Result: $passed passed, $failed failed"
if ($failed -gt 0) { exit 1 }
