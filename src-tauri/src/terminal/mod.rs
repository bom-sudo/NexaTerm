//! Terminal registry. Local PTYs and SSH shells implement [`TerminalBackend`] so the UI, logging,
//! split panes and multi-execution treat them identically.

pub mod local;
pub mod serial;
pub mod telnet;
pub mod shells;

use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub trait TerminalBackend: Send + Sync {
    fn write(&self, data: &[u8]) -> AppResult<()>;
    fn resize(&self, cols: u16, rows: u16) -> AppResult<()>;
    /// Terminate the process / close the channel. Must be idempotent.
    fn close(&self);
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TermEvent {
    /// Backend finished. `code` is the process exit code when known.
    Exit { code: Option<i64>, reason: Option<String> },
    /// SSH connection state change (connecting, connected, reconnecting, lost ...).
    Status { state: String, message: Option<String> },
}

/// Callbacks a backend uses to deliver output. Cloned freely across threads/tasks.
#[derive(Clone)]
pub struct TermSink {
    data: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    event: Arc<dyn Fn(TermEvent) + Send + Sync>,
    log: Arc<Mutex<Option<std::fs::File>>>,
}

impl TermSink {
    pub fn new(data: impl Fn(Vec<u8>) + Send + Sync + 'static, event: impl Fn(TermEvent) + Send + Sync + 'static) -> Self {
        Self { data: Arc::new(data), event: Arc::new(event), log: Arc::new(Mutex::new(None)) }
    }
    pub fn data(&self, bytes: Vec<u8>) {
        if let Ok(mut g) = self.log.lock() {
            if let Some(f) = g.as_mut() {
                if f.write_all(&bytes).is_err() {
                    *g = None; // stop logging on disk errors rather than failing the terminal
                }
            }
        }
        (self.data)(bytes)
    }
    pub fn event(&self, ev: TermEvent) {
        (self.event)(ev)
    }
}

pub struct TerminalEntry {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub backend: Arc<dyn TerminalBackend>,
    pub sink: TermSink,
    pub log_path: Mutex<Option<PathBuf>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub logging: Option<String>,
}

#[derive(Default)]
pub struct TerminalManager {
    terms: Mutex<HashMap<String, Arc<TerminalEntry>>>,
}

impl TerminalManager {
    pub fn insert(&self, id: String, kind: &str, label: &str, backend: Arc<dyn TerminalBackend>, sink: TermSink) {
        let e = Arc::new(TerminalEntry { id: id.clone(), kind: kind.into(), label: label.into(), backend, sink, log_path: Mutex::new(None) });
        self.terms.lock().unwrap().insert(id, e);
    }

    pub fn get(&self, id: &str) -> AppResult<Arc<TerminalEntry>> {
        self.terms
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("terminal {id}")))
    }

    pub fn write(&self, id: &str, data: &[u8]) -> AppResult<()> {
        self.get(id)?.backend.write(data)
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> AppResult<()> {
        if cols == 0 || rows == 0 || cols > 2000 || rows > 1000 {
            return Err(AppError::Validation("Invalid terminal size".into()));
        }
        self.get(id)?.backend.resize(cols, rows)
    }

    pub fn close(&self, id: &str) {
        let e = self.terms.lock().unwrap().remove(id);
        if let Some(e) = e {
            e.backend.close();
            if let Ok(mut l) = e.sink.log.lock() {
                *l = None;
            }
        }
    }

    pub fn close_all(&self) {
        let all: Vec<_> = self.terms.lock().unwrap().drain().map(|(_, v)| v).collect();
        for e in all {
            e.backend.close();
        }
    }

    pub fn list(&self) -> Vec<TerminalInfo> {
        self.terms
            .lock()
            .unwrap()
            .values()
            .map(|e| TerminalInfo {
                id: e.id.clone(),
                kind: e.kind.clone(),
                label: e.label.clone(),
                logging: e.log_path.lock().unwrap().as_ref().map(|p| p.display().to_string()),
            })
            .collect()
    }

    /// Start appending raw terminal output to `path` (ANSI sequences preserved).
    pub fn start_log(&self, id: &str, path: PathBuf) -> AppResult<()> {
        let e = self.get(id)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        *e.sink.log.lock().unwrap() = Some(f);
        *e.log_path.lock().unwrap() = Some(path);
        Ok(())
    }

    pub fn stop_log(&self, id: &str) -> AppResult<Option<PathBuf>> {
        let e = self.get(id)?;
        *e.sink.log.lock().unwrap() = None;
        let path = e.log_path.lock().unwrap().take();
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake(Arc<AtomicUsize>, Mutex<Vec<u8>>);
    impl TerminalBackend for Fake {
        fn write(&self, d: &[u8]) -> AppResult<()> {
            self.1.lock().unwrap().extend_from_slice(d);
            Ok(())
        }
        fn resize(&self, _: u16, _: u16) -> AppResult<()> {
            Ok(())
        }
        fn close(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn manager_lifecycle_and_logging() {
        let closed = Arc::new(AtomicUsize::new(0));
        let m = TerminalManager::default();
        let sink = TermSink::new(|_| {}, |_| {});
        m.insert("t1".into(), "local", "PS", Arc::new(Fake(closed.clone(), Mutex::new(vec![]))), sink.clone());
        m.write("t1", b"ls\r").unwrap();
        assert!(m.resize("t1", 0, 10).is_err());
        m.resize("t1", 120, 40).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("logs/t1.log");
        m.start_log("t1", p.clone()).unwrap();
        sink.data(b"hello\r\n".to_vec());
        assert_eq!(m.stop_log("t1").unwrap(), Some(p.clone()));
        sink.data(b"not logged".to_vec());
        assert_eq!(std::fs::read(&p).unwrap(), b"hello\r\n");

        m.close("t1");
        m.close("t1");
        assert_eq!(closed.load(Ordering::SeqCst), 1);
        assert!(m.write("t1", b"x").is_err());
    }
}
