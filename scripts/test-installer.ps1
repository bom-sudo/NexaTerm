<#
.SYNOPSIS
  Real installer test on this machine (per-user, no admin): wizard install, shortcuts, launch,
  self-test, uninstall (data retention), silent reinstall to a custom path with spaces, uninstall.
  Leaves the machine without NexaTerm installed; %APPDATA%\NexaTerm user data is kept (by design).
#>
param(
  [Parameter(Mandatory)] [string]$Setup,
  [string]$Shots = "$env:TEMP\nexaterm-test-shots",
  [string]$AltDir = "D:\devtools\tmp\NexaTerm Install Test",
  # Wizard install folder (the offline installer needs ~290 MB free on the target drive).
  [string]$GuiDir = "D:\devtools\tmp\NexaTerm GUI Install",
  # NSIS extracts solid-compressed data via %TEMP%; the offline installer needs ~300 MB there.
  [string]$TempDir
)
if ($TempDir) { New-Item -ItemType Directory -Force $TempDir | Out-Null; $env:TEMP = $TempDir; $env:TMP = $TempDir }
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\testlib.ps1"
New-Item -ItemType Directory -Force $Shots | Out-Null
$results = New-Object System.Collections.Generic.List[object]
function Result([string]$test, [bool]$ok, [string]$detail) {
  $results.Add([pscustomobject]@{ Test = $test; Result = $(if ($ok) { "PASS" } else { "FAIL" }); Detail = $detail })
  Write-Host ("{0}  {1}  {2}" -f $(if ($ok) { "PASS" } else { "FAIL" }), $test, $detail)
}
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\NexaTerm"
$startMenu = Join-Path ([Environment]::GetFolderPath("Programs")) "NexaTerm"
$desktopLnk = Join-Path ([Environment]::GetFolderPath("Desktop")) "NexaTerm.lnk"
$defaultDir = $GuiDir

function Stop-NexaTerm { Get-Process nexaterm -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1 }
function Invoke-Uninstall([string]$dir) {
  $u = Join-Path $dir "uninstall.exe"
  $p = Start-Process $u -ArgumentList "/S" -PassThru -Wait
  $deadline = (Get-Date).AddSeconds(90)   # NSIS uninstaller re-launches itself from %TEMP%
  while ((Test-Path $u) -and (Get-Date) -lt $deadline) { Start-Sleep 1 }
  Start-Sleep 2
  return -not (Test-Path $u)
}
function SelfTest([string]$exe, [string]$tag) {
  $out = Join-Path $Shots "selftest-$tag.txt"
  $p = Start-Process $exe -ArgumentList "--self-test" -RedirectStandardOutput $out -PassThru -Wait -WindowStyle Hidden
  $text = Get-Content $out -Raw
  Result "self-test ($tag)" ($p.ExitCode -eq 0) (($text -split "`n" | Where-Object { $_ -match "^(PASS|FAIL)  (data location|WebView2|ConPTY|persistence)" }) -join " | ").Trim()
}

Stop-NexaTerm
if (Test-Path "$defaultDir\uninstall.exe") { Invoke-Uninstall $defaultDir | Out-Null }
$appDataBefore = Test-Path "$env:APPDATA\NexaTerm\nexaterm.db"

