# Compatibility shim — use scripts\run-local.ps1 via start.bat
param(
    [switch]$Release
)

$mode = if ($Release) { "release" } else { "run" }
& (Join-Path $PSScriptRoot "run-local.ps1") -Mode $mode
exit $LASTEXITCODE
