# NexaTerm 1.0.0 โ€” Installation Guide

## Which file do I need?

| File | Use it when |
|---|---|
| `NexaTerm-Setup-x64.exe` | **Recommended.** Normal installation. Contains the Microsoft WebView2 Runtime offline installer, so it works without an internet connection. |
| `NexaTerm-Setup-x64-web.exe` | Smaller installer. Downloads the WebView2 Runtime from Microsoft only if the computer does not have it. |
| `NexaTerm-Portable-x64.zip` | No installation (USB drive, locked-down PC). See PORTABLE_GUIDE.md. |

Check the download against `SHA256SUMS.txt`:

```powershell
Get-FileHash .\NexaTerm-Setup-x64.exe -Algorithm SHA256
```

## Requirements

* Windows 10 (version 1809 or later) or Windows 11, 64-bit
* No administrator rights (per-user installation)
* About 30 MB disk space for NexaTerm (plus the WebView2 Runtime if it is not already present)
* **Offline installer only:** it carries the 294 MB WebView2 Runtime installer, so it needs about
  **300 MB free in `%TEMP%`** (normally on drive C:) while unpacking, and reports ~290 MB "space
  required" on the destination drive even if WebView2 is already installed. If the system drive is
  nearly full, install to another drive *and* free some space on C:, or simply use
  `NexaTerm-Setup-x64-web.exe`, which needs only a few MB.

## Install

1. Double-click `NexaTerm-Setup-x64.exe`.
2. Choose the installer language (English / Thai).
3. Windows SmartScreen may show "Windows protected your PC" because this free release is **not
   code-signed**. Verify the SHA-256 checksum first, then choose *More info โ’ Run anyway*.
4. Choose the installation folder (default `%LOCALAPPDATA%\NexaTerm`) and the Start-menu folder.
5. On the last page, optionally tick **Create desktop shortcut**, then *Finish* (optionally run NexaTerm).

Silent install (IT deployment): `NexaTerm-Setup-x64.exe /S`

## What the installer changes

* Program files in the chosen folder, Start-menu shortcut, optional desktop shortcut.
* An uninstall entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NexaTerm`.
* The WebView2 Runtime, only if it was missing.

## Your data

* Database, settings, logs: `%APPDATA%\NexaTerm\`
* Passwords and key passphrases: Windows Credential Manager (entries named `NexaTerm/โ€ฆ`)

## Uninstall

*Settings โ’ Apps โ’ NexaTerm โ’ Uninstall*, or the *Uninstall NexaTerm* shortcut. Program files and
shortcuts are removed. Your data in `%APPDATA%\NexaTerm` is **kept** so a reinstall finds your sessions
(the uninstaller offers a checkbox to delete it). Saved passwords can be removed in
*Control Panel โ’ Credential Manager โ’ Windows Credentials*.

## Troubleshooting

* `NexaTerm.exe --self-test > report.txt` (in the install folder) checks the database, Credential
  Manager, local terminal and WebView2 and writes `%APPDATA%\NexaTerm\logs\self-test.txt`.
* Application log: `%APPDATA%\NexaTerm\logs\nexaterm.log` (passwords are never logged).

