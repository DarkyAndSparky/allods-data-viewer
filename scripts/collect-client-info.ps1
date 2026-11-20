# Collects a "passport" of an Allods client folder for format research.
#
# Usage (PowerShell):
#   .\scripts\collect-client-info.ps1 -Client "C:\AstrumPlay\Аллоды Онлайн" -Name current
#
# Writes %USERPROFILE%\Downloads\BUILDS\client-<Name>\ with
#   files.txt   every file (relative path, size), without Cache and Mods
#   meta\       small version files (*.rev, *.version, *.ver, *.cfg is NOT copied)
# No game data is copied except those tiny version files, and nothing is uploaded.
param(
    [Parameter(Mandatory = $true)][string]$Client,
    [Parameter(Mandatory = $true)][string]$Name
)

$ErrorActionPreference = "Stop"
$out = Join-Path $env:USERPROFILE "Downloads\BUILDS\client-$Name"
New-Item -ItemType Directory -Force -Path $out, (Join-Path $out "meta") | Out-Null

$root = (Resolve-Path -LiteralPath $Client).Path
$skip = "\\Cache\\|\\Mods\\|\\Logs\\"
$files = Get-ChildItem -LiteralPath $root -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notmatch $skip }

$files |
    Select-Object @{ n = "path"; e = { $_.FullName.Substring($root.Length) } }, Length |
    Sort-Object path |
    ForEach-Object { "{0}`t{1}" -f $_.path, $_.Length } |
    Set-Content -Encoding UTF8 -LiteralPath (Join-Path $out "files.txt")

$files |
    Where-Object { $_.Length -lt 65536 -and $_.Extension -in ".rev", ".version", ".ver" } |
    ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $out "meta\$($_.Name)") -Force }

Write-Host "Done: $out"
Write-Host ("Files listed: {0}" -f $files.Count)
