//! Interactive SSH shell channel exposed as a [`TerminalBackend`].

use super::SshConnection;
use crate::error::{AppError, AppResult};
use crate::terminal::{TermEvent, TermSink, TerminalBackend};
use russh::ChannelMsg;
use std::sync::Arc;
use tokio::sync::mpsc;

enum Cmd {
    Data(Vec<u8>),
    Resize(u16, u16),
    Close,
}

pub struct SshShell {
    tx: mpsc::UnboundedSender<Cmd>,
    pub conn: Arc<SshConnection>,
}

pub struct ShellOptions {
    pub term: String,
    pub cols: u16,
    pub rows: u16,
    pub startup_dir: Option<String>,
    pub startup_command: Option<String>,
}

/// Quote a path for POSIX shells (single quotes, escaping embedded quotes).
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl SshShell {
    pub async fn open(conn: Arc<SshConnection>, opts: ShellOptions, sink: TermSink) -> AppResult<Arc<Self>> {
        let channel = conn.handle.channel_open_session().await?;
        channel
            .request_pty(false, &opts.term, opts.cols as u32, opts.rows as u32, 0, 0, &[])
            .await?;
        channel.request_shell(true).await?;
        let (mut read, write) = channel.split();

        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();

        // Startup directory / command are typed into the shell exactly like the user would.
        let mut startup = String::new();
        if let Some(d) = opts.startup_dir.filter(|d| !d.trim().is_empty()) {
            startup.push_str(&format!("cd {}\n", sh_quote(d.trim())));
        }
        if let Some(c) = opts.startup_command.filter(|c| !c.trim().is_empty()) {
            startup.push_str(c.trim_end());
            startup.push('\n');
        }
        if !startup.is_empty() {
            let _ = tx.send(Cmd::Data(startup.into_bytes()));
        }

        let wsink = sink.clone();
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                let r = match cmd {
                    Cmd::Data(d) => write.data_bytes(d).await,
                    Cmd::Resize(c, r) => write.window_change(c as u32, r as u32, 0, 0).await,
                    Cmd::Close => {
                        let _ = write.eof().await;
                        let _ = write.close().await;
                        break;
                    }
                };
                if r.is_err() {
                    wsink.event(TermEvent::Status { state: "lost".into(), message: Some("Write failed: connection lost".into()) });
                    break;
                }
            }
        });

        tokio::spawn(async move {
            let mut exit_code: Option<i64> = None;
            let mut clean = false;
            while let Some(msg) = read.wait().await {
                match msg {
                    ChannelMsg::Data { data } => sink.data(data.to_vec()),
                    ChannelMsg::ExtendedData { data, .. } => sink.data(data.to_vec()),
                    ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = Some(exit_status as i64);
                        clean = true;
                    }
                    ChannelMsg::ExitSignal { signal_name, .. } => {
                        clean = true;
                        sink.data(format!("\r\n[remote process terminated by signal {signal_name:?}]\r\n").into_bytes());
                    }
                    ChannelMsg::Eof => clean = true,
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            let reason = if clean { None } else { Some("Connection lost".to_string()) };
            sink.event(TermEvent::Exit { code: exit_code, reason });
        });

        Ok(Arc::new(Self { tx, conn }))
    }
}

impl TerminalBackend for SshShell {
    fn write(&self, data: &[u8]) -> AppResult<()> {
        self.tx.send(Cmd::Data(data.to_vec())).map_err(|_| AppError::Terminal("SSH channel is closed".into()))
    }
    fn resize(&self, cols: u16, rows: u16) -> AppResult<()> {
        self.tx.send(Cmd::Resize(cols, rows)).map_err(|_| AppError::Terminal("SSH channel is closed".into()))
    }
    fn close(&self) {
        let _ = self.tx.send(Cmd::Close);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quoting() {
        assert_eq!(super::sh_quote("/var/www"), "'/var/www'");
        assert_eq!(super::sh_quote("a'b"), "'a'\\''b'");
    }
}
