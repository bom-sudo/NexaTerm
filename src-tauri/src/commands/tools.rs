//! Tunnels, network toolbox, SSH keys, snippets.

use super::S;
use crate::database::models::*;
use crate::database::repo;
use crate::error::{AppError, AppResult};
use crate::keys::{self, KeyInfo};
use crate::network;
use crate::ssh::{self, Secrets};
use crate::tunnels::TunnelStatus;
use std::path::PathBuf;
use tauri::ipc::Channel;

// ---------------------------------------------------------------- tunnels

#[tauri::command]
pub fn tunnels_list(state: S) -> AppResult<Vec<TunnelProfile>> {
    repo::list_tunnels(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn tunnel_save(state: S, id: Option<String>, input: TunnelInput) -> AppResult<TunnelProfile> {
    if let Some(id) = &id {
        if state.tunnels.is_running(id) {
            return Err(AppError::Validation("Stop the tunnel before editing it".into()));
        }
    }
    repo::save_tunnel(&state.db.lock().unwrap(), id.as_deref(), &input)
}

#[tauri::command]
pub async fn tunnel_delete(state: S<'_>, id: String) -> AppResult<()> {
    state.tunnels.stop(&id).await?;
    repo::delete_tunnel(&state.db.lock().unwrap(), &id)
}

#[tauri::command]
pub async fn tunnel_start(state: S<'_>, id: String, password: Option<String>, passphrase: Option<String>) -> AppResult<()> {
    let (profile, session) = {
        let db = state.db.lock().unwrap();
        let p = repo::get_tunnel(&db, &id)?;
        let s = repo::get_session(&db, &p.session_id)?;
        (p, s)
    };
    // Fail fast on local port conflicts before spending time on the SSH handshake.
    if profile.kind != "remote" {
        drop(crate::tunnels::bind_listener(&profile.bind_address, profile.bind_port).await?);
    }
    let conn = ssh::connect(&state.ssh_ctx(), &session, &Secrets { password, passphrase }, 0).await?;
    state.tunnels.start(&profile, conn).await
}

#[tauri::command]
pub async fn tunnel_stop(state: S<'_>, id: String) -> AppResult<()> {
    state.tunnels.stop(&id).await
}

#[tauri::command]
pub fn tunnel_statuses(state: S) -> Vec<TunnelStatus> {
    state.tunnels.statuses()
}

// ---------------------------------------------------------------- network

#[tauri::command]
pub async fn net_run_tool(state: S<'_>, tool: String, host: String, count: Option<u32>, job_id: String, on_line: Channel<String>) -> AppResult<i32> {
    let cancel = state.new_job(&job_id);
    let r = network::run_tool(&tool, &host, count.unwrap_or(4), &cancel, |l| {
        let _ = on_line.send(l);
    })
    .await;
    state.finish_job(&job_id);
    r
}

#[tauri::command]
pub async fn net_dns(host: String) -> AppResult<network::DnsResult> {
    network::dns_lookup(&host).await
}

#[tauri::command]
pub async fn net_reverse(ip: String) -> AppResult<String> {
    network::reverse_dns(&ip).await
}

#[tauri::command]
pub async fn net_tcp_probe(host: String, port: i64, timeout_ms: Option<u64>) -> AppResult<network::TcpProbe> {
    network::tcp_probe(&host, port, timeout_ms.unwrap_or(3000)).await
}

#[tauri::command]
pub fn net_interfaces() -> AppResult<Vec<network::InterfaceInfo>> {
    network::interfaces()
}

#[tauri::command]
pub fn net_hostname() -> String {
    network::hostname()
}

#[tauri::command]
pub async fn net_public_ip() -> AppResult<String> {
    network::public_ip().await
}

// ---------------------------------------------------------------- keys

#[tauri::command]
pub fn keys_list(state: S) -> AppResult<Vec<SshKeyRecord>> {
    repo::list_keys(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn keys_default_dir() -> String {
    keys::default_ssh_dir().display().to_string()
}

fn register(state: &S, name: &str, info: &KeyInfo) -> AppResult<()> {
    repo::upsert_key(&state.db.lock().unwrap(), name, &info.path, &info.key_type, &info.fingerprint, Some(&info.comment), info.encrypted)
}

#[tauri::command]
pub fn key_generate(state: S, path: String, kind: String, bits: Option<usize>, comment: String, passphrase: Option<String>) -> AppResult<KeyInfo> {
    let p = PathBuf::from(&path);
    let info = keys::generate(&p, &kind, bits.unwrap_or(4096), &comment, passphrase.as_deref())?;
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("key").to_string();
    register(&state, &name, &info)?;
    Ok(info)
}

#[tauri::command]
pub fn key_import(state: S, path: String) -> AppResult<KeyInfo> {
    let p = PathBuf::from(&path);
    let info = keys::inspect(&p)?;
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("key").to_string();
    register(&state, &name, &info)?;
    Ok(info)
}

#[tauri::command]
pub fn key_inspect(path: String) -> AppResult<KeyInfo> {
    keys::inspect(&PathBuf::from(path))
}

#[tauri::command]
pub fn key_change_passphrase(state: S, path: String, old: Option<String>, new: Option<String>) -> AppResult<KeyInfo> {
    let p = PathBuf::from(&path);
    let info = keys::change_passphrase(&p, old.as_deref(), new.as_deref())?;
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("key").to_string();
    register(&state, &name, &info)?;
    Ok(info)
}

/// Removes the key from NexaTerm's list only. Key files are never deleted by the app.
#[tauri::command]
pub fn key_forget(state: S, id: String) -> AppResult<()> {
    repo::delete_key_record(&state.db.lock().unwrap(), &id)
}

// ---------------------------------------------------------------- X11

#[tauri::command]
pub async fn x11_status() -> crate::x11::XStatus {
    crate::x11::status().await
}

#[tauri::command]
pub fn x11_start(id: String) -> AppResult<()> {
    crate::x11::start(&id)
}

// ---------------------------------------------------------------- macros

#[tauri::command]
pub fn macros_list(state: S) -> AppResult<Vec<MacroProfile>> {
    repo::list_macros(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn macro_save(state: S, id: Option<String>, name: String, steps: Vec<MacroStep>) -> AppResult<String> {
    repo::save_macro(&state.db.lock().unwrap(), id.as_deref(), &name, &steps)
}

#[tauri::command]
pub fn macro_delete(state: S, id: String) -> AppResult<()> {
    repo::delete_macro(&state.db.lock().unwrap(), &id)
}

// ---------------------------------------------------------------- snippets

#[tauri::command]
pub fn snippets_list(state: S) -> AppResult<Vec<Snippet>> {
    repo::list_snippets(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn snippet_save(state: S, id: Option<String>, input: SnippetInput) -> AppResult<String> {
    repo::save_snippet(&state.db.lock().unwrap(), id.as_deref(), &input)
}

#[tauri::command]
pub fn snippet_delete(state: S, id: String) -> AppResult<()> {
    repo::delete_snippet(&state.db.lock().unwrap(), &id)
}

// ---------------------------------------------------------------- updates (only when the user asks)

#[tauri::command]
pub async fn update_check(state: S<'_>) -> AppResult<crate::updater::UpdateInfo> {
    let portable = state.portable;
    tokio::task::spawn_blocking(move || crate::updater::check(portable)).await.map_err(|e| AppError::Other(e.to_string()))?
}

/// Downloads and verifies the installer; progress is streamed as (done, total) bytes.
/// Returns the path of the verified installer.
#[tauri::command]
pub async fn update_download(state: S<'_>, on_progress: Channel<(u64, u64)>) -> AppResult<String> {
    let portable = state.portable;
    tokio::task::spawn_blocking(move || {
        let mut last = 0u64;
        crate::updater::download_installer(portable, |done, total| {
            // ~1% steps are plenty for a progress bar.
            if done == total || done.saturating_sub(last) >= (total / 100).max(64 * 1024) {
                last = done;
                let _ = on_progress.send((done, total));
            }
        })
        .map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| AppError::Other(e.to_string()))?
}

/// Starts the verified installer (progress window only; NexaTerm restarts afterwards) and quits.
#[tauri::command]
pub fn update_install(app: tauri::AppHandle, path: String) -> AppResult<()> {
    crate::updater::run_installer(std::path::Path::new(&path))?;
    log::info!("starting update installer {path}; exiting");
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(400));
        app.exit(0);
    });
    Ok(())
}

#[tauri::command]
pub fn open_release_page(url: String) -> AppResult<()> {
    crate::updater::open_page(&url)
}
