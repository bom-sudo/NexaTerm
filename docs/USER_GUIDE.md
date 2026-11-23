# NexaTerm User Guide

## Layout

* **Left rail** — Quick Connect, Sessions, Recent connections, Snippets, then tools that open as
  tabs: SSH Tunnels, SSH Keys, Network Tools, Known Hosts, Settings.
* **Side panel** — the list for the selected rail view.
* **Toolbar** — new session, quick connect, new terminal (▾ chooses the shell or splits),
  split/layout buttons, multi-execution, SFTP, terminal search/clear/reset/select-all/scroll,
  export output, start/stop logging, font size, settings.
* **Tab bar** — one tab per terminal group, SFTP browser, editor or tool. The counter on the right
  opens a searchable tab list. Double-click a tab to rename; middle-click to close.
* **Status bar** — connection state, host, user, connect time, active sessions, encoding, version.

## Local terminals

Ctrl+T or the toolbar opens your default shell (Settings → Local shells). The ▾ menu lists every
detected shell (PowerShell, PowerShell 7, CMD, Git Bash, WSL distros) and can open one as a split.
Save a *Local terminal* session to remember a custom executable, arguments and start directory.

## SSH sessions

1. **New** in the Sessions panel (Ctrl+Shift+N). Enter host, port, user and authentication.
2. Passwords/passphrases are optional — leave them empty to be asked on connect. If you type one,
   it is stored in **Windows Credential Manager**, never in NexaTerm's database.
3. Double-click the session to connect. On the first connection you are shown the server's
   SHA-256 fingerprint; compare it with the server (`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`)
   and choose **Trust and connect**.
4. If a server's key ever changes, NexaTerm blocks the connection and shows both fingerprints.
   Only replace the stored key after confirming the change with the server's administrator.

**ProxyJump:** set *Jump host* to another saved SSH session; that session must have its credentials
saved (or use key/agent auth). **Agent:** start the Windows *OpenSSH Authentication Agent* service and
`ssh-add` your keys. **Reconnect:** when a connection drops, the pane shows *Reconnect*.

Right-click a session for: connect in split pane, open SFTP, edit, duplicate (secrets are not
copied), favorite, move to folder, delete. Drag sessions onto folders to move them.
Import from `~/.ssh/config` or a NexaTerm JSON export via the import button.

## Split panes and tabs

Up to four panes per tab. Drag the dividers to resize. Shortcuts:

| Shortcut | Action |
|---|---|
| Ctrl+Shift+N | New session |
| Ctrl+T / Ctrl+Shift+T | New terminal |
| Ctrl+W / Ctrl+Shift+W | Close tab |
| Ctrl+Tab / Ctrl+Shift+Tab | Next / previous tab |
| Ctrl+F | Search in terminal (Enter / Shift+Enter, Esc closes) |
| Ctrl+Shift+F | Search sessions |
| Ctrl+Plus / Ctrl+Minus / Ctrl+0 | Font size |
| Ctrl+C (with selection) / Ctrl+V | Copy / paste; Ctrl+Shift+C / Ctrl+Shift+V always copy/paste |

Inside a terminal, Ctrl+W and Ctrl+T go to the shell by default (bash uses Ctrl+W to delete a word);
use the Ctrl+Shift variants, or change *Send Ctrl+W / Ctrl+T to the shell* in Settings.

## SFTP

Open from the toolbar while an SSH terminal is active (reuses its connection) or right-click an SSH
session → *Open SFTP browser*. Double-click folders to enter, files to edit. Upload with the toolbar
or by dragging files from Explorer. Downloads ask for a folder. Transfers run one at a time and can
be cancelled; partial files are cleaned up. Existing files are never overwritten without asking.
Deletes always ask, and folder deletes are recursive.

The editor saves by uploading a temporary copy, applying the original permissions, and renaming it
over the original, so a failed save never corrupts the file.

## Tunnels

SSH Tunnels → *New tunnel*: choose the gateway session and type:

* **Local (-L)**: `127.0.0.1:15432 → db.internal:5432` makes a remote database reachable locally.
* **Remote (-R)**: the server listens on a port and forwards back to a host reachable from your PC.
* **Dynamic (-D)**: a SOCKS5 proxy on a local port.

Tunnels run only while started; each uses its own SSH connection.

## Multi-execution

Toolbar → radio icon. Select target terminals, type a command and **Broadcast** — you get a
confirmation with the full target list, and a red warning for destructive commands. *Mirror typing*
sends every keystroke typed in a selected pane to the other selected panes. Selected panes have an
amber border and a MULTI-EXEC badge.

## Snippets

Snippets panel → *New*. Use `{{name}}` or `{{name:default}}` for values asked at run time. Running
shows the final command and the target terminal before anything is sent.

## Network tools

Ping, traceroute, DNS, reverse DNS, TCP port test, local interfaces and public IP. Each runs on
demand, can be cancelled, and targets one host at a time.

## Data, backup and portable mode

* Installed: `%APPDATA%\NexaTerm\nexaterm.db`; session logs in `%APPDATA%\NexaTerm\logs`.
* Portable: `portable-data\` next to `NexaTerm.exe`.
* Settings → Data → *Back up* / *Restore*. A safety copy is written before a restore.
* Passwords stay in Windows Credential Manager (search for `NexaTerm/` in *Windows Credentials*).