# ---------------------------------------------------------------- 1-3 GUI wizard install
$setupProc = Start-Process $Setup -PassThru
$w = Wait-ProcWindow @($setupProc.Id) "*" 30
Result "installer launches" ($null -ne $w) $(if ($w) { "window: '$($w.Title)'" } else { "no window" })
if ($w) { Save-WindowImage $w.Handle "$Shots\01-installer-first-page.png" }
# Language dialog -> Welcome -> Directory -> Start menu -> Install -> Finish
$pages = @()
$step = 0
$dirSet = $false
$deadline = (Get-Date).AddMinutes(4)
while (-not $setupProc.HasExited -and (Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 1500
  Show-ProcWindows $setupProc.Id
  $w = Get-ProcWindows @($setupProc.Id) | Select-Object -First 1
  if (-not $w) { continue }
  if ($w.Title -like "*Setup*" -and -not $dirSet -and (Set-DialogEdit $setupProc.Id $GuiDir)) { $dirSet = $true; $pages += "destination set to $GuiDir"; Start-Sleep 1 }
  # UI Automation presses the wizard's own default button; nothing is typed anywhere.
  $before = "$Shots\installer-step-{0:d2}.png" -f $step
  Save-WindowImage $w.Handle $before
  $pressed = Invoke-DialogButton $setupProc.Id "1"
  if ($pressed) { $pages += $pressed; $step++ }   # disabled while files are being installed -> retry
}
$setupProc.WaitForExit(30000) | Out-Null
Result "installer wizard completes" ($setupProc.HasExited -and $setupProc.ExitCode -eq 0) "exit $($setupProc.ExitCode); pages: $($pages -join ' > ')"

# ---------------------------------------------------------------- 4 shortcuts, files, registry
$exe = Join-Path $defaultDir "nexaterm.exe"
Result "files installed" ((Test-Path $exe) -and (Test-Path "$defaultDir\uninstall.exe")) $defaultDir
$lnk = Get-ChildItem $startMenu -Filter "*.lnk" -ErrorAction SilentlyContinue | Select-Object -First 1
Result "Start Menu shortcut" ($null -ne $lnk) $(if ($lnk) { $lnk.FullName } else { "missing in $startMenu" })
$reg = Get-ItemProperty $uninstKey -ErrorAction SilentlyContinue
Result "uninstall registry entry" ($null -ne $reg) $(if ($reg) { "DisplayName=$($reg.DisplayName) Version=$($reg.DisplayVersion) Publisher=$($reg.Publisher)" })
Result "exe version metadata" ((Get-Item $exe).VersionInfo.ProductVersion -like "1.0.0*") ("Product='{0}' Version={1} Company='{2}'" -f (Get-Item $exe).VersionInfo.ProductName, (Get-Item $exe).VersionInfo.ProductVersion, (Get-Item $exe).VersionInfo.CompanyName)

# ---------------------------------------------------------------- 5-6 launch from shortcut
Stop-NexaTerm   # finish page may have started it already
if ($lnk) {
  Start-Process $lnk.FullName
  Start-Sleep 2
  $procs = @(Get-Process nexaterm -ErrorAction SilentlyContinue | ForEach-Object Id)
  $mw = if ($procs.Count) { Wait-ProcWindow $procs "NexaTerm" 30 } else { $null }
  Result "app opens from Start Menu shortcut" ($null -ne $mw) $(if ($mw) { "window '$($mw.Title)', pid $($mw.Pid)" } else { "no window" })
  if ($mw) {
    Start-Sleep 4
    Save-WindowImage $mw.Handle "$Shots\10-installed-app.png"
  }
  Start-Sleep 2
  Result "app still running after 10 s" ($null -ne (Get-Process nexaterm -ErrorAction SilentlyContinue)) ""
  Stop-NexaTerm
}

# ---------------------------------------------------------------- 7-9 core functions in installed build
SelfTest $exe "installed-1"
SelfTest $exe "installed-2"   # second run proves persistence of the DB marker

# ---------------------------------------------------------------- 12-14 uninstall + retention
$ok = Invoke-Uninstall $defaultDir
Result "silent uninstall" $ok ""
Result "program files removed" (-not (Test-Path $exe)) $defaultDir
Result "Start Menu shortcut removed" (-not (Test-Path $startMenu) -or -not (Get-ChildItem $startMenu -Filter *.lnk)) ""
Result "uninstall registry entry removed" (-not (Test-Path $uninstKey)) ""
Result "user data retained (documented policy)" (Test-Path "$env:APPDATA\NexaTerm\nexaterm.db") "$env:APPDATA\NexaTerm (existed before test: $appDataBefore)"

# ---------------------------------------------------------------- 15 silent reinstall to custom path with spaces
if (Test-Path $AltDir) { Remove-Item -Recurse -Force $AltDir }
$p = Start-Process $Setup -ArgumentList "/S", "/D=$AltDir" -PassThru -Wait
$altExe = Join-Path $AltDir "nexaterm.exe"
Result "silent reinstall to path with spaces" ($p.ExitCode -eq 0 -and (Test-Path $altExe)) "exit $($p.ExitCode); $AltDir"
if (Test-Path $altExe) {
  SelfTest $altExe "reinstalled"
  Result "sessions survive reinstall" ((Get-Content (Join-Path $Shots "selftest-reinstalled.txt") -Raw) -match "previous run found") "persistence marker from earlier run found"
  Result "uninstall after reinstall" (Invoke-Uninstall $AltDir) ""
}
if (Test-Path $desktopLnk) { Write-Host "INFO desktop shortcut present: $desktopLnk" }

$results | ConvertTo-Json | Set-Content -Encoding utf8 "$Shots\installer-results.json"
$fail = @($results | Where-Object Result -eq "FAIL").Count
Write-Host ""
Write-Host "INSTALLER TESTS: $($results.Count - $fail) passed, $fail failed. Evidence: $Shots"
exit $fail
