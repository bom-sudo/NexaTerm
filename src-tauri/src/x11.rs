//! X11 forwarding support: detect a locally installed / running X server.
//! NexaTerm does not ship or download an X server (VcXsrv/Xming are GPL); it only uses one the
//! user installed, and only starts it when the user asks.

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XServer {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XStatus {
    /// Something accepts connections on 127.0.0.1:6000 (display :0).
    pub running: bool,
    pub installed: Vec<XServer>,
}

fn candidates() -> Vec<(&'static str, &'static str, PathBuf)> {
    let pf = std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_default();
    let pf86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from).unwrap_or_default();
    let apps = dirs::data_local_dir().unwrap_or_default().join(r"Microsoft\WindowsApps");
    vec![
        ("vcxsrv", "VcXsrv", pf.join(r"VcXsrv\vcxsrv.exe")),
        ("vcxsrv", "VcXsrv", pf86.join(r"VcXsrv\vcxsrv.exe")),
        ("xming", "Xming", pf.join(r"Xming\Xming.exe")),
        ("xming", "Xming", pf86.join(r"Xming\Xming.exe")),
        ("x410", "X410", apps.join("X410.exe")),
    ]
}

pub fn installed() -> Vec<XServer> {
    let mut seen = std::collections::HashSet::new();
    candidates()
        .into_iter()
        .filter(|(_, _, p)| p.is_file())
        .filter(|(id, _, _)| seen.insert(*id))
        .map(|(id, name, p)| XServer { id: id.into(), name: name.into(), path: p.display().to_string() })
        .collect()
}

pub async fn status() -> XStatus {
    let running = matches!(
        tokio::time::timeout(Duration::from_millis(400), tokio::net::TcpStream::connect(("127.0.0.1", 6000))).await,
        Ok(Ok(_))
    );
    XStatus { running, installed: installed() }
}

/// Start an installed X server on display :0 with TCP enabled and access control disabled
/// (needed for SSH X11 forwarding). Only servers found by `installed()` can be started.
pub fn start(id: &str) -> AppResult<()> {
    let server = installed().into_iter().find(|s| s.id == id).ok_or_else(|| AppError::NotFound(format!("X server '{id}' is not installed")))?;
    let args: &[&str] = match id {
        "vcxsrv" => &[":0", "-multiwindow", "-clipboard", "-wgl", "-ac"],
        "xming" => &[":0", "-multiwindow", "-clipboard", "-ac"],
        _ => &[],
    };
    std::process::Command::new(&server.path)
        .args(args)
        .spawn()
        .map_err(|e| AppError::Io(format!("Cannot start {}: {e}", server.name)))?;
    Ok(())
}
