//! In-process mock SSH + SFTP server for repeatable tests and manual UI testing.
//!
//! * Users: password `tester` / `secret`; any public key listed in `authorized_keys`.
//! * Shell: line-oriented echo shell. Commands: `whoami`, `size` (reports PTY size),
//!   `exit [code]`, `drop` (aborts the TCP connection without exit status), anything else echoes.
//! * direct-tcpip: real TCP forwarding (used by tunnels and ProxyJump).
//! * SFTP: backed by a real directory (`root`), paths are virtual ("/" = root).
#![allow(dead_code)]

use russh::keys::{PrivateKey, PublicKey};
use russh::server::{Auth, ChannelOpenHandle, Msg, Session, VerifiedPublicKey};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, SftpHandle, SftpString, Status, StatusCode, Version};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct MockConfig {
    pub host_key: PrivateKey,
    pub authorized_keys: Arc<Mutex<Vec<PublicKey>>>,
    pub root: PathBuf,
}

pub struct MockServer {
    pub port: u16,
    pub task: tokio::task::JoinHandle<()>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub fn random_host_key() -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap()
}

pub async fn start(cfg: MockConfig, port: u16) -> MockServer {
    let listener = TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = Arc::new(russh::server::Config {
        keys: vec![cfg.host_key.clone()],
        auth_rejection_time: std::time::Duration::from_millis(50),
        auth_rejection_time_initial: Some(std::time::Duration::from_millis(0)),
        inactivity_timeout: None,
        ..Default::default()
    });
    let task = tokio::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else { break };
            let handler = SshHandler { cfg: cfg.clone(), user: String::new(), channels: Default::default(), lines: Default::default(), shells: Default::default(), pty: (0, 0) };
            let config = config.clone();
            tokio::spawn(async move {
                if let Ok(session) = russh::server::run_stream(config, sock, handler).await {
                    let _ = session.await;
                }
            });
        }
    });
    MockServer { port, task }
}

struct SshHandler {
    cfg: MockConfig,
    user: String,
    channels: HashMap<ChannelId, Channel<Msg>>,
    lines: HashMap<ChannelId, Vec<u8>>,
    shells: std::collections::HashSet<ChannelId>,
    pty: (u32, u32),
}

impl russh::server::Handler for SshHandler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        if user == "tester" && password == "secret" {
            self.user = user.into();
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn auth_publickey(&mut self, user: &str, key: VerifiedPublicKey<'_>) -> Result<Auth, Self::Error> {
        let ok = self.cfg.authorized_keys.lock().unwrap().iter().any(|k| k.key_data() == key.public_key.key_data());
        if ok {
            self.user = user.into();
            Ok(Auth::Accept)
        } else {
            Ok(Auth::reject())
        }
    }

