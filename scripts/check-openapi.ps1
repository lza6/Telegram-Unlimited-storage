# Compare route_registry.rs IMPLEMENTED_ROUTES with docs/openapi.json paths.
# Usage: pwsh -File scripts/check-openapi.ps1

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

$openapiPath = Join-Path $root "docs/openapi.json"
$registryPath = Join-Path $root "app/src-tauri/src/route_registry.rs"

$doc = Get-Content $openapiPath -Raw | ConvertFrom-Json
$openapi = [System.Collections.Generic.HashSet[string]]::new()
foreach ($path in $doc.paths.PSObject.Properties.Name) {
    foreach ($method in $doc.paths.$path.PSObject.Properties.Name) {
        if ($method -match '^x' -or $method -eq 'parameters') { continue }
        [void]$openapi.Add("$($method.ToUpper()) $path")
    }
}

$text = Get-Content $registryPath -Raw
$impl = [System.Collections.Generic.HashSet[string]]::new()
[regex]::Matches($text, '\("([A-Z]+)",\s*"([^"]+)"\)') | ForEach-Object {
    [void]$impl.Add("$($_.Groups[1].Value) $($_.Groups[2].Value)")
}

$missingInOpenapi = $impl | Where-Object { -not $openapi.Contains($_) }
$extraInOpenapi = $openapi | Where-Object { -not $impl.Contains($_) }

if ($missingInOpenapi.Count -eq 0 -and $extraInOpenapi.Count -eq 0) {
    Write-Host "[PASS] OpenAPI paths match route_registry ($($impl.Count) routes)" -ForegroundColor Green
    exit 0
}

Write-Host "[FAIL] OpenAPI drift detected" -ForegroundColor Red
if ($missingInOpenapi) {
    Write-Host "In code but not OpenAPI:"
    $missingInOpenapi | ForEach-Object { Write-Host "  $_" }
}
if ($extraInOpenapi) {
    Write-Host "In OpenAPI but not route_registry:"
    $extraInOpenapi | ForEach-Object { Write-Host "  $_" }
}
exit 1
