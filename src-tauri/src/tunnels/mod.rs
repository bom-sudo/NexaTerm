//! SSH port forwarding. Tunnels only run when the user presses Start; nothing is auto-created.

pub mod socks;

use crate::database::models::TunnelProfile;
use crate::error::{AppError, AppResult};
use crate::ssh::SshConnection;
use crate::state::tokio_util_like::CancelFlag;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStatus {
    pub id: String,
    pub state: String,
    pub active_connections: u64,
    pub total_connections: u64,
    pub error: Option<String>,
}

struct Active {
    cancel: CancelFlag,
    conn: Arc<SshConnection>,
    kind: String,
    bind_address: String,
    bind_port: u16,
    active: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    error: Arc<Mutex<Option<String>>>,
}

#[derive(Default)]
pub struct TunnelManager {
    active: Mutex<HashMap<String, Active>>,
}

/// Check that a local port can be bound, giving a clear message for conflicts.
pub async fn bind_listener(addr: &str, port: u16) -> AppResult<TcpListener> {
    TcpListener::bind((addr, port)).await.map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => AppError::Validation(format!("Local port {port} is already in use on {addr}")),
        std::io::ErrorKind::PermissionDenied => AppError::Validation(format!("Not allowed to listen on {addr}:{port} (reserved or excluded port range)")),
        _ => AppError::Io(format!("Cannot listen on {addr}:{port}: {e}")),
    })
}

impl TunnelManager {
    pub fn is_running(&self, id: &str) -> bool {
        self.active.lock().unwrap().contains_key(id)
    }

    pub async fn start(&self, p: &TunnelProfile, conn: Arc<SshConnection>) -> AppResult<()> {
        if self.is_running(&p.id) {
            return Err(AppError::Validation("Tunnel is already running".into()));
        }
        let cancel = CancelFlag::default();
        let active = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(0));
        let error = Arc::new(Mutex::new(None));

        match p.kind.as_str() {
            "local" | "dynamic" => {
                let listener = bind_listener(&p.bind_address, p.bind_port).await?;
                let (c, a, t, err) = (cancel.clone(), active.clone(), total.clone(), error.clone());
                let conn2 = conn.clone();
                let dynamic = p.kind == "dynamic";
                let dest = (p.dest_host.clone().unwrap_or_default(), p.dest_port.unwrap_or(0));
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            _ = c.cancelled() => break,
                            acc = listener.accept() => {
                                let (mut sock, peer) = match acc { Ok(v) => v, Err(e) => { *err.lock().unwrap() = Some(e.to_string()); continue; } };
                                let conn = conn2.clone();
                                let (a, t, err, c2) = (a.clone(), t.clone(), err.clone(), c.clone());
                                let dest = dest.clone();
                                tokio::spawn(async move {
                                    a.fetch_add(1, Ordering::SeqCst);
                                    t.fetch_add(1, Ordering::SeqCst);
                                    let target = if dynamic {
                                        socks::handshake(&mut sock).await.map_err(|e| e.to_string())
                                    } else {
                                        Ok(dest)
                                    };
                                    let res = match target {
                                        Ok((host, port)) => match conn.handle.channel_open_direct_tcpip(host.clone(), port as u32, peer.ip().to_string(), peer.port() as u32).await {
                                            Ok(ch) => {
                                                if dynamic { let _ = socks::reply_ok(&mut sock).await; }
                                                let mut stream = ch.into_stream();
                                                tokio::select! {
                                                    _ = c2.cancelled() => Ok(()),
                                                    r = tokio::io::copy_bidirectional(&mut sock, &mut stream) => r.map(|_| ()).map_err(|e| e.to_string()),
                                                }
                                            }
                                            Err(e) => {
                                                if dynamic { let _ = socks::reply_fail(&mut sock).await; }
                                                Err(format!("SSH server could not open {host}:{port}: {e}"))
                                            }
                                        },
                                        Err(e) => Err(e),
                                    };
                                    if let Err(e) = res { *err.lock().unwrap() = Some(e); }
                                    a.fetch_sub(1, Ordering::SeqCst);
                                });
                            }
                        }
                    }
                });
            }
            "remote" => {
                let dest_host = p.dest_host.clone().ok_or_else(|| AppError::Validation("Destination host required".into()))?;
                let dest_port = p.dest_port.ok_or_else(|| AppError::Validation("Destination port required".into()))?;
                conn.remote_forwards.lock().unwrap().insert(p.bind_port as u32, (dest_host, dest_port));
                let r = conn.handle.tcpip_forward(p.bind_address.clone(), p.bind_port as u32).await;
                if let Err(e) = r {
                    conn.remote_forwards.lock().unwrap().remove(&(p.bind_port as u32));
                    return Err(AppError::Connection(format!("Server refused remote forward on port {}: {e}", p.bind_port)));
                }
            }
            other => return Err(AppError::Validation(format!("Unknown tunnel type {other}"))),
        }

        self.active.lock().unwrap().insert(
            p.id.clone(),
            Active { cancel, conn, kind: p.kind.clone(), bind_address: p.bind_address.clone(), bind_port: p.bind_port, active, total, error },
        );
        Ok(())
    }

    pub async fn stop(&self, id: &str) -> AppResult<()> {
        let a = self.active.lock().unwrap().remove(id);
        if let Some(a) = a {
            a.cancel.cancel();
            if a.kind == "remote" {
                let _ = a.conn.handle.cancel_tcpip_forward(a.bind_address.clone(), a.bind_port as u32).await;
                a.conn.remote_forwards.lock().unwrap().remove(&(a.bind_port as u32));
            }
            a.conn.disconnect().await;
        }
        Ok(())
    }

    pub async fn stop_all(&self) {
        let ids: Vec<String> = self.active.lock().unwrap().keys().cloned().collect();
        for id in ids {
            let _ = self.stop(&id).await;
        }
    }

    pub fn statuses(&self) -> Vec<TunnelStatus> {
        self.active
            .lock()
            .unwrap()
            .iter()
            .map(|(id, a)| TunnelStatus {
                id: id.clone(),
                state: if a.conn.is_closed() { "lost".into() } else { "running".into() },
                active_connections: a.active.load(Ordering::SeqCst),
                total_connections: a.total.load(Ordering::SeqCst),
                error: a.error.lock().unwrap().clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn detects_port_conflict() {
        let l = bind_listener("127.0.0.1", 0).await.unwrap();
        let port = l.local_addr().unwrap().port();
        match bind_listener("127.0.0.1", port).await {
            Err(AppError::Validation(m)) => assert!(m.contains("already in use"), "{m}"),
            Err(e) => panic!("unexpected error {e:?}"),
            Ok(_) => {} // some Windows configs allow SO_REUSEADDR-like rebinding; not a failure of ours
        }
    }
}
