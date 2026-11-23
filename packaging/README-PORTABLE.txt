NexaTerm Portable (x64)
=======================

NexaTerm - Remote Administration Workstation. Free and open source (MIT).

START
  Double-click NexaTerm.exe. No installation and no administrator rights are needed.

WHAT IS IN THIS FOLDER
  NexaTerm.exe           The application. The user interface is embedded in the exe.
  WebView2Loader.dll     Microsoft WebView2 loader. Must stay next to NexaTerm.exe.
  portable-data\         Your sessions, settings, known hosts, snippets, logs and backups.
                         Its presence switches NexaTerm into portable mode.
  LICENSE, THIRD_PARTY_LICENSES.md, README-PORTABLE.txt

REQUIREMENT
  Microsoft Edge WebView2 Runtime. It is part of Windows 11 and is installed on current
  Windows 10 systems by Windows Update. If NexaTerm does not open, install the runtime from
  https://developer.microsoft.com/microsoft-edge/webview2/ (Evergreen Standalone Installer),
  or use NexaTerm-Setup-x64.exe, which bundles it.

PASSWORDS
  Saved passwords and key passphrases are NOT stored in portable-data. They are kept in the
  Windows Credential Manager of the Windows user running NexaTerm (protected by Windows/DPAPI).
  On another computer or another Windows account, NexaTerm asks for them again.
  Private key files stay wherever you keep them; NexaTerm stores only their paths.

MOVING / COPYING
  Close NexaTerm, then move or copy the whole NexaTerm-Portable folder. Sessions and settings
  travel with it.

READ-ONLY LOCATIONS
  If portable-data cannot be written (read-only share, write-protected USB drive), NexaTerm
  uses %LOCALAPPDATA%\NexaTerm\portable-fallback for that run and shows a warning.

CHECKING A COPY
  NexaTerm.exe --self-test > report.txt
  runs a headless check (database, Credential Manager, local terminal, WebView2) and writes
  the result to report.txt and portable-data\logs\self-test.txt.

PRIVACY
  No account, no telemetry, no background network traffic.
