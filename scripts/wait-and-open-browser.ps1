param(
    [int]$Port = 1334,
    [string]$HostName = "localhost",
    [int]$TimeoutSec = 180
)

$ErrorActionPreference = "SilentlyContinue"
$url = "http://${HostName}:${Port}/"
$deadline = (Get-Date).AddSeconds($TimeoutSec)

while ((Get-Date) -lt $deadline) {
    try {
        $r = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 3
        if ($r.StatusCode -ge 200 -and $r.StatusCode -lt 500) {
            Start-Process $url | Out-Null
            exit 0
        }
    } catch {
        Start-Sleep -Milliseconds 800
    }
}

exit 1
