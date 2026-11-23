use crate::credentials::SecretStore;
use crate::database;
use crate::error::AppResult;
use crate::ssh::{Db, SshContext};
use crate::terminal::TerminalManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util_like::CancelFlag;

pub struct AppState {
    pub db: Db,
    pub secrets: Arc<dyn SecretStore>,
    pub terminals: TerminalManager,
    pub sftp: crate::sftp::SftpManager,
    pub tunnels: crate::tunnels::TunnelManager,
    pub data_dir: PathBuf,
    /// Cancellation flags for long-running jobs (network tools, transfers).
    pub jobs: Mutex<HashMap<String, CancelFlag>>,
    /// SSH connection behind each SSH terminal, so SFTP can reuse it without re-authenticating.
    pub ssh_conns: Mutex<HashMap<String, Arc<crate::ssh::SshConnection>>>,
    pub portable: bool,
    /// Message for the user when the preferred data location was unusable.
    pub data_notice: Option<String>,
}

impl AppState {
    pub fn new(data_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> AppResult<Self> {
        let conn = database::open(&data_dir.join("nexaterm.db"))?;
        Ok(Self::with_connection(conn, data_dir, secrets))
    }

    pub fn with_connection(conn: rusqlite::Connection, data_dir: PathBuf, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            db: Arc::new(Mutex::new(conn)),
            secrets,
            terminals: TerminalManager::default(),
            sftp: Default::default(),
            tunnels: Default::default(),
            data_dir,
            jobs: Mutex::new(HashMap::new()),
            ssh_conns: Mutex::new(HashMap::new()),
            portable: false,
            data_notice: None,
        }
    }

    pub fn ssh_ctx(&self) -> SshContext {
        SshContext { db: self.db.clone(), secrets: self.secrets.clone() }
    }

    pub fn new_job(&self, id: &str) -> CancelFlag {
        let f = CancelFlag::default();
        self.jobs.lock().unwrap().insert(id.to_string(), f.clone());
        f
    }

    pub fn cancel_job(&self, id: &str) -> bool {
        match self.jobs.lock().unwrap().remove(id) {
            Some(f) => {
                f.cancel();
                true
            }
            None => false,
        }
    }

    pub fn finish_job(&self, id: &str) {
        self.jobs.lock().unwrap().remove(id);
    }
}

/// Minimal cooperative cancellation primitive (avoids pulling tokio-util for one type).
pub mod tokio_util_like {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::Notify;

    #[derive(Clone, Default)]
    pub struct CancelFlag(Arc<(AtomicBool, Notify)>);

    impl CancelFlag {
        pub fn cancel(&self) {
            self.0 .0.store(true, Ordering::SeqCst);
            self.0 .1.notify_waiters();
        }
        pub fn is_cancelled(&self) -> bool {
            self.0 .0.load(Ordering::SeqCst)
        }
        pub async fn cancelled(&self) {
            loop {
                let n = self.0 .1.notified();
                if self.is_cancelled() {
                    return;
                }
                n.await;
            }
        }
    }
}
