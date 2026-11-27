<#
.SYNOPSIS
  Regenerates the README screenshots in docs\images\ from the real application (debug build) against
  the bundled mock SSH/SFTP server, using a throw-away data folder. Captures only the NexaTerm window
  (PrintWindow), never the screen. Cleans up its data folder and the demo credentials afterwards.
#>
param(
  [string]$Out = (Join-Path (Split-Path -Parent $PSScriptRoot) "docs\images"),
  [string]$Work = "$env:TEMP\nexaterm-showcase",
  [int]$Port = 2299,
  [string[]]$Scenes = @("workspace", "sftp", "multiexec", "tunnels", "keys", "hostkey", "light", "snippets")
)
$ErrorActionPreference = "Stop"
# Allow -Scenes "a,b" when called through powershell -File (arrives as one string).
$Scenes = @($Scenes | ForEach-Object { $_ -split "," } | Where-Object { $_ })
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
. "$PSScriptRoot\testlib.ps1"
if (Test-Path "$PSScriptRoot\devenv.ps1") { . "$PSScriptRoot\devenv.ps1" }
Set-Location $root
New-Item -ItemType Directory -Force $Out | Out-Null

Write-Host "Building debug app and examples..."
cargo build --manifest-path src-tauri\Cargo.toml --bin nexaterm --example mock_sshd --example seed_showcase
if ($LASTEXITCODE) { throw "cargo build failed" }
$dbg = "$root\src-tauri\target\debug"
# GNU builds: examples that link the app library need WebView2Loader.dll beside them.
if (Test-Path "$dbg\WebView2Loader.dll") { Copy-Item "$dbg\WebView2Loader.dll" "$dbg\examples\" -Force }

if (Test-Path $Work) { Remove-Item -Recurse -Force $Work }
New-Item -ItemType Directory -Force "$Work\sftp" | Out-Null
$data = "$Work\data"
$started = @()
try {
  $mock = Start-Process "$dbg\examples\mock_sshd.exe" -ArgumentList $Port, "`"$Work\sftp`"", "`"$Work\hostkey`"" -PassThru -NoNewWindow -RedirectStandardOutput "$Work\mock.log"
  $started += $mock.Id
  Start-Sleep 2
  & "$dbg\examples\seed_showcase.exe" $data $Port "$Work\hostkey" "$Work\sftp"
  if ($LASTEXITCODE) { throw "seeding failed" }

  $vite = Start-Process "cmd.exe" -ArgumentList "/c", "npx vite --port 1420 --strictPort > `"$Work\vite.log`" 2>&1" -PassThru -WindowStyle Hidden
  $started += $vite.Id
  $deadline = (Get-Date).AddSeconds(60)
  while (-not (Test-NetConnection 127.0.0.1 -Port 1420 -InformationLevel Quiet -WarningAction SilentlyContinue)) {
    if ((Get-Date) -gt $deadline) { throw "vite dev server did not start" }
    Start-Sleep 1
  }

  $wait = @{ workspace = 10; light = 10; sftp = 9; multiexec = 12; tunnels = 9; keys = 6; hostkey = 7; snippets = 8 }
  foreach ($scene in $Scenes) {
    $env:NEXATERM_DATA_DIR = $data
    $env:NEXATERM_SHOWCASE = $scene
    $env:NEXATERM_REPO = $root
    $app = Start-Process "$dbg\nexaterm.exe" -PassThru -NoNewWindow -RedirectStandardOutput "$Work\app-$scene.log" -RedirectStandardError "$Work\app-$scene.err"
    $w = Wait-ProcWindow @($app.Id) "NexaTerm" 60
    if (-not $w) { throw "no window for scene $scene" }
    Start-Sleep -Seconds $(if ($wait.ContainsKey($scene)) { $wait[$scene] } else { 10 })
    Save-WindowImage $w.Handle (Join-Path $Out "$scene.png")
    Write-Host "captured $scene"
    $app.CloseMainWindow() | Out-Null
    if (-not $app.WaitForExit(15000)) { $app.Kill() }
    Start-Sleep 1
  }
} finally {
  Remove-Item Env:NEXATERM_DATA_DIR, Env:NEXATERM_SHOWCASE, Env:NEXATERM_REPO -ErrorAction SilentlyContinue
  foreach ($procId in $started) {
    # stop the process tree we started (cmd -> node for vite)
    Get-CimInstance Win32_Process | Where-Object { $_.ParentProcessId -eq $procId } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
  }
  if (Test-Path "$data\nexaterm.db") { & "$dbg\examples\seed_showcase.exe" --cleanup $data }
  Start-Sleep 1
  Remove-Item -Recurse -Force $Work -ErrorAction SilentlyContinue
}
Get-ChildItem $Out -Filter *.png | Select-Object Name, Length
