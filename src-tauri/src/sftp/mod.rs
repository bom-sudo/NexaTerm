//! SFTP file manager backend (russh-sftp over an existing SSH connection).

use crate::error::{AppError, AppResult};
use crate::security::{validate_file_name, validate_remote_path};
use crate::ssh::SshConnection;
use crate::state::tokio_util_like::CancelFlag;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CHUNK: usize = 256 * 1024;
/// The built-in editor is for config/text files, not arbitrary blobs.
pub const MAX_EDIT_BYTES: u64 = 5 * 1024 * 1024;

pub struct SftpHandle {
    pub sftp: SftpSession,
    pub conn: Arc<SshConnection>,
}

#[derive(Default)]
pub struct SftpManager {
    sessions: Mutex<HashMap<String, Arc<SftpHandle>>>,
}

impl SftpManager {
    pub async fn open(&self, conn: Arc<SshConnection>) -> AppResult<String> {
        let ch = conn.handle.channel_open_session().await?;
        ch.request_subsystem(true, "sftp").await?;
        let sftp = SftpSession::new(ch.into_stream())
            .await
            .map_err(|e| AppError::Sftp(format!("SFTP subsystem unavailable: {e}")))?;
        let id = uuid::Uuid::new_v4().to_string();
        self.sessions.lock().unwrap().insert(id.clone(), Arc::new(SftpHandle { sftp, conn }));
        Ok(id)
    }

    pub fn get(&self, id: &str) -> AppResult<Arc<SftpHandle>> {
        self.sessions.lock().unwrap().get(id).cloned().ok_or_else(|| AppError::NotFound("SFTP session (reconnect)".into()))
    }

