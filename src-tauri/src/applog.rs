//! Application log: `<data dir>\logs\nexaterm.log`. Every line passes through `security::redact`.
//! Rotated to `nexaterm.log.old` at 5 MB. No log data ever leaves the machine.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_BYTES: u64 = 5 * 1024 * 1024;

struct FileLogger {
    path: PathBuf,
    file: Mutex<Option<std::fs::File>>,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info && !m.target().starts_with("russh") && !m.target().starts_with("tao")
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} {:5} {}: {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            r.level(),
            r.target(),
            crate::security::redact(&r.args().to_string())
        );
        let mut g = self.file.lock().unwrap();
        if let Some(f) = g.as_mut() {
            if f.metadata().map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
                *g = None;
                let _ = std::fs::rename(&self.path, self.path.with_extension("log.old"));
                *g = open(&self.path);
            }
        }
        if let Some(f) = g.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = f.flush();
        }
    }
}

fn open(path: &Path) -> Option<std::fs::File> {
    std::fs::OpenOptions::new().create(true).append(true).open(path).ok()
}

/// Install the file logger. Returns the log path (or None if the directory is unusable).
pub fn init(data_dir: &Path) -> Option<PathBuf> {
    let dir = data_dir.join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("nexaterm.log");
    let file = open(&path)?;
    let logger = FileLogger { path: path.clone(), file: Mutex::new(Some(file)) };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    Some(path)
}
