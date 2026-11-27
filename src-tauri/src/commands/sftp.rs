use super::S;
use crate::database::repo;
use crate::error::{AppError, AppResult};
use crate::sftp::RemoteEntry;
use crate::ssh::{self, Secrets};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::ipc::Channel;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpOpenReq {
    /// Reuse the connection of an open SSH terminal …
    pub terminal_id: Option<String>,
    /// … or open a dedicated connection for a saved session.
    pub session_id: Option<String>,
    pub password: Option<String>,
    pub passphrase: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpOpened {
    pub sftp_id: String,
    pub home: String,
    pub label: String,
}

#[tauri::command]
pub async fn sftp_open(state: S<'_>, req: SftpOpenReq) -> AppResult<SftpOpened> {
    let existing = req.terminal_id.as_ref().and_then(|t| state.ssh_conns.lock().unwrap().get(t).cloned());
    let conn = match (existing, &req.session_id) {
        (Some(c), _) if !c.is_closed() => c,
        (_, Some(sid)) => {
            let session = { repo::get_session(&state.db.lock().unwrap(), sid)? };
            ssh::connect(&state.ssh_ctx(), &session, &Secrets { password: req.password, passphrase: req.passphrase }, 0).await?
        }
        _ => return Err(AppError::Validation("No open SSH connection; connect the session first".into())),
    };
    let label = conn.label.clone();
    let id = state.sftp.open(conn).await?;
    let home = state.sftp.get(&id)?.home().await.unwrap_or_else(|_| "/".into());
    Ok(SftpOpened { sftp_id: id, home, label })
}

#[tauri::command]
pub async fn sftp_close(state: S<'_>, sftp_id: String) -> AppResult<()> {
    state.sftp.close(&sftp_id).await;
    Ok(())
}

#[tauri::command]
pub async fn sftp_list(state: S<'_>, sftp_id: String, path: String) -> AppResult<Vec<RemoteEntry>> {
    state.sftp.get(&sftp_id)?.list(&path).await
}

#[tauri::command]
pub async fn sftp_realpath(state: S<'_>, sftp_id: String, path: String) -> AppResult<String> {
    let h = state.sftp.get(&sftp_id)?;
    Ok(h.sftp.canonicalize(path).await?.to_string())
}

#[tauri::command]
pub async fn sftp_exists(state: S<'_>, sftp_id: String, path: String) -> AppResult<bool> {
    state.sftp.get(&sftp_id)?.exists(&path).await
}

#[tauri::command]
pub async fn sftp_mkdir(state: S<'_>, sftp_id: String, dir: String, name: String) -> AppResult<String> {
    state.sftp.get(&sftp_id)?.mkdir(&dir, &name).await
}

#[tauri::command]
pub async fn sftp_create_file(state: S<'_>, sftp_id: String, dir: String, name: String) -> AppResult<String> {
    state.sftp.get(&sftp_id)?.create_file(&dir, &name).await
}

#[tauri::command]
pub async fn sftp_rename(state: S<'_>, sftp_id: String, path: String, new_name: String) -> AppResult<String> {
    state.sftp.get(&sftp_id)?.rename(&path, &new_name).await
}

#[tauri::command]
pub async fn sftp_delete(state: S<'_>, sftp_id: String, path: String) -> AppResult<()> {
    state.sftp.get(&sftp_id)?.delete(&path).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFile {
    pub content: String,
    pub mode: Option<u32>,
}

#[tauri::command]
pub async fn sftp_read_text(state: S<'_>, sftp_id: String, path: String) -> AppResult<TextFile> {
    let (content, mode) = state.sftp.get(&sftp_id)?.read_text(&path).await?;
    Ok(TextFile { content, mode })
}

#[tauri::command]
pub async fn sftp_write_text(state: S<'_>, sftp_id: String, path: String, content: String) -> AppResult<()> {
    state.sftp.get(&sftp_id)?.write_text_atomic(&path, &content).await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

/// Throttle progress messages to ~10/s so large transfers do not flood IPC.
fn throttled(ch: Channel<Progress>) -> impl FnMut(u64, u64) {
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    move |done, total| {
        if done == total || last.elapsed().as_millis() >= 100 {
            last = std::time::Instant::now();
            let _ = ch.send(Progress { done, total });
        }
    }
}

#[tauri::command]
pub async fn sftp_upload(state: S<'_>, sftp_id: String, local_path: String, remote_dir: String, overwrite: bool, job_id: String, on_progress: Channel<Progress>) -> AppResult<String> {
    let h = state.sftp.get(&sftp_id)?;
    let local = PathBuf::from(&local_path);
    if !local.is_file() {
        return Err(AppError::Validation("Only regular files can be uploaded (folders are not supported yet)".into()));
    }
    let cancel = state.new_job(&job_id);
    let r = h.upload(&local, &remote_dir, overwrite, &cancel, throttled(on_progress)).await;
    state.finish_job(&job_id);
    r
}

#[tauri::command]
pub async fn sftp_download(state: S<'_>, sftp_id: String, remote_path: String, local_dir: String, overwrite: bool, job_id: String, on_progress: Channel<Progress>) -> AppResult<String> {
    let h = state.sftp.get(&sftp_id)?;
    let dir = PathBuf::from(&local_dir);
    if !dir.is_dir() {
        return Err(AppError::Validation("Download folder does not exist".into()));
    }
    let cancel = state.new_job(&job_id);
    let r = h.download(&remote_path, &dir, overwrite, &cancel, throttled(on_progress)).await;
    state.finish_job(&job_id);
    r.map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn sftp_upload_dir(state: S<'_>, sftp_id: String, local_path: String, remote_dir: String, overwrite: bool, job_id: String, on_progress: Channel<Progress>) -> AppResult<crate::sftp::FolderTransfer> {
    let h = state.sftp.get(&sftp_id)?;
    let local = PathBuf::from(&local_path);
    if !local.is_dir() {
        return Err(AppError::Validation("Local folder does not exist".into()));
    }
    let cancel = state.new_job(&job_id);
    let r = h.upload_dir(&local, &remote_dir, overwrite, &cancel, throttled(on_progress)).await;
    state.finish_job(&job_id);
    r
}

#[tauri::command]
pub async fn sftp_download_dir(state: S<'_>, sftp_id: String, remote_path: String, local_dir: String, overwrite: bool, job_id: String, on_progress: Channel<Progress>) -> AppResult<crate::sftp::FolderTransfer> {
    let h = state.sftp.get(&sftp_id)?;
    let dir = PathBuf::from(&local_dir);
    if !dir.is_dir() {
        return Err(AppError::Validation("Download folder does not exist".into()));
    }
    let cancel = state.new_job(&job_id);
    let r = h.download_dir(&remote_path, &dir, overwrite, &cancel, throttled(on_progress)).await;
    state.finish_job(&job_id);
    r
}

#[tauri::command]
pub async fn sftp_stat(state: S<'_>, sftp_id: String, path: String) -> AppResult<RemoteEntry> {
    state.sftp.get(&sftp_id)?.stat(&path).await
}

#[tauri::command]
pub async fn sftp_chmod(state: S<'_>, sftp_id: String, path: String, mode: u32, recursive: bool) -> AppResult<u32> {
    state.sftp.get(&sftp_id)?.chmod(&path, mode, recursive).await
}

#[tauri::command]
pub async fn sftp_chown(state: S<'_>, sftp_id: String, path: String, uid: Option<u32>, gid: Option<u32>) -> AppResult<()> {
    state.sftp.get(&sftp_id)?.chown(&path, uid, gid).await
}

#[tauri::command]
pub fn local_is_dir(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

#[tauri::command]
pub fn local_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

#[tauri::command]
pub fn downloads_dir() -> String {
    dirs::download_dir().or_else(dirs::home_dir).map(|p| p.display().to_string()).unwrap_or_default()
}
