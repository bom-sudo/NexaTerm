# NexaTerm {{VERSION}}

**One Workspace. Every Remote Connection.** Free and open-source (MIT) terminal, SSH, SFTP and
remote administration workstation for Windows 10/11 and Linux (x64). No account, no telemetry,
no paid tier, no usage limits.

## Which file do I need?

| You use | Download | How |
|---|---|---|
| Windows 10 / 11 (most people) | `NexaTerm-Setup-x64-web.exe` | Double-click and follow the wizard. Per-user install, no admin rights. |
| Windows, no installation | `NexaTerm-Portable-x64.zip` | Extract anywhere (also a USB drive) and run `NexaTerm.exe`. |
| Windows PC without internet | `NexaTerm-Setup-x64.exe` | Same installer with Microsoft WebView2 included (large file). |
| Ubuntu / Debian / Mint | `NexaTerm-x64.deb` | `sudo apt install ./NexaTerm-x64.deb` |
| Fedora / openSUSE / RHEL | `NexaTerm-x64.rpm` | `sudo dnf install ./NexaTerm-x64.rpm` |
| Any other Linux desktop | `NexaTerm-x64.AppImage` | `chmod +x NexaTerm-x64.AppImage && ./NexaTerm-x64.AppImage` |

`SHA256SUMS.txt` lists the SHA-256 checksum of every file. All files are built from the source in
this repository by GitHub Actions after the full test suite passes.

**If Windows shows "Windows protected your PC":** click *More info*, then *Run anyway*. Windows
shows this for new programs that few people have downloaded yet. NexaTerm is applying for a free
code signing certificate from SignPath Foundation; signed releases will show the publisher name.

## New in 1.1.0

* **Linux** (x64): deb, rpm and AppImage. Login shell in a real PTY, passwords in the desktop
  keyring (GNOME Keyring / KWallet), ssh-agent, serial devices, X11 forwarding to your display.
* **Telnet** and **Serial** sessions (COM ports on Windows, `/dev/ttyUSB*` / `/dev/ttyACM*` on Linux).
* **SSH:** two-factor logins (password then one-time code), agent forwarding, compression,
  X11 forwarding, PuTTY `.ppk` keys, `~/` in key paths.
* **SFTP:** upload / download whole folders, permissions dialog (chmod / chown), a side panel next
  to the terminal that can follow the shell's current folder.
* **Import** sessions from MobaXterm and PuTTY.
* **Terminal:** highlighting of errors, warnings and IP addresses; confirmation before pasting
  several lines; **macros** (record and replay keystrokes).

## Already in 1.0

Tabs and split panes, local shells (PowerShell, cmd, Git Bash, WSL), SSH with host-key
verification and ProxyJump, session manager with folders and favorites, SFTP browser and editor,
SSH tunnels (-L / -R / SOCKS), multi-execution, SSH key manager, snippets, network tools,
portable mode, backup / restore.

## Not included yet

RDP, VNC, a built-in X server, master password, remote monitoring.

Full history: [CHANGELOG.md](https://github.com/bom-sudo/NexaTerm/blob/main/CHANGELOG.md)
