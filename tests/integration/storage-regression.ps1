#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Storage regression test for Telegram-Drive API (K-Vault pattern)

.DESCRIPTION
    Validates the complete upload/download/share/delete cycle against
    a running Telegram-Drive API server. Run this before production
    deployments to catch regressions early.

.PARAMETER BaseUrl
    API base URL (default: http://localhost:1334)

.PARAMETER AccessPwd
    Admin password for X-Access-Pwd header

.PARAMETER ApiKey
    Alternative API key for X-API-Key header

.PARAMETER TimeoutSec
    Request timeout in seconds (default: 30)

.EXAMPLE
    .\storage-regression.ps1 -BaseUrl http://localhost:1334 -AccessPwd "your-password"

.EXAMPLE
    .\storage-regression.ps1 -BaseUrl https://oss.example.com -ApiKey "kvault_xxx"
#>

param(
    [string]$BaseUrl = "http://localhost:1334",
    [string]$AccessPwd = "",
    [string]$ApiKey = "",
    [int]$TimeoutSec = 30
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

# Colors for output
function Write-Step { param($msg) Write-Host "`n==>$msg" -ForegroundColor Cyan }
function Write-Pass { param($msg) Write-Host "  ✓ $msg" -ForegroundColor Green }
function Write-Fail { param($msg) Write-Host "  ✗ $msg" -ForegroundColor Red }
function Write-Info { param($msg) Write-Host "  → $msg" -ForegroundColor Gray }

# HTTP headers
$headers = @{
    "Content-Type" = "application/json"
}
if ($AccessPwd) {
    $headers["X-Access-Pwd"] = $AccessPwd
}
if ($ApiKey) {
    $headers["X-API-Key"] = $ApiKey
}

$testFileId = $null
$testShareToken = $null
$errors = @()

# Test 1: Health Check
Write-Step "Test 1: Health Check"
try {
    $health = Invoke-RestMethod -Uri "$BaseUrl/health/ready" -Method Get -Headers $headers -TimeoutSec $TimeoutSec
    if ($health.status -eq "ok" -and $health.ready -eq $true) {
        Write-Pass "Readiness check passed: status=ok, ready=true"
        Write-Info "Telegram connected: $($health.telegram_connected)"
        Write-Info "Ready: $($health.ready)"
    } else {
        Write-Fail "Readiness check failed: status=$($health.status), ready=$($health.ready)"
        $errors += "health-check"
    }
} catch {
    Write-Fail "Health check failed: $_"
    $errors += "health-check"
}

# Test 2: Upload small file
Write-Step "Test 2: Upload File"
try {
    # Generate random test content
    $testContent = "Regression test $(Get-Date -Format 'yyyy-MM-dd_HH:mm:ss') - $([Guid]::NewGuid())"
    $testBytes = [System.Text.Encoding]::UTF8.GetBytes($testContent)
    $testBase64 = [Convert]::ToBase64String($testBytes)

    # Create multipart form
    $boundary = "----RegressionTest$([Guid]::NewGuid().ToString('N'))"
    $bodyLines = @(
        "--$boundary",
        "Content-Disposition: form-data; name=`"file`"; filename=`"regression-test.txt`"",
        "Content-Type: text/plain",
        "",
        $testContent,
        "--$boundary--"
    )
    $body = $bodyLines -join "`r`n"

    $uploadHeaders = @{
        "Content-Type" = "multipart/form-data; boundary=$boundary"
    }
    if ($AccessPwd) { $uploadHeaders["X-Access-Pwd"] = $AccessPwd }
    if ($ApiKey) { $uploadHeaders["X-API-Key"] = $ApiKey }

    $upload = Invoke-RestMethod -Uri "$BaseUrl/api/v1/upload" -Method Post -Headers $uploadHeaders -Body $body -TimeoutSec $TimeoutSec

    if ($upload.success) {
        $testFileId = $upload.file_id
        Write-Pass "Upload succeeded: file_id=$testFileId"
        Write-Info "Size: $($upload.size) bytes"
    } else {
        Write-Fail "Upload failed: $($upload.error)"
        $errors += "upload"
    }
} catch {
    Write-Fail "Upload failed: $_"
    $errors += "upload"
}

# Test 3: List files (verify upload)
Write-Step "Test 3: List Files"
try {
    $list = Invoke-RestMethod -Uri "$BaseUrl/api/v1/files?limit=10" -Method Get -Headers $headers -TimeoutSec $TimeoutSec
    $found = $list.files | Where-Object { $_.id -eq $testFileId -or $_.file_id -eq $testFileId }
    if ($found) {
        Write-Pass "File appears in list"
        Write-Info "Total files: $($list.total)"
    } else {
        Write-Fail "Uploaded file not found in list"
        $errors += "list-files"
    }
} catch {
    Write-Fail "List files failed: $_"
    $errors += "list-files"
}

# Test 4: Download file
Write-Step "Test 4: Download File"
if ($testFileId) {
    try {
        $downloadHeaders = $headers.Clone()
        $downloadHeaders["Accept"] = "application/octet-stream"

        $download = Invoke-RestMethod -Uri "$BaseUrl/api/v1/files/$testFileId/download" -Method Get -Headers $downloadHeaders -TimeoutSec $TimeoutSec

        if ($download) {
            Write-Pass "Download succeeded"
            Write-Info "Content length: $($download.Length)"
        } else {
            Write-Fail "Download returned empty"
            $errors += "download"
        }
    } catch {
        Write-Fail "Download failed: $_"
        $errors += "download"
    }
} else {
    Write-Info "Skipped: No file_id from upload"
}

# Test 5: Create share
Write-Step "Test 5: Create Share"
if ($testFileId) {
    try {
        $shareBody = @{
            file_id = $testFileId
            expires_in_hours = 1
        } | ConvertTo-Json

        $share = Invoke-RestMethod -Uri "$BaseUrl/api/v1/shares" -Method Post -Headers $headers -Body $shareBody -TimeoutSec $TimeoutSec

        if ($share.success -or $share.token) {
            $testShareToken = $share.token
            Write-Pass "Share created: token=$testShareToken"
            Write-Info "URL: $($share.url)"
        } else {
            Write-Fail "Share creation failed: $($share.error)"
            $errors += "create-share"
        }
    } catch {
        Write-Fail "Create share failed: $_"
        $errors += "create-share"
    }
} else {
    Write-Info "Skipped: No file_id from upload"
}

# Test 6: Download via share link
Write-Step "Test 6: Download via Share Link"
if ($testShareToken) {
    try {
        $shareDownload = Invoke-RestMethod -Uri "$BaseUrl/s/$testShareToken" -Method Get -TimeoutSec $TimeoutSec
        if ($shareDownload) {
            Write-Pass "Share download succeeded"
        } else {
            Write-Fail "Share download returned empty"
            $errors += "share-download"
        }
    } catch {
        # May return redirect or file stream
        if ($_.Exception.Response.StatusCode -eq 302 -or $_.Exception.Response.StatusCode -eq 200) {
            Write-Pass "Share download initiated (redirect/stream)"
        } else {
            Write-Fail "Share download failed: $_"
            $errors += "share-download"
        }
    }
} else {
    Write-Info "Skipped: No share token"
}

# Test 7: Delete file
Write-Step "Test 7: Delete File"
if ($testFileId) {
    try {
        $delete = Invoke-RestMethod -Uri "$BaseUrl/api/v1/files/$testFileId" -Method Delete -Headers $headers -TimeoutSec $TimeoutSec
        Write-Pass "Delete succeeded"
    } catch {
        Write-Fail "Delete failed: $_"
        $errors += "delete"
    }
} else {
    Write-Info "Skipped: No file_id from upload"
}

# Test 8: Verify deletion
Write-Step "Test 8: Verify Deletion"
if ($testFileId) {
    Start-Sleep -Seconds 1
    try {
        $verifyList = Invoke-RestMethod -Uri "$BaseUrl/api/v1/files?limit=100" -Method Get -Headers $headers -TimeoutSec $TimeoutSec
        $stillExists = $verifyList.files | Where-Object { $_.id -eq $testFileId -or $_.file_id -eq $testFileId }
        if (-not $stillExists) {
            Write-Pass "File no longer in list (deleted)"
        } else {
            Write-Fail "File still exists in list"
            $errors += "verify-delete"
        }
    } catch {
        Write-Fail "Verify deletion failed: $_"
        $errors += "verify-delete"
    }
} else {
    Write-Info "Skipped: No file_id"
}

# Test 9: Metrics endpoint
Write-Step "Test 9: Metrics Endpoint"
try {
    $metrics = Invoke-RestMethod -Uri "$BaseUrl/metrics" -Method Get -TimeoutSec $TimeoutSec
    if ($metrics -match "telegram_drive_uptime_seconds") {
        Write-Pass "Metrics endpoint working"
        # Extract bot pool metrics if available
        if ($metrics -match "telegram_drive_bot_pool_total (\d+)") {
            Write-Info "Bot pool total: $([regex]::Match($metrics, 'telegram_drive_bot_pool_total (\d+)').Groups[1].Value)"
        }
    } else {
        Write-Fail "Metrics format unexpected"
        $errors += "metrics"
    }
} catch {
    Write-Info "Metrics endpoint may be disabled (this is OK)"
}

# Summary
Write-Host "`n========================================" -ForegroundColor Cyan
if ($errors.Count -eq 0) {
    Write-Host "All tests passed!" -ForegroundColor Green
    exit 0
} else {
    Write-Host "Failed tests: $($errors -join ', ')" -ForegroundColor Red
    exit 1
}
