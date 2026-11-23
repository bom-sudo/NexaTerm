# Development Status — NexaTerm 1.0.0 (2026-11-23)

Legend: ✅ built and tested · 🟡 implemented, tested by automated tests only (not clicked through) · ⬜ not implemented

## Release artifacts (`dist/release/v1.0.0/`, git-ignored — publish as a GitHub Release)

| File | Size | SHA-256 |
|---|---|---|
| `NexaTerm-Setup-x64.exe` (offline, bundles WebView2) | 301,061,042 B | `3bce9c6cd3001083f2c72dd494e8d872145139d7f0a824a00a6c565a8b24329a` |
| `NexaTerm-Setup-x64-web.exe` | 3,091,399 B | `e2e23244114b6c6233e31da9279bcbd23092f409e27d075adf2deb7bda57aa0d` |
| `NexaTerm-Portable-x64.zip` | 3,779,432 B | `bb16e7e41a644df7481b61fe86da9cc3116c4708abc643338edb793202b95276` |

Signing: **unsigned** (no certificate). Checksums are in `SHA256SUMS.txt`.

## Test results for 1.0.0 (run on the actual artifacts)

| Suite | Result | Notes |
|---|---|---|
| Frontend type check + Vitest | ✅ 18/18 | |
| Rust unit tests | ✅ 34/34 | |
| SSH/SFTP/tunnel integration (mock server) | ✅ 11/11 | |
| `verify-release.ps1` | ✅ VERIFY PASSED | PE files, versions, ZIP layout/integrity, checksums, portable self-test |
| Offline installer (`test-installer.ps1`) | ✅ 19/19 | Real wizard via UI Automation, Start-menu shortcut, registry, launch from shortcut, self-test ×2, uninstall, data retained, silent reinstall to path with spaces, uninstall. Run with `TEMP` on D: (see limitation 1). |
| Web installer (`test-installer.ps1`) | ✅ 19/19 | Same suite with default `TEMP` on the nearly full C: drive |
| Portable (`test-portable.ps1`) | ✅ 15/15 | Path with spaces, PATH without dev tools, GUI open + graceful close, persistence, logs, DB integrity, no secrets on disk, move folder, read-only fallback with in-app warning |

Environment: Windows 11 Pro (build 26300), x64, per-user without admin rights, WebView2 157 already installed.
**Not tested:** a clean Windows VM, Windows 10, a machine *without* WebView2 (the runtime-install path of
the installers), the uninstaller's "delete application data" checkbox (would delete this machine's real data),
and SSH against a real OpenSSH server (all SSH tests use the bundled mock server).

## Acceptance criteria

| Criterion | Status |
|---|---|
| Application starts on Windows | ✅ installed and portable release builds |
| Local PowerShell / CMD terminals | ✅ GUI (dev build) + ConPTY self-test in release builds |
| Multiple tabs, split terminal | ✅ GUI (dev build) |
| SSH password / private-key login | ✅ integration tests; password login also via GUI against mock server |
| Host fingerprint verification, known hosts persisted | ✅ integration + GUI |
| Session profiles create/edit, persist after restart | ✅ unit tests + release persistence tests |
| Multiple simultaneous SSH sessions | ✅ integration test |
| SFTP browse / upload / download | ✅ integration; browse + edit via GUI |
| Port forwarding | ✅ integration (local, SOCKS5); 🟡 tunnel UI |
| Credentials securely stored | ✅ Credential Manager tests, no secrets on disk |
| No mandatory internet, no paid services, no usage limits | ✅ |
| Windows installer builds | ✅ offline + web |
| Portable edition builds | ✅ |
| Documentation | ✅ |

## Feature status

| Module | Status |
|---|---|
| Terminal emulator, local shells | ✅ |
| SSH client, session manager | ✅ (import/export & folder drag-drop 🟡) |
| SFTP + remote editor | ✅ (folder transfer ⬜) |
| Tunnels, multi-execution, key manager, network tools, snippets, settings/backup | 🟡 backend tested, UI type-checked, not clicked through |
| Portable mode, read-only fallback, app log, `--self-test` | ✅ |
| Macros, RDP, VNC, X11, Telnet, Serial | ⬜ (hidden in the UI) |

## Known limitations

1. The **offline installer** needs ~300 MB free in `%TEMP%` (NSIS unpacks its solid archive there) and asks for
   ~290 MB on the destination drive even when WebView2 is already present. On this build machine (C: almost
   full) it aborts with default `TEMP`; the web installer works. Documented in INSTALLATION_GUIDE.md.
2. Unsigned executables → SmartScreen warning.
3. Portable edition needs the WebView2 Runtime already installed (standard on Windows 11 / updated Windows 10).
4. GNU-toolchain build: `WebView2Loader.dll` ships next to the exe.
5. Keyboard-interactive with several different prompts (password + OTP) not supported.
6. `ld` prints a harmless "multiple non-default manifests" warning on GNU builds.
7. During installer testing, an SSH connection attempt and one window close appeared in the log while the test
   window was on screen; neither reproduced in two controlled 30-second idle runs (no auto-connect, no crash
   events). Consistent with manual interaction with the test window.

## Next tasks

1. Test against real OpenSSH servers; test on a clean Windows 10 and 11 VM, including a PC without WebView2.
2. Click-through QA of the 🟡 screens.
3. Code signing and CI (GitHub Actions on `windows-latest` with MSVC, which also removes `WebView2Loader.dll`).
4. Phase 5 protocols (Telnet/Serial first), macros, SFTP folder transfers.
