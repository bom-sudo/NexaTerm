use crate::credentials::SecretStore;
use crate::database;
use crate::error::AppResult;
use crate::ssh::{Db, SshContext};
use crate::terminal::TerminalManager;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util_like::CancelFlag;

pub type PendingPrompts = Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Option<Vec<String>>>>>>;

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
    /// Set in setup; used to emit keyboard-interactive prompts (2FA) to the UI.
    pub app: Mutex<Option<tauri::AppHandle>>,
    /// Keyboard-interactive prompts waiting for the user, by prompt id.
    pub pending_prompts: PendingPrompts,
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
            app: Mutex::new(None),
            pending_prompts: Default::default(),
        }
    }

    pub fn ssh_ctx(&self) -> SshContext {
        SshContext { db: self.db.clone(), secrets: self.secrets.clone(), prompter: self.prompter() }
    }

    /// Prompter that shows keyboard-interactive questions (password, OTP ...) in the UI via the
    /// "ssh-prompt" event and waits up to 5 minutes for `ssh_prompt_answer`.
    fn prompter(&self) -> Option<crate::ssh::Prompter> {
        use tauri::Emitter;
        let app = self.app.lock().unwrap().clone()?;
        let pending = self.pending_prompts.clone();
        Some(Arc::new(move |req: crate::ssh::KbdPrompt| {
            let app = app.clone();
            let pending = pending.clone();
            Box::pin(async move {
                let (tx, rx) = tokio::sync::oneshot::channel();
                pending.lock().unwrap().insert(req.id.clone(), tx);
                let id = req.id.clone();
                if app.emit("ssh-prompt", &req).is_err() {
                    pending.lock().unwrap().remove(&id);
                    return None;
                }
                let answer = tokio::time::timeout(std::time::Duration::from_secs(300), rx).await;
                pending.lock().unwrap().remove(&id);
                answer.ok().and_then(|r| r.ok()).flatten()
            })
        }))
    }

    /// Deliver the answers typed by the user (None = cancelled). False for unknown/expired prompts.
    pub fn answer_prompt(&self, id: &str, answers: Option<Vec<String>>) -> bool {
        match self.pending_prompts.lock().unwrap().remove(id) {
            Some(tx) => tx.send(answers).is_ok(),
            None => false,
        }
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
