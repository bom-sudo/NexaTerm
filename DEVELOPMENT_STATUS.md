# Development Status — NexaTerm 1.0.0 (2026-11-23)

Legend: **Done** = built and tested · **Partial** = implemented and covered by automated tests, UI not clicked through · **Not started** = not implemented

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
| Frontend type check + Vitest | Passed 18/18 | |
| Rust unit tests | Passed 34/34 | |
| SSH/SFTP/tunnel integration (mock server) | Passed 11/11 | |
| `verify-release.ps1` | Passed (VERIFY PASSED) | PE files, versions, ZIP layout/integrity, checksums, portable self-test |
| Offline installer (`test-installer.ps1`) | Passed 19/19 | Real wizard via UI Automation, Start-menu shortcut, registry, launch from shortcut, self-test ×2, uninstall, data retained, silent reinstall to path with spaces, uninstall. Run with `TEMP` on D: (see limitation 1). |
| Web installer (`test-installer.ps1`) | Passed 19/19 | Same suite with default `TEMP` on the nearly full C: drive |
| Portable (`test-portable.ps1`) | Passed 15/15 | Path with spaces, PATH without dev tools, GUI open + graceful close, persistence, logs, DB integrity, no secrets on disk, move folder, read-only fallback with in-app warning |

Environment: Windows 11 Pro (build 26300), x64, per-user without admin rights, WebView2 157 already installed.
**Not tested:** a clean Windows VM, Windows 10, a machine *without* WebView2 (the runtime-install path of
the installers), the uninstaller's "delete application data" checkbox (would delete this machine's real data),
and SSH against a real OpenSSH server (all SSH tests use the bundled mock server).

## Acceptance criteria

| Criterion | Status |
|---|---|
| Application starts on Windows | Done: installed and portable release builds |
| Local PowerShell / CMD terminals | Done: GUI (dev build) + ConPTY self-test in release builds |
| Multiple tabs, split terminal | Done: GUI (dev build) |
| SSH password / private-key login | Done: integration tests; password login also via GUI against mock server |
| Host fingerprint verification, known hosts persisted | Done: integration + GUI |
| Session profiles create/edit, persist after restart | Done: unit tests + release persistence tests |
| Multiple simultaneous SSH sessions | Done: integration test |
| SFTP browse / upload / download | Done: integration; browse + edit via GUI |
| Port forwarding | Done: integration (local, SOCKS5); Partial: tunnel UI |
| Credentials securely stored | Done: Credential Manager tests, no secrets on disk |
| No mandatory internet, no paid services, no usage limits | Done |
| Windows installer builds | Done: offline + web |
| Portable edition builds | Done |
| Documentation | Done |

## Feature status

| Module | Status |
|---|---|
| Terminal emulator, local shells | Done |
| SSH client, session manager | Done (import/export and folder drag-drop: Partial) |
| SFTP + remote editor | Done (folder transfer: Not started) |
| Tunnels, multi-execution, key manager, network tools, snippets, settings/backup | Partial (backend tested, UI type-checked, not clicked through) |
| Portable mode, read-only fallback, app log, `--self-test` | Done |
| Telnet, Serial (COM) | Done (Telnet: integration test against a scripted server; Serial: settings and port validation unit-tested, not tested with real COM hardware) |
| X11 forwarding | Done (forwarding channel and X server detection implemented; not tested with a real X server and Linux GUI program) |
| SSH 2FA (keyboard-interactive, several prompts), agent forwarding, compression, .ppk keys | Done (2FA and compression integration-tested; agent forwarding not tested against a server) |
| SFTP folder transfer, permissions, side panel | Done (folder transfer and chmod integration-tested; follow-folder needs bash/zsh, not testable with the mock shell) |
| Import MobaXterm / PuTTY | Done (parsers unit-tested with constructed samples; not yet tested with real export files) |
| Output highlighting, multi-line paste confirmation | Done |
| Macros (record / replay) | Done |
| RDP, VNC, embedded X server, master password, remote monitoring bar, IP / port scanner, Wake-on-LAN, built-in TFTP/FTP/HTTP servers, detachable tabs, bundled Unix tools, custom key bindings | Not started |

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
2. Click-through QA of the screens marked Partial.
3. Code signing and CI (GitHub Actions on `windows-latest` with MSVC, which also removes `WebView2Loader.dll`).
4. Phase 5 protocols (Telnet/Serial first), macros, SFTP folder transfers.
