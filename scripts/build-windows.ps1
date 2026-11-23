<#
.SYNOPSIS
  Builds the NexaTerm Windows release: offline NSIS installer, optional web installer,
  portable ZIP, SHA-256 checksums and release documents in dist\release\v<version>\.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
  powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -SkipTests -NoWebInstaller
#>
param(
  [switch]$SkipTests,
  [switch]$NoWebInstaller,
  [switch]$SkipNpmInstall,
  # Parallel rustc jobs. 1 keeps peak memory low enough for 8 GB machines with a small page file.
  [int]$Jobs = 1
)
$ErrorActionPreference = "Stop"
$sw = [Diagnostics.Stopwatch]::StartNew()

# Normalise the drive letter to upper case (Vitest misbehaves with "d:\").
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$root = $root.Substring(0, 1).ToUpper() + $root.Substring(1)
Set-Location $root
if (Test-Path "$PSScriptRoot\devenv.ps1") { . "$PSScriptRoot\devenv.ps1" }

function Step([string]$name, [scriptblock]$action) {
  Write-Host ""
  Write-Host "==> $name" -ForegroundColor Cyan
  & $action
  if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "Step '$name' failed (exit code $LASTEXITCODE)" }
}
function Need([string]$cmd, [string]$hint) {
  $c = Get-Command $cmd -ErrorAction SilentlyContinue
  if (-not $c) { throw "Missing prerequisite '$cmd'. $hint" }
  return $c.Source
}

$version = (Get-Content "$root\package.json" -Raw | ConvertFrom-Json).version
$tauriVersion = (Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version
$cargoVersion = (Select-String -Path "$root\src-tauri\Cargo.toml" -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
if ($version -ne $tauriVersion -or $version -ne $cargoVersion) {
  throw "Version mismatch: package.json=$version tauri.conf.json=$tauriVersion Cargo.toml=$cargoVersion"
}
$out = "$root\dist\release\v$version"
$env:CARGO_BUILD_JOBS = "$Jobs"

Step "Check prerequisites" {
  $node = Need "node" "Install Node.js LTS from https://nodejs.org"
  Need "npm.cmd" "Comes with Node.js" | Out-Null
  Need "cargo" "Install Rust (rustup or standalone toolchain), see docs/DEVELOPER.md" | Out-Null
  $host_triple = ((rustc -vV) | Select-String "^host: (.+)").Matches[0].Groups[1].Value
  if ($host_triple -like "*-gnu") {
    Need "gcc" "Install MinGW-w64 GCC (WinLibs), see docs/DEVELOPER.md" | Out-Null
    Need "windres" "windres ships with MinGW-w64 binutils" | Out-Null
  } else {
    Need "cl" "Install Visual Studio Build Tools (Desktop development with C++)" | Out-Null
  }
  Write-Host "node $(node -v) | npm $(npm.cmd -v) | $(rustc -V) | host $host_triple"
  $wv = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
  Write-Host ("WebView2 runtime on build machine: " + $(if ($wv) { $wv.pv } else { "not found (only needed to run the app)" }))
  Write-Host "NSIS: downloaded and verified automatically by the Tauri CLI on first build."
  $global:LASTEXITCODE = 0
}

if (-not $SkipNpmInstall) { Step "Install frontend dependencies (npm ci)" { npm.cmd ci --no-audit --no-fund } }
Step "Frontend type check" { npx.cmd tsc --noEmit }
if (-not $SkipTests) {
  Step "Frontend unit tests" { npx.cmd vitest run }
  Step "Rust tests (unit + SSH/SFTP integration)" { cargo test --manifest-path src-tauri\Cargo.toml }
}

Step "Build NSIS installer (offline WebView2 runtime embedded)" { npx.cmd tauri build --bundles nsis }
$bundle = "$root\src-tauri\target\release\bundle\nsis\NexaTerm_${version}_x64-setup.exe"
if (-not (Test-Path $bundle)) { throw "Installer not produced: $bundle" }

if (Test-Path $out) { Remove-Item -Recurse -Force $out }
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $bundle "$out\NexaTerm-Setup-x64.exe"

if (-not $NoWebInstaller) {
  Step "Build small web installer (downloads WebView2 only if missing)" {
    npx.cmd tauri build --bundles nsis --config src-tauri\tauri.web-installer.conf.json
  }
  Copy-Item $bundle "$out\NexaTerm-Setup-x64-web.exe"
}

Step "Assemble portable package" {
  $rel = "$root\src-tauri\target\release"
  $stage = "$root\build\portable\NexaTerm-Portable"
  if (Test-Path "$root\build\portable") { Remove-Item -Recurse -Force "$root\build\portable" }
  New-Item -ItemType Directory -Force "$stage\portable-data" | Out-Null
  Copy-Item "$rel\nexaterm.exe" "$stage\NexaTerm.exe"
  # GNU builds load WebView2Loader.dll dynamically; Windows only finds it next to the exe.
  if (Test-Path "$rel\WebView2Loader.dll") { Copy-Item "$rel\WebView2Loader.dll" $stage }
  Copy-Item "$root\packaging\README-PORTABLE.txt", "$root\LICENSE", "$root\THIRD_PARTY_LICENSES.md" $stage
  Copy-Item "$root\packaging\portable-data-README.txt" "$stage\portable-data\README.txt"
  $zip = "$out\NexaTerm-Portable-x64.zip"
  Compress-Archive -Path "$root\build\portable\NexaTerm-Portable" -DestinationPath $zip -CompressionLevel Optimal
}

Step "Release documents" {
  foreach ($d in "RELEASE_NOTES.md", "INSTALLATION_GUIDE.md", "PORTABLE_GUIDE.md") {
    (Get-Content "$root\packaging\$d" -Raw).Replace("{{VERSION}}", $version) | Set-Content -Encoding utf8 "$out\$d"
  }
}

Step "SHA-256 checksums" {
  $lines = Get-ChildItem $out -File | Where-Object { $_.Name -ne "SHA256SUMS.txt" } | Sort-Object Name | ForEach-Object {
    "{0} *{1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
  }
  [IO.File]::WriteAllLines("$out\SHA256SUMS.txt", $lines)
}

Step "Verify release" { powershell -NoProfile -ExecutionPolicy Bypass -File "$PSScriptRoot\verify-release.ps1" -ReleaseDir $out }

Write-Host ""
Write-Host "Release v$version built in $([int]$sw.Elapsed.TotalMinutes) min:" -ForegroundColor Green
Get-ChildItem $out -File | Sort-Object Name | ForEach-Object { "{0,-34} {1,12:N0} bytes" -f $_.Name, $_.Length }
Write-Host $out
