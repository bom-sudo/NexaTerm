# Changelog

## Unreleased

### Added
- SFTP: upload and download whole folders (also by dragging folders from Explorer), permissions dialog (chmod with checkboxes or octal, optionally recursive; chown by uid/gid), and an SFTP side panel next to the terminal that can follow the shell's current folder (OSC 7 prompt hook, opt-in per session).
- Import sessions from MobaXterm (.mxtsessions / MobaXterm.ini) and PuTTY (registry of the current user or a .reg export).
- Terminal output highlighting of error / warning / success words and IPv4 addresses (never inside full-screen programs or already coloured text); confirmation before pasting several lines. Both can be turned off in Settings.
- Telnet sessions (option negotiation, window size, terminal type) and Serial (COM) sessions with speed, data bits, parity, stop bits and flow control.
- SSH: keyboard-interactive logins with several prompts (password then one-time code), agent forwarding (ssh -A), zlib compression, X11 forwarding to a locally installed X server (VcXsrv, Xming, X410), PuTTY .ppk private keys.
- Macros: record keystrokes from a terminal (including pauses) and replay them after a preview, with destructive-command warnings.

## 1.0.0 — 2026-11-23

Release engineering (no new end-user features).

### Added
- Release artifacts: `NexaTerm-Setup-x64.exe` (offline: bundles the WebView2 Runtime installer), `NexaTerm-Setup-x64-web.exe` (downloads WebView2 only if missing), `NexaTerm-Portable-x64.zip`, `SHA256SUMS.txt`, release notes and guides in `dist/release/v1.0.0/`.
- Installer: English/Thai wizard, per-user install, install-folder and Start-menu pages, optional desktop shortcut, uninstaller; "delete application data" now also removes `%APPDATA%\NexaTerm` (NSIS hook).
- Portable mode via `portable-data\` next to the exe, with automatic fallback to `%LOCALAPPDATA%\NexaTerm\portable-fallback` (and an in-app warning) when the folder is read-only.
- Application log `logs\nexaterm.log` (redacted, rotated at 5 MB).
- `NexaTerm.exe --self-test`: headless diagnostics for installed/portable builds.
- Official NexaTerm branding: app icon, monochrome icon, dark/light wordmarks.
- Build automation: `scripts/build-windows.ps1`, `scripts/verify-release.ps1`, `scripts/test-installer.ps1`, `scripts/test-portable.ps1`.

### Changed
- Version 1.0.0 across package.json, Cargo.toml and tauri.conf.json; publisher "NexaTerm Project"; identifier `org.nexaterm.workstation`.
- Vite output moved to `build/web` (so `dist/release` is never wiped or embedded).
- Tauri build tools (NSIS, WebView2 installer) cached under `src-tauri/target/.tauri` instead of the system drive.

## 0.1.0 — 2026-11-23

First working release (Phases 1–4 core).

### Added
- Tauri 2 + React desktop shell: sidebar, toolbar, tab bar with searchable tab list, status bar, dark/light themes, onboarding.
- Terminal: xterm.js tabs, 2-pane and 4-pane splits with draggable dividers, search, clipboard, export, session logging, font/cursor/scrollback settings, four color themes.
- Local shells via ConPTY with detection of PowerShell, PowerShell 7, CMD, Git Bash and WSL.
- SSH (russh): password, key, agent, keyboard-interactive; SHA-256 host-key verification with known-hosts store; ProxyJump; keepalive, timeout, retries; reconnect.
- Session manager: folders, favorites, tags, search, sort, history, Quick Connect, OpenSSH config import, JSON import/export without secrets.
- Windows Credential Manager storage for passwords and passphrases.
- SFTP browser with transfer queue, progress, cancellation, drag-and-drop upload and an atomic-save remote editor.
- SSH tunnels: local, remote, dynamic SOCKS5.
- Multi-execution with confirmation and destructive-command warnings.
- SSH key manager (Ed25519/RSA generation, import, passphrase change, public key export).
- Command snippets with variables; network toolbox (ping, traceroute, DNS, reverse DNS, TCP test, interfaces, public IP).
- SQLite with migrations, backup/restore, portable mode.
- Mock SSH/SFTP server and integration test suite; NSIS installer and portable zip builds.
