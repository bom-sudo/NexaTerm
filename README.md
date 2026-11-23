# NexaTerm

**One Workspace. Every Remote Connection.**

NexaTerm is a free, open-source (MIT) remote administration workstation for Windows 10/11:
a tabbed terminal with split panes, an SSH client with real host-key verification, an SFTP file
manager, SSH tunnels, multi-execution, an SSH key manager, command snippets and network diagnostics —
in one offline-first desktop app. No account, no telemetry, no paid tier, no usage limits.

## Features (v1.0.0)

| Area | What works today |
|---|---|
| Terminal | xterm.js, tabs, split view (2 side-by-side / stacked, 4-pane grid) with draggable dividers, 256-color + truecolor, Unicode 11, search, copy/paste (Ctrl+C with selection, Ctrl+V, Ctrl+Shift+C/V, right-click), clear/reset/select-all/scroll-to-bottom, export output, session logging to file, font/cursor/theme settings, 4 color themes |
| Local shells | Auto-detects Windows PowerShell, PowerShell 7, Command Prompt, Git Bash and WSL distributions; ConPTY backend; custom executables with structured arguments |
| SSH | Password, private key (OpenSSH/PEM, encrypted keys), Windows OpenSSH agent, keyboard-interactive (password-style prompts); SHA-256 host key verification with known-hosts store; ProxyJump via another saved session; keepalive, timeout, retries; startup directory/command; reconnect; many simultaneous sessions |
| Sessions | Create/edit/duplicate/delete, folders (rename/delete, drag-and-drop), favorites, tags, search, sort, recent connections, history, import `~/.ssh/config`, JSON import/export (no secrets), Quick Connect |
| SFTP | Browse with back/forward/up/home, path bar, sort, filter, size/date/permissions/owner, upload (dialog or drag & drop from Explorer), download, sequential transfer queue with progress and cancel, mkdir/new file/rename/delete with confirmations, remote text editor with atomic save |
| Tunnels | Local (-L), remote (-R) and dynamic SOCKS5 (-D) forwards; start/stop/restart/edit/delete; live connection counters; port-conflict detection |
| Multi-execution | Pick target terminals, broadcast a command (with confirmation and destructive-command warnings) or mirror keystrokes; per-target result |
| SSH keys | Generate Ed25519 / RSA (with passphrase), import, view/copy/export public key, change/remove passphrase, malformed-file detection |
| Snippets | Categories, search, `{{variable:default}}` prompts, preview-before-run, copy |
| Network tools | Ping, traceroute, DNS, reverse DNS, TCP port test, local interfaces/hostname, public IP (on click only) — all cancellable |
| Data | SQLite with migrations, backup/restore, Windows Credential Manager for secrets, portable mode |

Not yet implemented (no UI is shown for them): RDP, VNC, X11 forwarding, Telnet, serial, macro
recording. See [DEVELOPMENT_STATUS.md](DEVELOPMENT_STATUS.md).

## Install (v1.0.0)

| Download | Notes |
|---|---|
| `NexaTerm-Setup-x64.exe` (≈287 MB) | Wizard, per-user, no admin. Bundles the WebView2 Runtime → works offline. Needs ~300 MB free in `%TEMP%` while installing. |
| `NexaTerm-Setup-x64-web.exe` (≈3 MB) | Same installer; downloads WebView2 from Microsoft only if missing. |
| `NexaTerm-Portable-x64.zip` (≈3.6 MB) | Extract anywhere, run `NexaTerm.exe`; data stays in `portable-data\`. |

Releases are **not code-signed**; verify downloads with `SHA256SUMS.txt`.
See [docs/INSTALL.md](docs/INSTALL.md) and the guides shipped with each release.

## Build from source

Prerequisites: Node.js 22+, Rust stable, and a C toolchain (MSVC Build Tools, or the no-admin MinGW
setup in [docs/DEVELOPER.md](docs/DEVELOPER.md)).

```powershell
git clone <your-repo-url> NexaTerm
cd NexaTerm
npm ci                            # install exact dependency versions
npm run tauri dev                 # run in development mode
npm test                          # frontend unit tests (Vitest)
npm run test:rust                 # Rust unit + SSH/SFTP integration tests
npm run build:release             # full pipeline -> dist/release/v<version>/ (installers, portable zip, checksums)
npm run verify:release            # re-verify a release folder, incl. self-test of the portable exe
```

A local SSH/SFTP test server (login `tester` / `secret`) for trying the app without a real server:
`cargo run --manifest-path src-tauri/Cargo.toml --example mock_sshd -- 2222`

## Repository layout

```
src/                 React + TypeScript UI (features/, layouts/, store/, services/api.ts)
src-tauri/           Rust core (ssh/, sftp/, terminal/, tunnels/, database/, credentials/ …), migrations, tests
packaging/           installer hooks and release documents (templated with the version)
public/              logo, icon and wordmarks
scripts/             build-windows.ps1, verify-release.ps1, test-installer.ps1, test-portable.ps1, devenv.*
docs/                architecture, developer, install and user guides
```

Generated folders (`node_modules/`, `build/`, `dist/`, `src-tauri/target/`) are git-ignored.
Release binaries belong on a GitHub Release, not in the repository.

## Documentation

* [User guide](docs/USER_GUIDE.md) · [Installation](docs/INSTALL.md) · [Developer guide](docs/DEVELOPER.md)
* [Architecture](docs/ARCHITECTURE.md) · [Security](SECURITY.md) · [Third-party licenses](THIRD_PARTY_LICENSES.md)
* [Development status](DEVELOPMENT_STATUS.md) · [Changelog](CHANGELOG.md)

NexaTerm is an independent clean-room project. It is not affiliated with, and contains no code or
assets from, MobaXterm or any other commercial product.
