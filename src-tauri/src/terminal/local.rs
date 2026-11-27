//! Local shell backend using Windows ConPTY (via portable-pty).

use super::{TermEvent, TermSink, TerminalBackend};
use crate::error::{AppError, AppResult};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub struct LocalSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

pub struct LocalPty {
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl LocalPty {
    /// Spawn `program` with structured arguments (never via a shell string).
    pub fn spawn(spec: LocalSpec, sink: TermSink) -> AppResult<Arc<Self>> {
        if spec.program.trim().is_empty() {
            return Err(AppError::Validation("Shell executable is required".into()));
        }
        let pty = native_pty_system();
        let pair = pty
            .openpty(PtySize { rows: spec.rows.max(1), cols: spec.cols.max(1), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| AppError::Terminal(format!("Cannot create pseudo console: {e}")))?;

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        let cwd = spec
            .cwd
            .filter(|c| !c.trim().is_empty() && std::path::Path::new(c).is_dir())
            .or_else(|| dirs::home_dir().map(|h| h.display().to_string()));
        if let Some(c) = cwd {
            cmd.cwd(c);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AppError::Terminal(format!("Cannot start '{}': {e}", spec.program)))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| AppError::Terminal(e.to_string()))?;
        let writer = pair.master.take_writer().map_err(|e| AppError::Terminal(e.to_string()))?;
        let killer = child.clone_killer();
        let master = Arc::new(Mutex::new(Some(pair.master)));

        // Reader thread: blocking reads off the UI thread; raw bytes go to xterm.js which handles
        // UTF-8 sequences split across chunks.
        let rsink = sink.clone();
        std::thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => rsink.data(buf[..n].to_vec()),
                    }
                }
            })
            .map_err(|e| AppError::Terminal(e.to_string()))?;

        // Waiter thread: ConPTY does not EOF the reader while the master is alive, so drop the
        // master once the child exits.
        let wmaster = master.clone();
        std::thread::Builder::new()
            .name("pty-waiter".into())
            .spawn(move || {
                let status = child.wait();
                std::thread::sleep(std::time::Duration::from_millis(150)); // let final output drain
                wmaster.lock().unwrap().take();
                let code = status.ok().map(|s| s.exit_code() as i64);
                sink.event(TermEvent::Exit { code, reason: None });
            })
            .map_err(|e| AppError::Terminal(e.to_string()))?;

        Ok(Arc::new(Self { master, writer: Mutex::new(Some(writer)), killer: Mutex::new(killer) }))
    }
}

impl TerminalBackend for LocalPty {
    fn write(&self, data: &[u8]) -> AppResult<()> {
        let mut g = self.writer.lock().unwrap();
        let w = g.as_mut().ok_or_else(|| AppError::Terminal("Terminal is closed".into()))?;
        w.write_all(data)?;
        w.flush()?;
        Ok(())
    }

    fn resize(&self, cols: u16, rows: u16) -> AppResult<()> {
        if let Some(m) = self.master.lock().unwrap().as_ref() {
            m.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                .map_err(|e| AppError::Terminal(e.to_string()))?;
        }
        Ok(())
    }

