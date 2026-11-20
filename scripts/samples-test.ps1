# Runs the tests on real packs (nothing is copied). Usage:
#   .\scripts\samples-test.ps1 -Packs "C:\AstrumPlay\Аллоды Онлайн\data\Packs"
param([string]$Packs = "C:\AstrumPlay\Аллоды Онлайн\data\Packs")
$ErrorActionPreference = "Stop"
# Russian locale: console and files in UTF-8 (this file is UTF-8 with BOM).
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location -LiteralPath $root
function Step($name, [scriptblock]$cmd) {
    Write-Host ""
    Write-Host "=== $name ===" -ForegroundColor Cyan
    & $cmd
    if ($LASTEXITCODE -ne 0) { Write-Host "ОШИБКА на шаге: $name" -ForegroundColor Red; exit $LASTEXITCODE }
}
$env:ADV_SAMPLES = $Packs
Step "тесты на реальных файлах" { cargo test -p adv-core --test samples -- --nocapture }
