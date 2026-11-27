# Changelog

## 1.1.1 — 2026-11-28

### Added
- Status bar shows the CPU, memory, root disk, network rate and uptime of the SSH server in the active pane (Linux servers, read every 3 s over the same connection; can be turned off in Settings).
- Check for updates in Settings and on the version number in the status bar. Installed Windows builds download the new installer, verify its SHA-256 against the release checksums and update in place; portable and Linux builds open the release page. Runs only when clicked.
- Close button on every pane of a split tab.

### Fixed
- Thai installer: the desktop-shortcut checkbox and other installer messages were blank, and several standard texts were mistranslated. The installer now has its own NexaTerm sidebar image.
- New session dialog: an empty or invalid host, user name, key file or port is marked on the field itself with a message saying what to enter, instead of a short error at the bottom.
- Linux: folder downloads over SFTP put files in subfolders under wrong names.

## 1.1.0 — 2026-11-27

### Added
- SFTP: upload and download whole folders (also by dragging folders from Explorer), permissions dialog (chmod with checkboxes or octal, optionally recursive; chown by uid/gid), and an SFTP side panel next to the terminal that can follow the shell's current folder (OSC 7 prompt hook, opt-in per session).
- Import sessions from MobaXterm (.mxtsessions / MobaXterm.ini) and PuTTY (registry of the current user or a .reg export).
- Terminal output highlighting of error / warning / success words and IPv4 addresses (never inside full-screen programs or already coloured text); confirmation before pasting several lines. Both can be turned off in Settings.
- Telnet sessions (option negotiation, window size, terminal type) and Serial (COM) sessions with speed, data bits, parity, stop bits and flow control.
- SSH: keyboard-interactive logins with several prompts (password then one-time code), agent forwarding (ssh -A), zlib compression, X11 forwarding to a locally installed X server (VcXsrv, Xming, X410), PuTTY .ppk private keys.
- Macros: record keystrokes from a terminal (including pauses) and replay them after a preview, with destructive-command warnings.
- Linux x64: login-shell PTYs, desktop keyring (Secret Service) for secrets, ssh-agent via `SSH_AUTH_SOCK`, serial devices (`/dev/ttyUSB*`, `/dev/ttyACM*`, termios), X11 forwarding to `$DISPLAY` (Unix socket), ping/traceroute; deb, rpm and AppImage bundles.
- GitHub Actions: CI builds and tests on Linux and Windows (with a real GNOME Keyring for the secret-store test); Release workflow drafts a GitHub release with Windows and Linux files.
- Code signing: Release workflow signs the exe and both installers through SignPath Foundation when configured; `scripts/build-windows.ps1 -Sign` signs local builds with your own certificate (store thumbprint or PFX) via `scripts/sign-file.ps1`; `verify-release.ps1 -RequireSignature` enforces valid, timestamped signatures. See docs/CODE_SIGNING.md.

### Changed
- X11 forwarding now works like OpenSSH: the server only gets a random fake cookie, which is checked and replaced with the display's real cookie locally.
- Private key paths may start with `~/`.

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