    async fn channel_open_session(&mut self, channel: Channel<Msg>, reply: ChannelOpenHandle, _s: &mut Session) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }

    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _oa: &str,
        _op: u32,
        reply: ChannelOpenHandle,
        _s: &mut Session,
    ) -> Result<(), Self::Error> {
        match tokio::net::TcpStream::connect((host, port as u16)).await {
            Ok(mut sock) => {
                reply.accept().await;
                tokio::spawn(async move {
                    let mut stream = channel.into_stream();
                    let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
                });
            }
            Err(_) => reply.reject(russh::ChannelOpenFailure::ConnectFailed).await,
        }
        Ok(())
    }

    async fn pty_request(&mut self, ch: ChannelId, _t: &str, c: u32, r: u32, _pw: u32, _ph: u32, _m: &[(russh::Pty, u32)], s: &mut Session) -> Result<(), Self::Error> {
        self.pty = (c, r);
        s.channel_success(ch)?;
        Ok(())
    }

    async fn window_change_request(&mut self, _ch: ChannelId, c: u32, r: u32, _pw: u32, _ph: u32, _s: &mut Session) -> Result<(), Self::Error> {
        self.pty = (c, r);
        Ok(())
    }

    async fn shell_request(&mut self, ch: ChannelId, s: &mut Session) -> Result<(), Self::Error> {
        self.shells.insert(ch);
        s.channel_success(ch)?;
        s.data(ch, format!("mock-shell ready for {}\r\n$ ", self.user).into_bytes())?;
        Ok(())
    }

    async fn data(&mut self, ch: ChannelId, data: &[u8], s: &mut Session) -> Result<(), Self::Error> {
        // Only the interactive shell echoes; SFTP / forwarded channels carry their own streams.
        if !self.shells.contains(&ch) {
            return Ok(());
        }
        let mut out = Vec::new();
        let mut lines = Vec::new();
        {
            let buf = self.lines.entry(ch).or_default();
            for &b in data {
                if b == b'\r' || b == b'\n' {
                    lines.push(String::from_utf8_lossy(buf).to_string());
                    buf.clear();
                } else {
                    buf.push(b);
                    out.push(b); // local echo like a real PTY
                }
            }
        }
        if !out.is_empty() {
            s.data(ch, out)?;
        }
        for line in lines {
            let line = line.trim().to_string();
            let reply = match line.split_whitespace().next().unwrap_or("") {
                "" => String::new(),
                "whoami" => self.user.clone(),
                "size" => format!("{}x{}", self.pty.0, self.pty.1),
                "drop" => return Err(russh::Error::Disconnect),
                "exit" => {
                    let code = line.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
                    s.data(ch, b"\r\nlogout\r\n".to_vec())?;
                    s.exit_status_request(ch, code)?;
                    s.eof(ch)?;
                    s.close(ch)?;
                    return Ok(());
                }
                _ => format!("echo: {line}"),
            };
            s.data(ch, format!("\r\n{reply}{}$ ", if reply.is_empty() { "" } else { "\r\n" }).into_bytes())?;
        }
        Ok(())
    }

    async fn subsystem_request(&mut self, ch: ChannelId, name: &str, s: &mut Session) -> Result<(), Self::Error> {
        if name == "sftp" {
            if let Some(channel) = self.channels.remove(&ch) {
                s.channel_success(ch)?;
                russh_sftp::server::run(channel.into_stream(), SftpFs::new(self.cfg.root.clone())).await;
                return Ok(());
            }
        }
        s.channel_failure(ch)?;
        Ok(())
    }
}

// ------------------------------------------------------------------ SFTP over a real directory

struct SftpFs {
    root: PathBuf,
    files: HashMap<String, std::fs::File>,
    dirs: HashMap<String, Option<Vec<File>>>,
    next: u64,
}

impl SftpFs {
    fn new(root: PathBuf) -> Self {
        Self { root, files: HashMap::new(), dirs: HashMap::new(), next: 0 }
    }
    fn virt(p: &str) -> String {
        let mut parts: Vec<&str> = vec![];
        for seg in p.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                s => parts.push(s),
            }
        }
        format!("/{}", parts.join("/"))
    }
    fn real(&self, p: &str) -> PathBuf {
        let v = Self::virt(p);
        self.root.join(v.trim_start_matches('/'))
    }
    fn handle(&mut self) -> String {
        self.next += 1;
        format!("h{}", self.next)
    }
    fn ok(id: u32) -> Status {
        Status { id, status_code: StatusCode::Ok, error_message: "Ok".into(), language_tag: "en-US".into() }
    }
    fn io(e: std::io::Error) -> StatusCode {
        match e.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NoSuchFile,
            std::io::ErrorKind::PermissionDenied => StatusCode::PermissionDenied,
            _ => StatusCode::Failure,
        }
    }
    fn attrs(p: &Path) -> Result<FileAttributes, StatusCode> {
        let m = std::fs::symlink_metadata(p).map_err(Self::io)?;
        Ok(FileAttributes::from(&m))
    }
}

impl russh_sftp::server::Handler for SftpFs {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(&mut self, _v: u32, _e: HashMap<String, String>) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn open(&mut self, id: u32, filename: SftpString, pflags: OpenFlags, _a: FileAttributes) -> Result<Handle, Self::Error> {
        let p = self.real(&filename.to_string());
        let f = std::fs::OpenOptions::new()
            .read(pflags.contains(OpenFlags::READ))
            .write(pflags.contains(OpenFlags::WRITE) || pflags.contains(OpenFlags::APPEND))
            .create(pflags.contains(OpenFlags::CREATE))
            .truncate(pflags.contains(OpenFlags::TRUNCATE))
            .open(&p)
            .map_err(Self::io)?;
        let h = self.handle();
        self.files.insert(h.clone(), f);
        Ok(Handle { id, handle: h.into() })
    }

    async fn close(&mut self, id: u32, handle: SftpHandle) -> Result<Status, Self::Error> {
        let h = hkey(&handle);
        self.files.remove(&h);
        self.dirs.remove(&h);
        Ok(Self::ok(id))
    }