    fn close(&self) {
        let _ = self.killer.lock().unwrap().kill();
        self.writer.lock().unwrap().take();
        self.master.lock().unwrap().take();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn cmd_runs_echoes_resizes_and_exits() {
        let (dtx, drx) = mpsc::channel::<Vec<u8>>();
        let (etx, erx) = mpsc::channel::<TermEvent>();
        let dtx = Mutex::new(dtx);
        let etx = Mutex::new(etx);
        let sink = TermSink::new(move |d| { let _ = dtx.lock().unwrap().send(d); }, move |e| { let _ = etx.lock().unwrap().send(e); });
        let pty = LocalPty::spawn(
            LocalSpec { program: "cmd.exe".into(), args: vec!["/Q".into(), "/K".into()], cwd: None, cols: 80, rows: 24 },
            sink,
        )
        .unwrap();
        pty.resize(100, 30).unwrap();
        pty.write(b"echo NEXA_%COMPUTERNAME:~0,0%OK\r\n").unwrap();
        let mut out = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut answered = false;
        while std::time::Instant::now() < deadline {
            if let Ok(d) = drx.recv_timeout(Duration::from_millis(200)) {
                out.extend(d);
                // ConPTY asks for the cursor position (DSR) and blocks until answered; xterm.js
                // replies automatically in the app, the test must do it itself.
                if !answered && String::from_utf8_lossy(&out).contains("\x1b[6n") {
                    pty.write(b"\x1b[1;1R").unwrap();
                    answered = true;
                }
                if String::from_utf8_lossy(&out).contains("NEXA_OK") {
                    break;
                }
            }
        }
        assert!(String::from_utf8_lossy(&out).contains("NEXA_OK"), "output: {}", String::from_utf8_lossy(&out));
        pty.write(b"exit 3\r\n").unwrap();
        match erx.recv_timeout(Duration::from_secs(15)).unwrap() {
            TermEvent::Exit { code, .. } => assert_eq!(code, Some(3)),
            other => panic!("unexpected {other:?}"),
        }
        pty.close();
        assert!(pty.write(b"x").is_err());
    }

    #[test]
    fn close_kills_process() {
        let (etx, erx) = mpsc::channel::<TermEvent>();
        let etx = Mutex::new(etx);
        let sink = TermSink::new(|_| {}, move |e| { let _ = etx.lock().unwrap().send(e); });
        let pty = LocalPty::spawn(LocalSpec { program: "cmd.exe".into(), args: vec![], cwd: None, cols: 80, rows: 24 }, sink).unwrap();
        pty.close();
        assert!(matches!(erx.recv_timeout(Duration::from_secs(10)), Ok(TermEvent::Exit { .. })));
    }
}

#[cfg(all(test, unix))]
mod unix_tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn sh_runs_echoes_resizes_and_exits() {
        let (dtx, drx) = mpsc::channel::<Vec<u8>>();
        let (etx, erx) = mpsc::channel::<TermEvent>();
        let dtx = Mutex::new(dtx);
        let etx = Mutex::new(etx);
        let sink = TermSink::new(move |d| { let _ = dtx.lock().unwrap().send(d); }, move |e| { let _ = etx.lock().unwrap().send(e); });
        let pty = LocalPty::spawn(LocalSpec { program: "/bin/sh".into(), args: vec![], cwd: None, cols: 80, rows: 24 }, sink).unwrap();
        pty.resize(100, 30).unwrap();
        pty.write(b"echo NEXA_$((40+2))_OK\n").unwrap();
        let mut out = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while std::time::Instant::now() < deadline && !String::from_utf8_lossy(&out).contains("NEXA_42_OK") {
            if let Ok(d) = drx.recv_timeout(Duration::from_millis(200)) {
                out.extend(d);
            }
        }
        assert!(String::from_utf8_lossy(&out).contains("NEXA_42_OK"), "output: {}", String::from_utf8_lossy(&out));
        pty.write(b"exit 3\n").unwrap();
        match erx.recv_timeout(Duration::from_secs(15)).unwrap() {
            TermEvent::Exit { code, .. } => assert_eq!(code, Some(3)),
            other => panic!("unexpected {other:?}"),
        }
        pty.close();
        assert!(pty.write(b"x").is_err());
    }

    #[test]
    fn close_kills_process() {
        let (etx, erx) = mpsc::channel::<TermEvent>();
        let etx = Mutex::new(etx);
        let sink = TermSink::new(|_| {}, move |e| { let _ = etx.lock().unwrap().send(e); });
        let pty = LocalPty::spawn(LocalSpec { program: "/bin/sh".into(), args: vec![], cwd: None, cols: 80, rows: 24 }, sink).unwrap();
        pty.close();
        assert!(matches!(erx.recv_timeout(Duration::from_secs(10)), Ok(TermEvent::Exit { .. })));
    }
}
