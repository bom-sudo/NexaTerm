# Installing NexaTerm

## System requirements

* Windows 10 (1809+) or Windows 11, x64
* Microsoft Edge **WebView2 Runtime** — built into Windows 11 and kept current on Windows 10 by
  Windows Update. If it is missing, the installer downloads Microsoft's bootstrapper.
* No administrator rights are needed for either edition.
* Optional: Windows OpenSSH Authentication Agent (for agent auth), Git for Windows (Git Bash), WSL.

## Installer edition

Run `NexaTerm-Setup-x64.exe` (offline) or `NexaTerm-Setup-x64-web.exe`. It installs for the current user under
`%LOCALAPPDATA%\NexaTerm` by default (changeable in the wizard), adds a Start-menu entry and an uninstaller. User data lives in
`%APPDATA%\NexaTerm` and is kept on uninstall.

## Portable edition

Unzip `NexaTerm-Portable-x64.zip` to any folder (e.g. a USB drive) and start
`NexaTerm.exe` inside `NexaTerm-Portable\`. The `portable-data\` folder next to the exe makes NexaTerm
keep its database, settings and logs there (falls back to `%LOCALAPPDATA%\NexaTerm\portable-fallback`
with a warning if the folder is read-only).
Keep `WebView2Loader.dll` next to the exe.

Note: saved passwords are stored in the Windows Credential Manager of the user running NexaTerm,
not on the USB drive. On another PC you will be asked for them again.

## Upgrading / uninstalling

Install the new version over the old one; the database is migrated automatically. Back up first
via Settings → Data → *Back up*. To remove saved passwords, delete the `NexaTerm/…` entries in
Control Panel → Credential Manager → Windows Credentials.
