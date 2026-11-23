# NexaTerm Architecture

NexaTerm is a Tauri 2 desktop app: a React/TypeScript UI in WebView2, backed by a Rust core that owns every
privileged operation (processes, sockets, files, secrets). The UI never touches the network or the disk directly.

```
┌───────────────────────── WebView2 (React + xterm.js) ─────────────────────────┐
│ features/: terminal · sessions · ssh · sftp · tunnels · network-tools ·      │
│            ssh-keys · snippets · settings        store/: zustand slices       │
│ services/api.ts  ── typed wrappers around invoke()/Channel ──                 │
└──────────────────────────────┬────────────────────────────────────────────────┘
                               │ Tauri IPC (commands + Channels for byte streams)
┌──────────────────────────────▼────────────────────────────────────────────────┐
│ commands/      thin #[tauri::command] layer, input validation, error mapping   │
│ terminal/      TerminalManager: id → Box<dyn TerminalBackend> (PTY or SSH)    │
│   local.rs     ConPTY via portable-pty, shell detection                        │
│ ssh/           russh client, host-key verification, auth, ProxyJump            │
│ sftp/          russh-sftp ops, streaming transfer queue with cancellation      │
│ tunnels/       local (-L), remote (-R), dynamic SOCKS5 (-D) forwards           │
│ network/       ping/tracert (structured args), DNS, TCP probe, interfaces       │
│ keys/          Ed25519/RSA generation & inspection (ssh-key)                   │
│ credentials/   SecretStore trait → Windows Credential Manager (in-memory in tests)│
│ database/      SQLite (rusqlite, bundled), versioned migrations, repositories   │
│ security/      redaction, path & host validation                               │
└───────────────────────────────────────────────────────────────────────────────┘
```

## Key decisions

| Area | Choice | Why |
|---|---|---|
| SSH / SFTP | `russh` + `russh-sftp` (Apache-2.0), `ring` crypto backend | Pure Rust, async (Tokio), actively maintained, SFTP + forwarding + agent support, no OpenSSL. `libssh2` would require OpenSSL/WinCNG C builds and blocking I/O. |
| Local PTY | `portable-pty` (MIT) | Uses Windows ConPTY; mature (from WezTerm). |
| Database | `rusqlite` with bundled SQLite (MIT / public domain) | No system SQLite needed; single file in the app data dir. |
| Secrets | Windows Credential Manager via `windows` crate (MIT/Apache) | OS-protected per-user storage (DPAPI under the hood). DB only stores *whether* a secret exists. |
| Streams | `tauri::ipc::Channel` with raw byte payloads | Avoids JSON-encoding terminal output; ordered per channel. |
| Frontend state | `zustand` | Small, no boilerplate. |

## Terminal model

Every pane in the workspace binds to a backend *terminal id*. Local shells and SSH shells implement the same
`TerminalBackend` trait (`write`, `resize`, `close`), so tabs, split panes, logging and multi-execution do not care
what is on the other end. Output flows Rust → `Channel` → xterm.js; input flows xterm.js → `terminal_write`.

## SSH host-key verification

1. On connect, `check_server_key` compares the presented key against `known_hosts` (SQLite).
2. Match → continue. Unknown → connection aborts with `HostKeyUnknown { fingerprint, key }`.
   Changed → aborts with `HostKeyChanged { old, new }`.
3. The UI shows the SHA-256 fingerprint and requires an explicit decision. Only the *exact* key that was shown can be
   trusted (`known_hosts_trust`), then the connection is retried. There is no "trust all" switch anywhere.

## Secrets

* Passwords / passphrases: Credential Manager target `NexaTerm/<session-id>/<kind>`.
* Private keys stay in files the user chooses; the DB stores only the path.
* Logs pass through `security::redact`; error messages never include secrets.
* Session export writes no secrets.

## Data location

* Installed: `%APPDATA%\NexaTerm\nexaterm.db`
* Portable: if a `portable-data\` folder sits next to the executable, data lives there. If it is not
  writable, `%LOCALAPPDATA%\NexaTerm\portable-fallback` is used and the user is warned.
