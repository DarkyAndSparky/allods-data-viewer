<#
.SYNOPSIS
  Collects a small, private report about the game's .pak files for Allods Data Viewer.

.DESCRIPTION
  Reads every .pak (ZIP) under the game folder and writes, next to the script (or to -OutDir):
    report.txt           client files, versions, per-pak summary, extension statistics
    index\<pak>.csv      entry list: name, size, packed size
    headers.txt          first 64 bytes (hex) of the smallest entries of every extension
    samples\             up to -PerExt of the smallest entries of every extension (each at most -MaxKB)
    adv-samples.zip      all of the above in one file
  Nothing is uploaded. Nothing is modified in the game folder. The result is for sending to the
  project maintainer's assistant only; it must NEVER be committed to the repository.

  Works with Windows PowerShell 5.1 and PowerShell 7.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\collect-samples.ps1 -GameDir "C:\Games\Allods"
#>
param(
  [Parameter(Mandatory = $true)][string]$GameDir,
  [string]$OutDir = (Join-Path ([Environment]::GetFolderPath('Desktop')) 'adv-samples'),
  [int]$PerExt = 2,
  [int]$MaxKB = 300
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

if (-not (Test-Path -LiteralPath $GameDir)) { throw "Folder not found: $GameDir" }
if (Test-Path -LiteralPath $OutDir) { Remove-Item -LiteralPath $OutDir -Recurse -Force }
New-Item -ItemType Directory -Path $OutDir, (Join-Path $OutDir 'index'), (Join-Path $OutDir 'samples') | Out-Null

$report = New-Object System.Collections.Generic.List[string]
$report.Add("Allods Data Viewer sample report, $(Get-Date -Format 'yyyy-MM-dd HH:mm')")
$report.Add("Game folder: $GameDir")
$report.Add('')

# 1. Client files at the top level (names, sizes, versions of executables)
$report.Add('== Top level files ==')
Get-ChildItem -LiteralPath $GameDir -File | ForEach-Object {
  $v = ''
  if ($_.Extension -in '.exe', '.dll') { $v = " version=$($_.VersionInfo.FileVersion)" }
  $report.Add(("{0}  {1} bytes{2}" -f $_.Name, $_.Length, $v))
}
$report.Add('')

# 2. Other data files that are not inside a pak (.loc, .fsb, .bsb, .ogv, .bin ...)
$report.Add('== Loose data files (not .pak), first 200 ==')
Get-ChildItem -LiteralPath $GameDir -Recurse -File |
  Where-Object { $_.Extension -notin '.pak', '.exe', '.dll' } |
  Select-Object -First 200 |
  ForEach-Object { $report.Add(("{0}  {1} bytes" -f $_.FullName.Substring($GameDir.Length).TrimStart('\'), $_.Length)) }
$report.Add('')

# 3. Paks
$extStats = @{}      # ext -> list of @{Pak;Name;Len}
$paks = Get-ChildItem -LiteralPath $GameDir -Recurse -File -Filter *.pak
$report.Add("== Pak files: $($paks.Count) ==")
foreach ($pak in $paks) {
  try {
    $zip = [System.IO.Compression.ZipFile]::OpenRead($pak.FullName)
  } catch {
    $report.Add(("{0}: cannot open as ZIP ({1})" -f $pak.Name, $_.Exception.Message))
    continue
  }
  try {
    $rows = New-Object System.Collections.Generic.List[string]
    $rows.Add('name,size,packed')
    $n = 0
    foreach ($e in $zip.Entries) {
      if ($e.FullName.EndsWith('/')) { continue }
      $n++
      $rows.Add(('"{0}",{1},{2}' -f $e.FullName.Replace('"', '""'), $e.Length, $e.CompressedLength))
      $ext = [IO.Path]::GetExtension($e.FullName).ToLowerInvariant()
      if (-not $extStats.ContainsKey($ext)) { $extStats[$ext] = New-Object System.Collections.Generic.List[object] }
      $extStats[$ext].Add([pscustomobject]@{ Pak = $pak.FullName; PakName = $pak.Name; Name = $e.FullName; Len = $e.Length })
    }
    $rows | Set-Content -LiteralPath (Join-Path $OutDir ("index\" + $pak.Name + '.csv')) -Encoding UTF8
    $report.Add(("{0}: {1} entries, {2} bytes" -f $pak.Name, $n, $pak.Length))
  } finally { $zip.Dispose() }
}
$report.Add('')

# 4. Extension statistics
$report.Add('== Extensions across all paks ==')
$extStats.GetEnumerator() | Sort-Object { $_.Value.Count } -Descending | ForEach-Object {
  $sum = ($_.Value | Measure-Object Len -Sum).Sum
  $label = if ($_.Key) { $_.Key } else { '(none)' }
  $report.Add(("{0,-10} {1,7} entries {2,14} bytes" -f $label, $_.Value.Count, $sum))
}

# 5. Headers and small samples
$headers = New-Object System.Collections.Generic.List[string]
$maxBytes = $MaxKB * 1024
foreach ($kv in $extStats.GetEnumerator()) {
  $label = if ($kv.Key) { $kv.Key } else { '(none)' }
  $picked = $kv.Value | Where-Object { $_.Len -gt 0 } | Sort-Object Len | Select-Object -First ([Math]::Max($PerExt, 3))
  $copied = 0
  foreach ($item in $picked) {
    $zip = [System.IO.Compression.ZipFile]::OpenRead($item.Pak)
    try {
      $entry = $zip.GetEntry($item.Name)
      $stream = $entry.Open()
      try {
        $buf = New-Object byte[] 64
        $read = $stream.Read($buf, 0, 64)
      } finally { $stream.Dispose() }
      $hex = [BitConverter]::ToString($buf, 0, $read).Replace('-', ' ')
      $headers.Add(("{0} | {1} | {2} | {3} bytes | {4}" -f $label, $item.PakName, $item.Name, $item.Len, $hex))
      if ($copied -lt $PerExt -and $item.Len -le $maxBytes) {
        $safe = ($item.PakName + '__' + $item.Name).Replace('/', '__').Replace('\', '__')
        $target = Join-Path (Join-Path $OutDir 'samples') $safe
        [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $target, $true)
        $copied++
      }
    } finally { $zip.Dispose() }
  }
}
$headers | Set-Content -LiteralPath (Join-Path $OutDir 'headers.txt') -Encoding UTF8
$report | Set-Content -LiteralPath (Join-Path $OutDir 'report.txt') -Encoding UTF8

# 6. One archive to send
$zipPath = Join-Path (Split-Path $OutDir -Parent) 'adv-samples.zip'
if (Test-Path -LiteralPath $zipPath) { Remove-Item -LiteralPath $zipPath -Force }
[System.IO.Compression.ZipFile]::CreateFromDirectory($OutDir, $zipPath)
Write-Host "Done. Send this file: $zipPath"