    pub async fn close(&self, id: &str) {
        let h = self.sessions.lock().unwrap().remove(id);
        if let Some(h) = h {
            let _ = h.sftp.close().await;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<u32>,
    pub permissions: String,
    pub mode: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
}

pub fn join_remote(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

pub fn parent_remote(path: &str) -> String {
    let t = path.trim_end_matches('/');
    match t.rfind('/') {
        Some(0) | None => "/".into(),
        Some(i) => t[..i].into(),
    }
}

fn perm_string(attrs: &FileAttributes) -> String {
    let kind = if attrs.is_dir() { 'd' } else if attrs.is_symlink() { 'l' } else { '-' };
    format!("{kind}{}", attrs.permissions())
}

impl SftpHandle {
    pub async fn home(&self) -> AppResult<String> {
        Ok(self.sftp.canonicalize(".").await?.to_string())
    }

    pub async fn list(&self, dir: &str) -> AppResult<Vec<RemoteEntry>> {
        validate_remote_path(dir)?;
        let rd = self.sftp.read_dir(dir).await?;
        let mut out = Vec::new();
        for e in rd {
            let name = e.file_name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            let path = join_remote(dir, &name);
            let mut attrs = e.metadata();
            let is_symlink = attrs.is_symlink();
            if is_symlink {
                // Follow symlinks so linked directories are navigable.
                if let Ok(t) = self.sftp.metadata(path.clone()).await {
                    attrs = FileAttributes { ..t };
                }
            }
            out.push(RemoteEntry {
                name,
                path,
                is_dir: attrs.is_dir(),
                is_symlink,
                size: attrs.size.unwrap_or(0),
                modified: attrs.mtime,
                permissions: perm_string(&attrs),
                mode: attrs.permissions,
                owner: attrs.user.clone().or(attrs.uid.map(|u| u.to_string())),
                group: attrs.group.clone().or(attrs.gid.map(|g| g.to_string())),
            });
        }
        out.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        Ok(out)
    }

    pub async fn exists(&self, path: &str) -> AppResult<bool> {
        Ok(self.sftp.try_exists(validate_remote_path(path)?).await?)
    }

    pub async fn mkdir(&self, dir: &str, name: &str) -> AppResult<String> {
        let p = join_remote(validate_remote_path(dir)?, validate_file_name(name)?);
        self.sftp.create_dir(p.clone()).await?;
        Ok(p)
    }

    pub async fn create_file(&self, dir: &str, name: &str) -> AppResult<String> {
        let p = join_remote(validate_remote_path(dir)?, validate_file_name(name)?);
        if self.sftp.try_exists(p.clone()).await? {
            return Err(AppError::Validation(format!("'{name}' already exists")));
        }
        let mut f = self.sftp.create(p.clone()).await?;
        f.shutdown().await.ok();
        Ok(p)
    }

    pub async fn rename(&self, from: &str, new_name: &str) -> AppResult<String> {
        validate_remote_path(from)?;
        let to = join_remote(&parent_remote(from), validate_file_name(new_name)?);
        if self.sftp.try_exists(to.clone()).await? {
            return Err(AppError::Validation(format!("'{new_name}' already exists")));
        }
        self.sftp.rename(from, to.clone()).await?;
        Ok(to)
    }

    /// Delete a file, or a directory recursively. The UI confirms before calling this.
    pub fn delete<'a>(&'a self, path: &'a str) -> futures::future::BoxFuture<'a, AppResult<()>> {
        Box::pin(async move {
            validate_remote_path(path)?;
            if path == "/" || path.trim_end_matches('/').is_empty() {
                return Err(AppError::Validation("Refusing to delete the root directory".into()));
            }
            let meta = self.sftp.symlink_metadata(path).await?;
            if meta.is_dir() {
                for e in self.sftp.read_dir(path).await? {
                    let name = e.file_name().to_string();
                    if name == "." || name == ".." {
                        continue;
                    }
                    self.delete(&join_remote(path, &name)).await?;
                }
                self.sftp.remove_dir(path).await?;
            } else {
                self.sftp.remove_file(path).await?;
            }
            Ok(())
        })
    }

    pub async fn read_text(&self, path: &str) -> AppResult<(String, Option<u32>)> {
        let meta = self.sftp.metadata(validate_remote_path(path)?).await?;
        if meta.is_dir() {
            return Err(AppError::Validation("Cannot edit a directory".into()));
        }
        if meta.size.unwrap_or(0) > MAX_EDIT_BYTES {
            return Err(AppError::Validation("File is larger than 5 MB; download it instead".into()));
        }
        let data = self.sftp.read(path).await?;
        let text = String::from_utf8(data).map_err(|_| AppError::Validation("File is not valid UTF-8 text".into()))?;
        Ok((text, meta.permissions))
    }

    /// Safe save: write a temporary sibling, copy permissions, then atomically replace.
    pub async fn write_text_atomic(&self, path: &str, content: &str) -> AppResult<()> {
        validate_remote_path(path)?;
        let dir = parent_remote(path);
        let name = path.rsplit('/').next().unwrap_or("file");
        let tmp = join_remote(&dir, &format!(".{name}.nexaterm-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]));
        let orig_perm = self.sftp.metadata(path).await.ok().and_then(|m| m.permissions);
        {
            let mut f = self.sftp.create(tmp.clone()).await?;
            if let Err(e) = async {
                f.write_all(content.as_bytes()).await?;
                f.flush().await?;
                f.shutdown().await
            }
            .await
            {
                let _ = self.sftp.remove_file(tmp.clone()).await;
                return Err(AppError::Sftp(format!("upload failed: {e}")));
            }
        }
        if let Some(p) = orig_perm {
            let attrs = FileAttributes { permissions: Some(p & 0o7777), ..FileAttributes::empty() };
            let _ = self.sftp.set_metadata(tmp.clone(), attrs).await;
        }
        self.replace(&tmp, path).await
    }

    /// Move `tmp` over `dest`: posix-rename when the server supports it, otherwise a
    /// backup-rename dance so the original is never lost if a step fails.
    async fn replace(&self, tmp: &str, dest: &str) -> AppResult<()> {
        if let Ok(true) = self.sftp.posix_rename(tmp, dest).await {
            return Ok(());
        }
        if !self.sftp.try_exists(dest).await? {
            self.sftp.rename(tmp, dest).await?;
            return Ok(());
        }
        let backup = format!("{dest}.nexaterm-bak");
        self.sftp.rename(dest, backup.clone()).await?;
        if let Err(e) = self.sftp.rename(tmp, dest).await {
            let _ = self.sftp.rename(backup, dest).await;
            return Err(e.into());
        }
        let _ = self.sftp.remove_file(backup).await;
        Ok(())
    }

    /// Stream a local file to `remote_dir/<name>`. Data goes to a hidden partial file that is
    /// renamed into place only after the full upload succeeded.
    pub async fn upload(&self, local: &Path, remote_dir: &str, overwrite: bool, cancel: &CancelFlag, mut progress: impl FnMut(u64, u64)) -> AppResult<String> {
        let name = local.file_name().and_then(|n| n.to_str()).ok_or_else(|| AppError::Validation("Invalid local file".into()))?;
        let dest = join_remote(validate_remote_path(remote_dir)?, name);
        if !overwrite && self.sftp.try_exists(dest.clone()).await? {
            return Err(AppError::Validation(format!("'{name}' already exists on the server")));
        }
        let mut src = tokio::fs::File::open(local).await?;
        let total = src.metadata().await?.len();
        let part = join_remote(remote_dir, &format!(".{name}.nexaterm-part"));
        let mut dst = self.sftp.open_with_flags(part.clone(), OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE).await?;
        let mut buf = vec![0u8; CHUNK];
        let mut done = 0u64;
        let res: AppResult<()> = async {
            loop {
                if cancel.is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                let n = src.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                dst.write_all(&buf[..n]).await.map_err(|e| AppError::Sftp(e.to_string()))?;
                done += n as u64;
                progress(done, total);
            }
            dst.flush().await.map_err(|e| AppError::Sftp(e.to_string()))?;
            dst.shutdown().await.map_err(|e| AppError::Sftp(e.to_string()))?;
            Ok(())
        }
        .await;
        if let Err(e) = res {
            drop(dst);
            let _ = self.sftp.remove_file(part).await;
            return Err(e);
        }
        self.replace(&part, &dest).await?;
        Ok(dest)
    }

    /// Stream a remote file to `local_dir`. Writes `<name>.part` and renames on success.
    pub async fn download(&self, remote: &str, local_dir: &Path, overwrite: bool, cancel: &CancelFlag, mut progress: impl FnMut(u64, u64)) -> AppResult<PathBuf> {
        validate_remote_path(remote)?;
        let name = remote.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        validate_file_name(name)?;
        if name.contains([':', '*', '?', '"', '<', '>', '|']) {
            return Err(AppError::Validation(format!("'{name}' is not a valid Windows file name")));
        }
        let dest = local_dir.join(name);
        if !overwrite && dest.exists() {
            return Err(AppError::Validation(format!("'{name}' already exists locally")));
        }
        let meta = self.sftp.metadata(remote).await?;
        if meta.is_dir() {
            return Err(AppError::Validation("Directory download is not supported yet; download files individually".into()));
        }
        let total = meta.size.unwrap_or(0);
        let part = local_dir.join(format!("{name}.part"));
        let mut src = self.sftp.open(remote).await?;
        let mut dst = tokio::fs::File::create(&part).await?;
        let mut buf = vec![0u8; CHUNK];
        let mut done = 0u64;
        let res: AppResult<()> = async {
            loop {
                if cancel.is_cancelled() {
                    return Err(AppError::Cancelled);
                }
                let n = src.read(&mut buf).await.map_err(|e| AppError::Sftp(e.to_string()))?;
                if n == 0 {
                    break;
                }
                dst.write_all(&buf[..n]).await?;
                done += n as u64;
                progress(done, total);
            }
            dst.flush().await?;
            dst.sync_all().await?;
            Ok(())
        }
        .await;
        drop(dst);
        if let Err(e) = res {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }
        if dest.exists() {
            tokio::fs::remove_file(&dest).await?;
        }
        tokio::fs::rename(&part, &dest).await?;
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_helpers() {
        assert_eq!(join_remote("/home/u", "a.txt"), "/home/u/a.txt");
        assert_eq!(join_remote("/", "etc"), "/etc");
        assert_eq!(parent_remote("/home/u/a.txt"), "/home/u");
        assert_eq!(parent_remote("/home"), "/");
        assert_eq!(parent_remote("/"), "/");
        assert_eq!(parent_remote("/home/u/"), "/home");
    }
}
