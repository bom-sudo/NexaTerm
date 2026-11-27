<#
.SYNOPSIS
  Authenticode-signs one file with signtool.exe. Called by the Tauri bundler (bundle.windows.signCommand)
  for the app exe, the NSIS uninstaller and the installer, and by build-windows.ps1.

  The certificate comes from environment variables, so no secret ever lands in a config file:
    NEXATERM_SIGN_THUMBPRINT     SHA-1 thumbprint of a code signing certificate in Cert:\CurrentUser\My
                                 or Cert:\LocalMachine\My (also works for USB tokens / HSM-backed certs)
    NEXATERM_SIGN_PFX            path to a .pfx file (alternative to the thumbprint)
    NEXATERM_SIGN_PFX_PASSWORD   password of the .pfx
    NEXATERM_SIGN_TIMESTAMP_URL  RFC 3161 timestamp server (default http://timestamp.digicert.com)
    NEXATERM_SIGNTOOL            full path to signtool.exe (otherwise PATH, then the Windows SDK)

.EXAMPLE
  $env:NEXATERM_SIGN_THUMBPRINT = "0123...ABCD"
  powershell -ExecutionPolicy Bypass -File scripts\sign-file.ps1 dist\release\v1.0.0\NexaTerm-Setup-x64.exe
#>
param([Parameter(Mandatory = $true)][string]$Path)
$ErrorActionPreference = "Stop"

function Find-SignTool {
  if ($env:NEXATERM_SIGNTOOL -and (Test-Path $env:NEXATERM_SIGNTOOL)) { return $env:NEXATERM_SIGNTOOL }
  $c = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($c) { return $c.Source }
  $kits = @("${env:ProgramFiles(x86)}\Windows Kits\10\bin", "$env:ProgramFiles\Windows Kits\10\bin")
  foreach ($k in $kits) {
    if (-not (Test-Path $k)) { continue }
    $found = Get-ChildItem $k -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
      Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
    if ($found) { return $found.FullName }
  }
  throw "signtool.exe not found. Install the Windows SDK 'Signing Tools for Desktop Apps' (https://developer.microsoft.com/windows/downloads/windows-sdk/) or set NEXATERM_SIGNTOOL."
}

if (-not (Test-Path $Path)) { throw "File to sign not found: $Path" }
$signtool = Find-SignTool
$ts = if ($env:NEXATERM_SIGN_TIMESTAMP_URL) { $env:NEXATERM_SIGN_TIMESTAMP_URL } else { "http://timestamp.digicert.com" }
$common = @("sign", "/fd", "sha256", "/tr", $ts, "/td", "sha256", "/d", "NexaTerm", "/du", "https://github.com/bom-sudo/NexaTerm")

if ($env:NEXATERM_SIGN_THUMBPRINT) {
  $thumb = ($env:NEXATERM_SIGN_THUMBPRINT -replace '[^0-9A-Fa-f]', '').ToUpper()
  $cert = Get-ChildItem Cert:\CurrentUser\My, Cert:\LocalMachine\My -ErrorAction SilentlyContinue | Where-Object { $_.Thumbprint -eq $thumb } | Select-Object -First 1
  if (-not $cert) { throw "Certificate $thumb not found in Cert:\CurrentUser\My or Cert:\LocalMachine\My" }
  if (-not ($cert.EnhancedKeyUsageList | Where-Object { $_.ObjectId -eq "1.3.6.1.5.5.7.3.3" })) { throw "Certificate $thumb is not a code signing certificate" }
  $args2 = $common + @("/sha1", $thumb)
  if ($cert.PSParentPath -like "*LocalMachine*") { $args2 += "/sm" }
} elseif ($env:NEXATERM_SIGN_PFX) {
  if (-not (Test-Path $env:NEXATERM_SIGN_PFX)) { throw "PFX not found: $env:NEXATERM_SIGN_PFX" }
  $args2 = $common + @("/f", $env:NEXATERM_SIGN_PFX)
  if ($env:NEXATERM_SIGN_PFX_PASSWORD) { $args2 += @("/p", $env:NEXATERM_SIGN_PFX_PASSWORD) }
} else {
  throw "No signing certificate configured: set NEXATERM_SIGN_THUMBPRINT or NEXATERM_SIGN_PFX."
}

# Timestamp servers are occasionally unavailable; retry a few times.
for ($i = 1; $i -le 3; $i++) {
  & $signtool @args2 $Path
  if ($LASTEXITCODE -eq 0) { break }
  if ($i -eq 3) { throw "signtool failed for $Path (exit $LASTEXITCODE)" }
  Start-Sleep -Seconds (5 * $i)
}
$sig = Get-AuthenticodeSignature $Path
if ($sig.Status -ne "Valid") { throw "Signature check failed for ${Path}: $($sig.Status) $($sig.StatusMessage)" }
Write-Host "signed: $Path ($($sig.SignerCertificate.Subject))"
exit 0
