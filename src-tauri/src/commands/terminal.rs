use super::S;
use crate::credentials;
use crate::database::repo;
use crate::error::{AppError, AppResult};
use crate::ssh::{self, shell::{ShellOptions, SshShell}, Secrets};
use crate::terminal::{local::{LocalPty, LocalSpec}, shells, TermEvent, TermSink, TerminalInfo};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};

fn make_sink(on_data: Channel<InvokeResponseBody>, on_event: Channel<TermEvent>) -> TermSink {
    TermSink::new(
        move |bytes| {
            let _ = on_data.send(InvokeResponseBody::Raw(bytes));
        },
        move |ev| {
            let _ = on_event.send(ev);
        },
    )
}

#[tauri::command]
pub fn shells_list() -> Vec<shells::ShellProfile> {
    shells::detect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalOpen {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub label: String,
    pub session_id: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[tauri::command]
pub fn terminal_open_local(state: S, spec: LocalOpen, on_data: Channel<InvokeResponseBody>, on_event: Channel<TermEvent>) -> AppResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let sink = make_sink(on_data, on_event);
    let pty = LocalPty::spawn(LocalSpec { program: spec.program, args: spec.args, cwd: spec.cwd, cols: spec.cols, rows: spec.rows }, sink.clone())?;
    state.terminals.insert(id.clone(), "local", &spec.label, pty, sink);
    if let Some(sid) = &spec.session_id {
        let db = state.db.lock().unwrap();
        let _ = repo::touch_connected(&db, sid);
        let _ = repo::add_history(&db, Some(sid), &spec.label, "local", None, "connected");
    }
    Ok(id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshOpen {
    pub session_id: String,
    pub password: Option<String>,
    pub passphrase: Option<String>,
    /// Persist a secret typed into the prompt (only after successful authentication).
    #[serde(default)]
    pub save_secret: bool,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshOpened {
    pub terminal_id: String,
    pub host: String,
    pub username: String,
    pub label: String,
}

#[tauri::command]
pub async fn terminal_open_ssh(state: S<'_>, req: SshOpen, on_data: Channel<InvokeResponseBody>, on_event: Channel<TermEvent>) -> AppResult<SshOpened> {
    let session = { repo::get_session(&state.db.lock().unwrap(), &req.session_id)? };
    let secrets = Secrets { password: req.password.clone(), passphrase: req.passphrase.clone() };
    let ctx = state.ssh_ctx();
    let retries = session.options.retries.unwrap_or(0).min(5);
    let mut attempt = 0;
    let conn = loop {
        match ssh::connect(&ctx, &session, &secrets, 0).await {
            Ok(c) => break c,
            // Retry only transient network failures, never auth or host-key problems.
            Err(e @ (AppError::Timeout | AppError::Connection(_))) if attempt < retries => {
                attempt += 1;
                log::info!("connect retry {attempt} for {}: {e}", session.name);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Err(e) => {
                let outcome = match &e {
                    AppError::AuthFailed(_) => "auth failed",
                    AppError::HostKeyUnknown { .. } | AppError::HostKeyChanged { .. } => "host key check",
                    AppError::SecretRequired(_) => "needs secret",
                    _ => "failed",
                };
                log::warn!("ssh connect to session {} failed ({outcome}): {e}", session.id);
                let db = state.db.lock().unwrap();
                let _ = repo::add_history(&db, Some(&session.id), &session.name, "ssh", session.host.as_deref(), outcome);
                return Err(e);
            }
        }
    };

    if req.save_secret {
        let db = state.db.lock().unwrap();
        if let Some(p) = req.password.as_deref().filter(|p| !p.is_empty()) {
            state.secrets.set(&credentials::target(&session.id, "password"), p)?;
            repo::set_secret_flags(&db, &session.id, Some(true), None)?;
        }
        if let Some(p) = req.passphrase.as_deref().filter(|p| !p.is_empty()) {
            state.secrets.set(&credentials::target(&session.id, "passphrase"), p)?;
            repo::set_secret_flags(&db, &session.id, None, Some(true))?;
        }
    }

    let sink = make_sink(on_data, on_event);
    let opts = ShellOptions {
        term: session.options.term_type.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "xterm-256color".into()),
        cols: req.cols.max(1),
        rows: req.rows.max(1),
        startup_dir: session.options.startup_dir.clone(),
        startup_command: session.options.startup_command.clone(),
    };
    let shell = SshShell::open(conn.clone(), opts, sink.clone()).await?;
    let id = uuid::Uuid::new_v4().to_string();
    state.terminals.insert(id.clone(), "ssh", &session.name, shell, sink);
    state.ssh_conns.lock().unwrap().insert(id.clone(), conn.clone());
    log::info!("ssh session {} connected to {}:{}", session.id, conn.host, conn.port);
    {
        let db = state.db.lock().unwrap();
        let _ = repo::touch_connected(&db, &session.id);
        let _ = repo::add_history(&db, Some(&session.id), &session.name, "ssh", session.host.as_deref(), "connected");
    }
    Ok(SshOpened { terminal_id: id, host: conn.host.clone(), username: conn.username.clone(), label: session.name })
}

#[tauri::command]
pub fn terminal_write(state: S, id: String, data: String) -> AppResult<()> {
    state.terminals.write(&id, data.as_bytes())
}

/// Multi-execution: send the same input to several terminals. Returns per-terminal results.
#[tauri::command]
pub fn terminal_broadcast(state: S, ids: Vec<String>, data: String) -> Vec<(String, Option<String>)> {
    ids.into_iter()
        .map(|id| {
            let r = state.terminals.write(&id, data.as_bytes()).err().map(|e| e.to_string());
            (id, r)
        })
        .collect()
}

#[tauri::command]
pub fn terminal_resize(state: S, id: String, cols: u16, rows: u16) -> AppResult<()> {
    state.terminals.resize(&id, cols, rows)
}

#[tauri::command]
pub fn terminal_close(state: S, id: String) {
    state.terminals.close(&id);
    state.ssh_conns.lock().unwrap().remove(&id);
}

#[tauri::command]
pub fn terminals_list(state: S) -> Vec<TerminalInfo> {
    state.terminals.list()
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(40).collect()
}

#[tauri::command]
pub fn terminal_log_start(state: S, id: String, dir: Option<String>) -> AppResult<String> {
    let e = state.terminals.get(&id)?;
    let base = dir.map(std::path::PathBuf::from).unwrap_or_else(|| state.data_dir.join("logs"));
    let path = base.join(format!("{}-{}.log", safe_name(&e.label), chrono::Local::now().format("%Y%m%d-%H%M%S")));
    state.terminals.start_log(&id, path.clone())?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn terminal_log_stop(state: S, id: String) -> AppResult<Option<String>> {
    Ok(state.terminals.stop_log(&id)?.map(|p| p.display().to_string()))
}

#[tauri::command]
pub fn known_hosts_list(state: S) -> AppResult<Vec<crate::database::models::KnownHost>> {
    repo::list_known_hosts(&state.db.lock().unwrap())
}

/// Explicit user decision to trust the exact key that was displayed.
#[tauri::command]
pub fn known_host_trust(state: S, host: String, port: u16, public_key: String) -> AppResult<String> {
    let host = crate::security::validate_host(&host)?;
    ssh::trust_host_key(&state.db.lock().unwrap(), &host, port, &public_key)
}

#[tauri::command]
pub fn known_host_remove(state: S, host: String, port: u16) -> AppResult<()> {
    repo::delete_known_host(&state.db.lock().unwrap(), &host, port)
}

#[tauri::command]
pub fn write_text_file(path: String, content: String) -> AppResult<()> {
    std::fs::write(path, content)?;
    Ok(())
}
