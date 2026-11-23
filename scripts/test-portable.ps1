<#
.SYNOPSIS
  Real portable-edition test: extract the release ZIP into a path with spaces, run with a minimal
  PATH (no dev tools), GUI launch + graceful close, persistence across runs, logs, moving the
  folder, and the read-only fallback. Cleans up its test folders afterwards.
#>
param(
  [Parameter(Mandatory)] [string]$Zip,
  [string]$Work = "D:\devtools\tmp\Portable Test",
  [string]$Shots = "$env:TEMP\nexaterm-test-shots"
)
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\testlib.ps1"
Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Force $Shots | Out-Null
$results = New-Object System.Collections.Generic.List[object]
function Result([string]$test, [bool]$ok, [string]$detail) {
  $results.Add([pscustomobject]@{ Test = $test; Result = $(if ($ok) { "PASS" } else { "FAIL" }); Detail = $detail })
  Write-Host ("{0}  {1}  {2}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $test, $detail)
}
# Minimal environment: only Windows directories on PATH (proves no Node/Rust/devtools needed).
$env:PATH = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"

function SelfTest([string]$exe, [string]$tag) {
  $out = Join-Path $Shots "portable-selftest-$tag.txt"
  $p = Start-Process $exe -ArgumentList "--self-test" -RedirectStandardOutput $out -PassThru -Wait -WindowStyle Hidden
  return @{ Code = $p.ExitCode; Text = (Get-Content $out -Raw) }
}
function GuiRun([string]$exe, [string]$tag) {
  $p = Start-Process $exe -PassThru
  $w = Wait-ProcWindow @($p.Id) "NexaTerm" 30
  if ($w) { Start-Sleep 5; Save-WindowImage $w.Handle "$Shots\portable-$tag.png" }
  $alive = -not $p.HasExited
  $closed = $false
  if ($alive) { $p.CloseMainWindow() | Out-Null; $closed = $p.WaitForExit(15000) }
  if (-not $p.HasExited) { $p.Kill() }
  return @{ Window = $w; Alive = $alive; Closed = $closed }
}

if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
$run1 = Join-Path $Work "Run One"
[IO.Compression.ZipFile]::ExtractToDirectory($Zip, $run1)
$root1 = Join-Path $run1 "NexaTerm-Portable"
$exe1 = Join-Path $root1 "NexaTerm.exe"
Result "extract ZIP to path with spaces" (Test-Path $exe1) $root1
Result "no development environment on PATH" (-not (Get-Command node -ErrorAction SilentlyContinue) -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) "PATH=$env:PATH"

$s1 = SelfTest $exe1 "1"
Result "self-test #1 (fresh folder)" ($s1.Code -eq 0) "exit $($s1.Code)"
Result "data stored in portable-data" ($s1.Text -match [regex]::Escape("$root1\portable-data") -and $s1.Text -match "portable=true") ""
Result "database created" (Test-Path "$root1\portable-data\nexaterm.db") ""

$g = GuiRun $exe1 "gui-1"
Result "double-click NexaTerm.exe opens the window" ($null -ne $g.Window) $(if ($g.Window) { "'$($g.Window.Title)'" })
Result "graceful close (window close button equivalent)" $g.Closed ""
$log = Get-Content "$root1\portable-data\logs\nexaterm.log" -Raw -ErrorAction SilentlyContinue
Result "application log written" ($log -match "starting; data dir" -and $log -match "shutting down") "$root1\portable-data\logs\nexaterm.log"

$s2 = SelfTest $exe1 "2"
Result "persistence after restart" ($s2.Code -eq 0 -and $s2.Text -match "previous run found") ($s2.Text -split "`n" | Where-Object { $_ -match "persistence" })
Result "database integrity" ($s2.Text -match "PASS  database open \+ integrity") ""
Result "no plaintext secrets in portable-data" (-not (Select-String -Path "$root1\portable-data\*" -Pattern "self-test-secret" -SimpleMatch -ErrorAction SilentlyContinue)) "Credential Manager test value not found on disk"

# Move the folder elsewhere.
$moved = Join-Path $Work "Moved Elsewhere\NexaTerm-Portable"
New-Item -ItemType Directory -Force (Split-Path $moved) | Out-Null
Move-Item $root1 $moved
$s3 = SelfTest (Join-Path $moved "NexaTerm.exe") "moved"
Result "works after moving folder; data travels" ($s3.Code -eq 0 -and $s3.Text -match "previous run found" -and $s3.Text -match [regex]::Escape("$moved\portable-data")) ""

# Read-only portable-data -> fallback.
$pd = Join-Path $moved "portable-data"
& icacls $pd /deny "${env:USERNAME}:(OI)(CI)(W,AD,WD)" | Out-Null
try {
  $s4 = SelfTest (Join-Path $moved "NexaTerm.exe") "readonly"
  $fallback = Join-Path $env:LOCALAPPDATA "NexaTerm\portable-fallback"
  Result "read-only folder: falls back and reports it" ($s4.Code -eq 0 -and $s4.Text -match "not writable" -and $s4.Text -match [regex]::Escape($fallback)) ($s4.Text -split "`n" | Where-Object { $_ -match "data location" })
  $g2 = GuiRun (Join-Path $moved "NexaTerm.exe") "gui-readonly"
  Result "read-only folder: app still opens" ($null -ne $g2.Window) ""
} finally {
  & icacls $pd /remove:d $env:USERNAME | Out-Null
}
$s5 = SelfTest (Join-Path $moved "NexaTerm.exe") "after-readonly"
Result "portable data intact after read-only episode" ($s5.Code -eq 0 -and $s5.Text -match "previous run found") ""

# Cleanup test folders and the fallback directory created by this test.
Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force (Join-Path $env:LOCALAPPDATA "NexaTerm\portable-fallback") -ErrorAction SilentlyContinue

$results | ConvertTo-Json | Set-Content -Encoding utf8 "$Shots\portable-results.json"
$fail = @($results | Where-Object Result -eq "FAIL").Count
Write-Host ""
Write-Host "PORTABLE TESTS: $($results.Count - $fail) passed, $fail failed. Evidence: $Shots"
exit $fail
