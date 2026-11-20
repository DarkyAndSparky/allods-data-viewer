# Runs every check from CLAUDE.md in order. Usage: .\scripts\check-all.ps1
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
Step "web: typecheck" { Push-Location web; npm run typecheck; $c = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $c }
Step "web: test" { Push-Location web; npm test; $c = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $c }
Step "web: build" { Push-Location web; npm run build; $c = $LASTEXITCODE; Pop-Location; $global:LASTEXITCODE = $c }
Step "cargo fmt" { cargo fmt --all --check }
Step "cargo clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Step "cargo test" { cargo test --workspace }
Step "check-repo" { node scripts/check-repo.mjs }
Step "version check" { node scripts/version.mjs check }
Write-Host ""
Write-Host "Все проверки пройдены." -ForegroundColor Green
