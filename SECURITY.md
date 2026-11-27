# Security

NexaTerm handles credentials and opens connections to production systems, so security is a design
requirement, not a feature. This document describes the controls that are implemented and verified
by tests, and the known limitations.

## Reporting a vulnerability

Please report privately to the maintainers (do not open a public issue with exploit details).
Include the version, steps to reproduce and impact.

## Implemented controls

| Area | Control | Where / test |
|---|---|---|
| SSH host keys | Every connection checks the server key against `known_hosts` (SQLite). Unknown keys abort the connection and show the SHA-256 fingerprint; the user must explicitly trust it. Changed keys are rejected and show old + new fingerprint; replacing requires an extra acknowledgement. Only the exact key that was displayed can be trusted (it is round-tripped and re-parsed). There is no "accept all" option. | `ssh::evaluate_host_key`, `tests/ssh_integration.rs` (unknown, changed) |
| Credentials | Passwords and key passphrases are stored only in **Windows Credential Manager** (`NexaTerm/<session-id>/<kind>`, per-user, DPAPI-protected by Windows). SQLite stores only a boolean "has secret". Secrets typed into prompts are saved only if the user ticks "Save". | `credentials`, `commands::sessions::tests::secrets_go_to_store_not_db`, Credential Manager round-trip test |
| Private keys | Never copied into the database or uploaded; NexaTerm stores the file path only. Key generation refuses to overwrite files. | `keys` |
| Exports & backups | Session exports contain no secrets and reset the secret flags. DB backups contain no secrets (they are not in the DB). | `repo::tests::export_import_has_no_secrets` |
| Logs & errors | Every error crossing IPC passes through `security::redact` (masks `password=`, `passphrase:`, `token=`, PEM blocks). | `security::tests`, `secrets_never_reach_logs_or_errors` |
| Process spawning | Local shells, `ping.exe`, `tracert.exe`, `wsl.exe` are started with structured argument vectors — never a concatenated command line. Host names are validated (no whitespace, metacharacters or leading `-`). | `security::validate_host`, `network::tests` (rejects `127.0.0.1 & calc`) |
| Paths | SFTP names are validated (no separators, `.`/`..`, NUL); deleting `/` is refused; downloads reject Windows-invalid names. | `security`, `sftp` |
| File transfers | Uploads write a hidden partial file and rename on success; downloads write `.part` and rename; cancelled/failed transfers delete partial files. Overwrites require confirmation. | `sftp_browse_upload_download_cancel_edit` |
| Remote editing | Save writes a temp file, copies permissions, then atomically replaces (posix-rename) or uses a backup-rename sequence so the original is never lost on failure. | same test |
| Tunnels | Tunnels start only on user action. Remote-forwarded channels are accepted only for ports the user requested; all others are rejected. Port conflicts are reported. | `tunnels`, `local_and_dynamic_tunnels_lifecycle` |
| Multi-execution | Broadcasting requires a confirmation listing every target; destructive patterns (`rm -rf`, `mkfs`, `dd of=/dev`, `shutdown`, `DROP TABLE`, …) trigger a stronger warning. Clear visual indicator while active. | `utils/commands.test.ts` |
| Tauri surface | Minimal capability set: core + dialog + clipboard. No shell, fs, or http plugin is exposed to the webview. Strict CSP (`default-src 'self'`). | `src-tauri/capabilities/default.json`, `tauri.conf.json` |
| Network | No telemetry, analytics, automatic update checks or background requests. Requests not tied to a user connection run only when clicked: "Public IP" (api.ipify.org) and "Check for updates" (api.github.com; a downloaded installer is run only if its SHA-256 matches the release's SHA256SUMS.txt). | code review: grep for network calls in `network/` |
| Privileges | Runs as the current user; installer is per-user (`currentUser`), portable needs no admin. Nothing is elevated. | `tauri.conf.json` |
| Cleanup | On window close all PTYs are killed, SSH connections dropped, tunnels stopped. | `lib.rs` `on_window_event` |

## Known limitations

- `freezePrototype` is disabled because xterm.js assigns to built-in prototypes; CSP and the minimal
  capability set remain the main webview hardening.
- Keyboard-interactive auth answers hidden prompts with the session password; multi-step prompts
  with different questions (e.g. OTP after password) are not yet supported interactively.
- Session log files (`%APPDATA%\NexaTerm\logs`) contain raw terminal output, which may include
  anything displayed in the terminal. They are only written when logging is switched on.
- The SSH agent integration supports the Windows OpenSSH agent named pipe only (not Pageant).
- Credential Manager entries are readable by any process running as the same Windows user — the
  same trust boundary as other Windows applications.
