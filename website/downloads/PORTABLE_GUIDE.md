# NexaTerm 1.0.0 โ€” Portable Guide

## Use

1. Extract `NexaTerm-Portable-x64.zip` to any writable folder (USB drive, `D:\Tools`, a folder with
   spaces in its name โ€” all fine).
2. Open `NexaTerm-Portable\` and double-click **NexaTerm.exe**.

No installation, no administrator rights, no registry entries written by NexaTerm, no online activation.

## Folder layout

```
NexaTerm-Portable/
โ”โ”€โ”€ NexaTerm.exe            application (UI assets are embedded in the exe)
โ”โ”€โ”€ WebView2Loader.dll      Microsoft WebView2 loader โ€” must stay next to the exe
โ”โ”€โ”€ portable-data/          database, settings, logs, backups (created/filled on first run)
โ”โ”€โ”€ README-PORTABLE.txt
โ”โ”€โ”€ LICENSE
โ””โ”€โ”€ THIRD_PARTY_LICENSES.md
```

There are no separate `resources/` or `libraries/` folders: the web UI is compiled into
`NexaTerm.exe`, and the loader DLL is the only library that has to ship beside it, because Windows
only loads it from the executable's folder.

This is a multi-file portable package, not a single self-contained executable.

## Prerequisite: WebView2 Runtime

NexaTerm renders its UI with Microsoft Edge WebView2, which is part of Windows 11 and of current,
updated Windows 10. It is not redistributable inside a portable folder in a useful way. If NexaTerm
does not start, install Microsoft's *Evergreen Standalone Installer*, or use `NexaTerm-Setup-x64.exe`,
which includes it. `NexaTerm.exe --self-test` reports whether it is present.

## Passwords and portability

* `portable-data` never contains passwords.
* Passwords and key passphrases you choose to save go to the **Windows Credential Manager of the
  current Windows user**, protected by DPAPI. They do **not** move with the folder: on another PC or
  Windows account NexaTerm asks for them again. Sessions, folders, known hosts, snippets and settings
  do move with the folder.

## Read-only locations

If `portable-data` cannot be written, NexaTerm uses `%LOCALAPPDATA%\NexaTerm\portable-fallback` for that
session and shows a warning in the app. Nothing is lost silently.

## Moving, backing up, updating

* Close NexaTerm, then move/copy the whole folder.
* Back up = copy `portable-data`. In-app: Settings โ’ Data โ’ Back up.
* Update: extract the new version and copy your old `portable-data` folder into it.

## Self-test

```powershell
.\NexaTerm.exe --self-test > report.txt
```
Checks data folder, database integrity, persistence, session CRUD, Credential Manager, shell
detection, ConPTY terminal, logging and WebView2. Exit code 0 = all passed.

