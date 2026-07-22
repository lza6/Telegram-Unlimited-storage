# WebDAV smoke — requires WEBDAV_ENABLED=true in server env
param(
    [string]$BaseUrl = "http://localhost:1334",
    [string]$AccessPwd = "test"
)

$ErrorActionPreference = "Stop"
$auth = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("admin:$AccessPwd"))
$headers = @{ Authorization = "Basic $auth" }

$code = & curl.exe -s -o NUL -w "%{http_code}" -X OPTIONS "$BaseUrl/webdav/"
if ($code -eq "404") {
    Write-Host "[SKIP] WebDAV disabled (404 on OPTIONS /webdav/)" -ForegroundColor Yellow
    exit 0
}

$propfind = & curl.exe -s -w "`n%{http_code}" -X PROPFIND "$BaseUrl/webdav/" -H "Authorization: Basic $auth" -H "Depth: 1"
$body = ($propfind -split "`n")[0..-2] -join "`n"
$status = ($propfind -split "`n")[-1]
if ($status -ne "200") {
    Write-Host "[FAIL] PROPFIND http $status" -ForegroundColor Red
    exit 1
}
if ($body -notmatch "multistatus") {
    Write-Host "[FAIL] PROPFIND body missing multistatus" -ForegroundColor Red
    exit 1
}

$unauth = & curl.exe -s -o NUL -w "%{http_code}" -X PROPFIND "$BaseUrl/webdav/"
if ($unauth -ne "401") {
    Write-Host "[FAIL] expected 401 without auth, got $unauth" -ForegroundColor Red
    exit 1
}

Write-Host "[PASS] WebDAV OPTIONS/PROPFIND/auth" -ForegroundColor Green