    async fn read(&mut self, id: u32, handle: SftpHandle, offset: u64, len: u32) -> Result<Data, Self::Error> {
        let f = self.files.get_mut(&hkey(&handle)).ok_or(StatusCode::Failure)?;
        f.seek(SeekFrom::Start(offset)).map_err(Self::io)?;
        let mut buf = vec![0u8; len.min(256 * 1024) as usize];
        let n = f.read(&mut buf).map_err(Self::io)?;
        if n == 0 {
            return Err(StatusCode::Eof);
        }
        buf.truncate(n);
        Ok(Data { id, data: buf })
    }

    async fn write(&mut self, id: u32, handle: SftpHandle, offset: u64, data: Vec<u8>) -> Result<Status, Self::Error> {
        let f = self.files.get_mut(&hkey(&handle)).ok_or(StatusCode::Failure)?;
        f.seek(SeekFrom::Start(offset)).map_err(Self::io)?;
        f.write_all(&data).map_err(Self::io)?;
        Ok(Self::ok(id))
    }

    async fn lstat(&mut self, id: u32, path: SftpString) -> Result<Attrs, Self::Error> {
        Ok(Attrs { id, attrs: Self::attrs(&self.real(&path.to_string()))? })
    }

    async fn stat(&mut self, id: u32, path: SftpString) -> Result<Attrs, Self::Error> {
        let m = std::fs::metadata(self.real(&path.to_string())).map_err(Self::io)?;
        Ok(Attrs { id, attrs: FileAttributes::from(&m) })
    }

    async fn fstat(&mut self, id: u32, handle: SftpHandle) -> Result<Attrs, Self::Error> {
        let f = self.files.get(&hkey(&handle)).ok_or(StatusCode::Failure)?;
        Ok(Attrs { id, attrs: FileAttributes::from(&f.metadata().map_err(Self::io)?) })
    }

    async fn setstat(&mut self, id: u32, _p: SftpString, _a: FileAttributes) -> Result<Status, Self::Error> {
        Ok(Self::ok(id))
    }

    async fn fsetstat(&mut self, id: u32, _h: SftpHandle, _a: FileAttributes) -> Result<Status, Self::Error> {
        Ok(Self::ok(id))
    }

    async fn opendir(&mut self, id: u32, path: SftpString) -> Result<Handle, Self::Error> {
        let dir = self.real(&path.to_string());
        let mut files = vec![];
        for e in std::fs::read_dir(&dir).map_err(Self::io)? {
            let e = e.map_err(Self::io)?;
            let attrs = FileAttributes::from(&e.metadata().map_err(Self::io)?);
            files.push(File::new(e.file_name().to_string_lossy().to_string(), attrs));
        }
        let h = self.handle();
        self.dirs.insert(h.clone(), Some(files));
        Ok(Handle { id, handle: h.into() })
    }

    async fn readdir(&mut self, id: u32, handle: SftpHandle) -> Result<Name, Self::Error> {
        match self.dirs.get_mut(&hkey(&handle)).and_then(Option::take) {
            Some(files) if !files.is_empty() => Ok(Name { id, files }),
            _ => Err(StatusCode::Eof),
        }
    }

    async fn remove(&mut self, id: u32, filename: SftpString) -> Result<Status, Self::Error> {
        std::fs::remove_file(self.real(&filename.to_string())).map_err(Self::io)?;
        Ok(Self::ok(id))
    }

    async fn mkdir(&mut self, id: u32, path: SftpString, _a: FileAttributes) -> Result<Status, Self::Error> {
        std::fs::create_dir(self.real(&path.to_string())).map_err(Self::io)?;
        Ok(Self::ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: SftpString) -> Result<Status, Self::Error> {
        std::fs::remove_dir(self.real(&path.to_string())).map_err(Self::io)?;
        Ok(Self::ok(id))
    }

    async fn realpath(&mut self, id: u32, path: SftpString) -> Result<Name, Self::Error> {
        let p = path.to_string();
        let v = if p == "." || p.is_empty() { "/".to_string() } else { Self::virt(&p) };
        Ok(Name { id, files: vec![File::dummy(v)] })
    }

    async fn rename(&mut self, id: u32, oldpath: SftpString, newpath: SftpString) -> Result<Status, Self::Error> {
        let to = self.real(&newpath.to_string());
        if to.exists() {
            return Err(StatusCode::Failure); // SFTP v3 rename does not overwrite
        }
        std::fs::rename(self.real(&oldpath.to_string()), to).map_err(Self::io)?;
        Ok(Self::ok(id))
    }
}

fn hkey(h: &SftpHandle) -> String {
    String::from_utf8_lossy(h.as_bytes()).into_owned()
}
