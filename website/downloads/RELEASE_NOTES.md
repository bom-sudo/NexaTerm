# NexaTerm 1.0.0 โ€” Release Notes

**One Workspace. Every Remote Connection.** Free and open-source (MIT) remote administration
workstation for Windows 10/11 x64. No account, no telemetry, no paid tier, no usage limits.

## Downloads

| File | Description |
|---|---|
| `NexaTerm-Setup-x64.exe` | Installer wizard, per-user, includes the WebView2 Runtime offline installer (works without internet) |
| `NexaTerm-Setup-x64-web.exe` | Small installer; downloads WebView2 from Microsoft only if missing |
| `NexaTerm-Portable-x64.zip` | Portable edition (extract and run `NexaTerm.exe`) |
| `SHA256SUMS.txt` | SHA-256 checksums of the files above |

**Signing status: unsigned.** No code-signing certificate is used for this release. Windows SmartScreen
may warn on first run. Verify the checksums before running.

## Included in this version

* Terminal: tabs, split panes (2/4) with resizable dividers, search, clipboard, export, session logging, themes, font/cursor settings.
* Local shells via ConPTY: PowerShell, PowerShell 7, Command Prompt, Git Bash, WSL (auto-detected).
* SSH: password, private key (incl. encrypted), Windows OpenSSH agent, keyboard-interactive (password prompts),
  SHA-256 host-key verification with known-hosts store, ProxyJump, keepalive, retries, reconnect.
* Session manager: folders, favorites, tags, search, history, Quick Connect, `~/.ssh/config` import, JSON import/export (no secrets).
* SFTP browser: transfers with progress/cancel, drag-and-drop upload, remote editor with atomic save.
* SSH tunnels (-L, -R, SOCKS5 -D), multi-execution with safety confirmation, SSH key manager,
  command snippets with variables, network tools (ping, traceroute, DNS, reverse DNS, TCP test, interfaces, public IP).
* Secrets in Windows Credential Manager only; SQLite database with migrations and backup/restore.
* New in 1.0.0: portable mode with `portable-data` folder and read-only fallback, application log
  (`logs\nexaterm.log`, redacted), `--self-test` diagnostics, N logo, English/Thai installer.

## Not included yet

RDP, VNC, X11 forwarding, Telnet, serial, macro recording, recursive folder transfers over SFTP.

## Known limitations

* Unsigned executables (SmartScreen warning).
* Keyboard-interactive logins that ask several different questions (e.g. password then OTP) are not yet supported.
* SSH has been validated against NexaTerm's bundled test server and its automated test suite; test against
  your own servers before relying on it in production.
* The portable edition needs the WebView2 Runtime to be present on the PC (standard on Windows 11 / updated Windows 10).

