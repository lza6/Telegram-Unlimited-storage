# 兼容入口，转 compose-up.ps1
param(
    [switch]$Rebuild,
    [switch]$Logs,
    [switch]$Build
)

& "$PSScriptRoot\compose-up.ps1" @PSBoundParameters
