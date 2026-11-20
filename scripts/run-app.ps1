# Builds the web part and starts the desktop window. Usage: .\scripts\run-app.ps1
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
Step "web: npm ci" { Push-Location web; npm ci; $c = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $c }
Step "web: build" { Push-Location web; npm run build; $c = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $c }
Step "запуск окна" { cargo run --release -p adv-desktop }
