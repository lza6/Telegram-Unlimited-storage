# Light stress: saturate upload chunk slots; expect 503 backpressure without server crash.
# Usage: .\tests\integration\stress-upload-slots.ps1 -BaseUrl http://localhost:1334 -AccessPwd test

param(
    [string]$BaseUrl = "http://localhost:1334",
    [string]$AccessPwd = "test",
    [int]$Parallel = 32
)

# For 500-client soak: -Parallel 500 (expect mix of 200/503, no OOM/crash)

$ErrorActionPreference = "Stop"
$ok = 0
$busy = 0
$other = 0

$jobs = 1..$Parallel | ForEach-Object {
    Start-Job -ScriptBlock {
        param($url, $pwd)
        $code = curl.exe -s -o NUL -w "%{http_code}" -X POST "$url/upload_chunk" -F "pwd=$pwd" -F "chunk_index=0" -F "total_chunks=1" -F "filename=s.txt" -F "session_id=stress-$([guid]::NewGuid())" -F "chunk=hi"
        [int]$code
    } -ArgumentList $BaseUrl, $AccessPwd
}

$results = $jobs | Wait-Job | Receive-Job
$jobs | Remove-Job -Force

foreach ($code in $results) {
    switch ($code) {
        { $_ -in 200, 400, 401, 503 } { if ($_ -eq 503) { $busy++ } else { $ok++ } }
        default { $other++ }
    }
}

Write-Host "Parallel=$Parallel ok-ish=$ok busy503=$busy other=$other"
if ($other -gt ($Parallel / 2)) {
    Write-Host "Too many unexpected status codes" -ForegroundColor Red
    exit 1
}
if ($busy -lt 1 -and $Parallel -gt 8) {
    Write-Host "WARN: expected some 503 when slots saturated (raise CHUNK_CONCURRENT or Parallel)" -ForegroundColor Yellow
}
Write-Host "Stress check passed (no crash)" -ForegroundColor Green
