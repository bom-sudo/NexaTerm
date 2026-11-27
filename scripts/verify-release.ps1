<#
.SYNOPSIS
  Verifies a NexaTerm release directory: files present and non-empty, PE executables, version
  metadata, ZIP integrity and layout, SHA-256 checksums. -RunSelfTest also extracts the portable
  ZIP to a temp folder and runs `NexaTerm.exe --self-test`. -RequireSignature fails unless every
  installer and the portable NexaTerm.exe carry a valid, timestamped Authenticode signature.
#>
param(
  [string]$ReleaseDir,
  [switch]$RunSelfTest,
  [switch]$RequireSignature
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.IO.Compression.FileSystem
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$version = (Get-Content "$root\package.json" -Raw | ConvertFrom-Json).version
if (-not $ReleaseDir) { $ReleaseDir = "$root\dist\release\v$version" }
$failures = New-Object System.Collections.Generic.List[string]
function Check([string]$name, [bool]$ok, [string]$detail = "") {
  if ($ok) { Write-Host "PASS  $name $detail" -ForegroundColor Green }
  else { Write-Host "FAIL  $name $detail" -ForegroundColor Red; $failures.Add($name) }
}
function CheckSignature([string]$label, [string]$path) {
  $sig = Get-AuthenticodeSignature $path
  $who = if ($sig.SignerCertificate) { $sig.SignerCertificate.Subject } else { "" }
  if ($RequireSignature) {
    Check "signature: $label" ($sig.Status -eq "Valid") "($($sig.Status) $who)"
    Check "timestamp: $label" ($null -ne $sig.TimeStamperCertificate) "(signature stays valid after the certificate expires)"
  } else {
    Write-Host "INFO  signature ${label}: $($sig.Status) $who"
  }
}
function IsPE([string]$path) {
  $b = [IO.File]::ReadAllBytes($path)[0..1]
  return ($b[0] -eq 0x4D -and $b[1] -eq 0x5A)
}

$required = "NexaTerm-Setup-x64.exe", "NexaTerm-Portable-x64.zip", "SHA256SUMS.txt", "RELEASE_NOTES.md", "INSTALLATION_GUIDE.md", "PORTABLE_GUIDE.md"
foreach ($f in $required) {
  $p = Join-Path $ReleaseDir $f
  Check "exists: $f" ((Test-Path $p) -and (Get-Item $p).Length -gt 0) $(if (Test-Path $p) { "($((Get-Item $p).Length) bytes)" })
}

foreach ($exe in Get-ChildItem $ReleaseDir -Filter "*.exe") {
  Check "PE executable: $($exe.Name)" (IsPE $exe.FullName)
  $vi = $exe.VersionInfo
  Check "installer version: $($exe.Name)" ($vi.ProductVersion -like "$version*") "(ProductVersion=$($vi.ProductVersion))"
  CheckSignature $exe.Name $exe.FullName
}

# Portable ZIP: integrity + layout + exe metadata
$zipPath = Join-Path $ReleaseDir "NexaTerm-Portable-x64.zip"
if (Test-Path $zipPath) {
  $tmp = Join-Path ([IO.Path]::GetTempPath()) ("nexaterm-verify-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
  try {
    $zip = [IO.Compression.ZipFile]::OpenRead($zipPath)
    $names = $zip.Entries | ForEach-Object { $_.FullName.Replace('\', '/') }
    $zip.Dispose()
    foreach ($e in "NexaTerm-Portable/NexaTerm.exe", "NexaTerm-Portable/README-PORTABLE.txt", "NexaTerm-Portable/LICENSE", "NexaTerm-Portable/THIRD_PARTY_LICENSES.md", "NexaTerm-Portable/portable-data/README.txt") {
      Check "zip contains $e" ($names -contains $e)
    }
    $hasLoader = $names -contains "NexaTerm-Portable/WebView2Loader.dll"
    Write-Host "INFO  WebView2Loader.dll in zip: $hasLoader (required for GNU-toolchain builds)"
    [IO.Compression.ZipFile]::ExtractToDirectory($zipPath, $tmp)   # throws on CRC/corruption
    Check "zip extracts cleanly" $true
    $exe = Join-Path $tmp "NexaTerm-Portable\NexaTerm.exe"
    Check "portable NexaTerm.exe is a PE file" (IsPE $exe) "($((Get-Item $exe).Length) bytes)"
    $vi = (Get-Item $exe).VersionInfo
    Check "portable exe version" ($vi.ProductVersion -like "$version*" -or $vi.FileVersion -like "$version*") "(FileVersion=$($vi.FileVersion), Product=$($vi.ProductName))"
    $leaked = $names | Where-Object { $_ -match "node_modules|\.rs$|\.tsx?$|/target/|\.db$|\.pdb$" }
    Check "zip has no source/dev/data files" (-not $leaked) ($leaked -join ", ")
    CheckSignature "portable NexaTerm.exe" $exe
    if ($RunSelfTest) {
      $p = Start-Process -FilePath $exe -ArgumentList "--self-test" -RedirectStandardOutput "$tmp\selftest.txt" -PassThru -Wait -WindowStyle Hidden
      Get-Content "$tmp\selftest.txt" | ForEach-Object { Write-Host "      $_" }
      Check "portable self-test" ($p.ExitCode -eq 0) "(exit $($p.ExitCode))"
    }
  } catch {
    Check "zip integrity" $false $_.Exception.Message
  } finally {
    Start-Sleep -Milliseconds 300
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  }
}

# Checksums
$sums = Join-Path $ReleaseDir "SHA256SUMS.txt"
if (Test-Path $sums) {
  foreach ($line in Get-Content $sums) {
    if ($line -match '^([0-9a-f]{64}) \*(.+)$') {
      $f = Join-Path $ReleaseDir $Matches[2]
      $ok = (Test-Path $f) -and ((Get-FileHash $f -Algorithm SHA256).Hash.ToLower() -eq $Matches[1])
      Check "sha256 $($Matches[2])" $ok
    } else { Check "checksum line format" $false $line }
  }
}

Write-Host ""
if ($failures.Count) { Write-Host "VERIFY FAILED: $($failures.Count) check(s)" -ForegroundColor Red; exit 1 }
Write-Host "VERIFY PASSED" -ForegroundColor Green
exit 0
