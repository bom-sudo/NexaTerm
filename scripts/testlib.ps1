# Helpers for release testing. Captures ONLY a given process's window (PrintWindow), never the
# screen, and sends keystrokes only when that process owns the foreground window.
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public class NxWin {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  public struct RECT { public int L, T, R, B; }
}
"@

function Get-ProcWindows([int[]]$ProcessIds) {
  $list = New-Object System.Collections.Generic.List[object]
  $cb = [NxWin+EnumProc] {
    param($h, $l)
    $procId = [uint32]0
    [NxWin]::GetWindowThreadProcessId($h, [ref]$procId) | Out-Null
    if ($ProcessIds -contains [int]$procId -and [NxWin]::IsWindowVisible($h)) {
      $sb = New-Object System.Text.StringBuilder 256
      [NxWin]::GetWindowText($h, $sb, 256) | Out-Null
      $list.Add([pscustomobject]@{ Handle = $h; Pid = [int]$procId; Title = $sb.ToString() })
    }
    return $true
  }
  [NxWin]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null
  return $list
}

function Wait-ProcWindow([int[]]$ProcessIds, [string]$TitleLike = "*", [int]$TimeoutSec = 30) {
  $deadline = (Get-Date).AddSeconds($TimeoutSec)
  while ((Get-Date) -lt $deadline) {
    $w = Get-ProcWindows $ProcessIds | Where-Object { $_.Title -like $TitleLike } | Select-Object -First 1
    if ($w) { return $w }
    Start-Sleep -Milliseconds 300
  }
  return $null
}

# Window-only capture (PW_RENDERFULLCONTENT = 2 works for WebView2 content).
function Save-WindowImage([IntPtr]$Handle, [string]$Path) {
  $r = New-Object NxWin+RECT
  [NxWin]::GetWindowRect($Handle, [ref]$r) | Out-Null
  $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc()
  [NxWin]::PrintWindow($Handle, $hdc, 2) | Out-Null
  $g.ReleaseHdc($hdc); $g.Dispose()
  $bmp.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
}

# Press a dialog button of a specific process via UI Automation (no focus, no keystrokes).
# NSIS wizard: control ID 1 = OK / Next / Install / Finish, 2 = Cancel, 3 = Back.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
function Invoke-DialogButton([int]$ProcessId, [string]$AutomationId = "1") {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ProcessIdProperty), $ProcessId
  $wins = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
  foreach ($w in $wins) {
    $bc = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::AutomationIdProperty), $AutomationId
    $btn = $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $bc)
    if ($btn -and $btn.Current.IsEnabled) {
      try {
        $name = $btn.Current.Name
        $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        return "$($w.Current.Name): pressed '$name'"
      } catch {
        return $null   # page was changing while we pressed; caller retries
      }
    }
  }
  return $null
}

# Set the first edit box of the process's dialog (e.g. NSIS destination folder) via UI Automation.
function Set-DialogEdit([int]$ProcessId, [string]$Value) {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ProcessIdProperty), $ProcessId
  foreach ($w in $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)) {
    $ec = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::Edit)
    $edit = $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $ec)
    if ($edit) { $edit.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Value); return $true }
  }
  return $false
}

# Un-minimize all windows of a process without activating them (SW_SHOWNOACTIVATE).
function Show-ProcWindows([int]$ProcessId) {
  Get-ProcWindows @($ProcessId) | ForEach-Object { [NxWin]::ShowWindow($_.Handle, 4) | Out-Null }
}

# Send keys only if the target window is really in the foreground; otherwise throw (never type
# into another application).
function Send-KeysGuarded([IntPtr]$Handle, [int[]]$ProcessIds, [string]$Keys) {
  [NxWin]::ShowWindow($Handle, 9) | Out-Null
  [NxWin]::SetForegroundWindow($Handle) | Out-Null
  Start-Sleep -Milliseconds 400
  $fg = [NxWin]::GetForegroundWindow()
  $procId = [uint32]0
  [NxWin]::GetWindowThreadProcessId($fg, [ref]$procId) | Out-Null
  if (-not ($ProcessIds -contains [int]$procId)) { throw "Foreground window is not the test target (pid $procId); refusing to send keys." }
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
}
