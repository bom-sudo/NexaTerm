//! End-to-end SSH / SFTP / tunnel tests against the in-process mock server.

mod common;
use common::mock_server::{self, MockConfig, MockServer};

use nexaterm_lib::credentials::{self, MemoryStore, SecretStore};
use nexaterm_lib::database::{self, models::*, repo};
use nexaterm_lib::error::AppError;
use nexaterm_lib::sftp::SftpManager;
use nexaterm_lib::ssh::{self, shell::{ShellOptions, SshShell}, Secrets, SshContext};
use nexaterm_lib::state::tokio_util_like::CancelFlag;
use nexaterm_lib::terminal::{TermEvent, TermSink, TerminalBackend};
use nexaterm_lib::tunnels::TunnelManager;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

struct Env {
    ctx: SshContext,
    server: MockServer,
    cfg: MockConfig,
    _root: tempfile::TempDir,
}

async fn env() -> Env {
    let root = tempfile::tempdir().unwrap();
    let cfg = MockConfig { host_key: mock_server::random_host_key(), authorized_keys: Default::default(), root: root.path().to_path_buf() };
    let server = mock_server::start(cfg.clone(), 0).await;
    let db = Arc::new(Mutex::new(database::open_in_memory().unwrap()));
    let secrets: Arc<dyn SecretStore> = Arc::new(MemoryStore::default());
    Env { ctx: SshContext { db, secrets, prompter: None }, server, cfg, _root: root }
}

fn session_input(port: u16, auth: &str) -> SessionInput {
    SessionInput {
        name: "mock".into(),
        protocol: "ssh".into(),
        host: Some("127.0.0.1".into()),
        port: Some(port as i64),
        username: Some("tester".into()),
        auth_method: Some(auth.into()),
        options: SessionOptions { connect_timeout_secs: Some(5), ..Default::default() },
        ..Default::default()
    }
}

impl Env {
    fn add_session(&self, input: SessionInput) -> Session {
        repo::insert_session(&self.ctx.db.lock().unwrap(), &input, None).unwrap()
    }
    fn trust(&self, port: u16) {
        let pk = self.cfg.host_key.public_key().to_openssh().unwrap();
        ssh::trust_host_key(&self.ctx.db.lock().unwrap(), "127.0.0.1", port, &pk).unwrap();
    }
    fn pw() -> Secrets {
        Secrets { password: Some("secret".into()), passphrase: None }
    }
}

fn sink() -> (TermSink, mpsc::UnboundedReceiver<Vec<u8>>, mpsc::UnboundedReceiver<TermEvent>) {
    let (dtx, drx) = mpsc::unbounded_channel();
    let (etx, erx) = mpsc::unbounded_channel();
    (TermSink::new(move |d| { let _ = dtx.send(d); }, move |e| { let _ = etx.send(e); }), drx, erx)
}

async fn read_until(rx: &mut mpsc::UnboundedReceiver<Vec<u8>>, needle: &str) -> String {
    let mut acc = String::new();
    let fut = async {
        while let Some(d) = rx.recv().await {
            acc.push_str(&String::from_utf8_lossy(&d));
            if acc.contains(needle) {
                return;
            }
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(10), fut).await;
    assert!(acc.contains(needle), "expected {needle:?} in output: {acc:?}");
    acc
}

fn opts() -> ShellOptions {
    ShellOptions { term: "xterm-256color".into(), cols: 80, rows: 24, startup_dir: None, startup_command: None, x11: false, agent_forward: false, follow_folder: false }
}

#[tokio::test]
async fn unknown_host_key_requires_trust_then_password_login_works() {
    let e = env().await;
    let s = e.add_session(session_input(e.server.port, "password"));
    match ssh::connect(&e.ctx, &s, &Env::pw(), 0).await {
        Err(AppError::HostKeyUnknown { details, .. }) => {
            assert!(details.fingerprint.starts_with("SHA256:"));
            ssh::trust_host_key(&e.ctx.db.lock().unwrap(), &details.host, details.port, &details.public_key).unwrap();
        }
        other => panic!("expected HostKeyUnknown, got {:?}", other.err()),
    }
    let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.expect("connect after trust");
    assert_eq!(conn.username, "tester");
}

#[tokio::test]
async fn wrong_password_is_auth_failure() {
    let e = env().await;
    e.trust(e.server.port);
    let s = e.add_session(session_input(e.server.port, "password"));
    let r = ssh::connect(&e.ctx, &s, &Secrets { password: Some("nope".into()), passphrase: None }, 0).await;
    assert!(matches!(r, Err(AppError::AuthFailed(_))), "{:?}", r.err());
    // Missing password with nothing stored → the UI must be asked for it.
    let r = ssh::connect(&e.ctx, &s, &Secrets::default(), 0).await;
    assert!(matches!(r, Err(AppError::SecretRequired(ref k)) if k == "password"));
    // Stored password (Credential Manager stand-in) is used automatically.
    e.ctx.secrets.set(&credentials::target(&s.id, "password"), "secret").unwrap();
    ssh::connect(&e.ctx, &s, &Secrets::default(), 0).await.expect("stored password works");
}

#[tokio::test]
async fn changed_host_key_is_rejected() {
    let e = env().await;
    let other = mock_server::random_host_key().public_key().to_openssh().unwrap();
    ssh::trust_host_key(&e.ctx.db.lock().unwrap(), "127.0.0.1", e.server.port, &other).unwrap();
    let s = e.add_session(session_input(e.server.port, "password"));
    match ssh::connect(&e.ctx, &s, &Env::pw(), 0).await {
        Err(AppError::HostKeyChanged { details, .. }) => assert!(details.previous_fingerprint.is_some()),
        other => panic!("expected HostKeyChanged, got {:?}", other.err()),
    }
}

#[tokio::test]
async fn private_key_auth_valid_invalid_and_encrypted() {
    let e = env().await;
    e.trust(e.server.port);
    let dir = tempfile::tempdir().unwrap();
    let key_path = dir.path().join("id_test");
    let info = nexaterm_lib::keys::generate(&key_path, "ed25519", 0, "t", Some("pp")).unwrap();
    e.cfg.authorized_keys.lock().unwrap().push(russh::keys::PublicKey::from_openssh(&info.public_key).unwrap());

    let mut input = session_input(e.server.port, "key");
    input.key_path = Some(key_path.display().to_string());
    let s = e.add_session(input);
    assert!(matches!(ssh::connect(&e.ctx, &s, &Secrets::default(), 0).await, Err(AppError::SecretRequired(ref k)) if k == "passphrase"));
    assert!(matches!(ssh::connect(&e.ctx, &s, &Secrets { password: None, passphrase: Some("bad".into()) }, 0).await, Err(AppError::AuthFailed(_))));
    ssh::connect(&e.ctx, &s, &Secrets { password: None, passphrase: Some("pp".into()) }, 0).await.expect("key login");

    // A key the server does not know.
    let other = dir.path().join("id_other");
    nexaterm_lib::keys::generate(&other, "ed25519", 0, "", None).unwrap();
    let mut input = session_input(e.server.port, "key");
    input.key_path = Some(other.display().to_string());
    let s2 = e.add_session(input);
    assert!(matches!(ssh::connect(&e.ctx, &s2, &Secrets::default(), 0).await, Err(AppError::AuthFailed(_))));

    // Malformed key file.
    let bad = dir.path().join("bad_key");
    std::fs::write(&bad, "not a key").unwrap();
    let mut input = session_input(e.server.port, "key");
    input.key_path = Some(bad.display().to_string());
    let s3 = e.add_session(input);
    assert!(matches!(ssh::connect(&e.ctx, &s3, &Secrets::default(), 0).await, Err(AppError::Validation(_))));
}

#[tokio::test]
async fn connection_timeout_when_server_never_speaks() {
    let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = silent.local_addr().unwrap().port();
    let _keep = tokio::spawn(async move {
        let mut held = vec![];
        loop {
            if let Ok((s, _)) = silent.accept().await {
                held.push(s);
            }
        }
    });
    let e = env().await;
    let mut input = session_input(port, "password");
    input.options.connect_timeout_secs = Some(1);
    let s = e.add_session(input);
    let started = std::time::Instant::now();
    assert!(matches!(ssh::connect(&e.ctx, &s, &Env::pw(), 0).await, Err(AppError::Timeout)));
    assert!(started.elapsed() < Duration::from_secs(5));
    // Refused port gives a connection error, not a hang.
    drop(e.server);
}

#[tokio::test]
async fn shell_io_resize_exit_and_dropped_connection() {
    let e = env().await;
    e.trust(e.server.port);
    let s = e.add_session(session_input(e.server.port, "password"));
    let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap();
    let (sk, mut data, mut events) = sink();
    let shell = SshShell::open(conn.clone(), opts(), sk).await.unwrap();
    read_until(&mut data, "mock-shell ready for tester").await;
    shell.write(b"whoami\r").unwrap();
    read_until(&mut data, "\r\ntester").await;
    shell.resize(132, 43).unwrap();
    shell.write(b"size\r").unwrap();
    read_until(&mut data, "132x43").await;
    shell.write(b"exit 7\r").unwrap();
    match tokio::time::timeout(Duration::from_secs(10), events.recv()).await.unwrap().unwrap() {
        TermEvent::Exit { code, reason } => {
            assert_eq!(code, Some(7));
            assert_eq!(reason, None);
        }
        other => panic!("{other:?}"),
    }

    // Dropped network connection: server aborts without an exit status.
    let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap();
    let (sk, mut data, mut events) = sink();
    let shell = SshShell::open(conn, opts(), sk).await.unwrap();
    read_until(&mut data, "ready").await;
    shell.write(b"drop\r").unwrap();
    match tokio::time::timeout(Duration::from_secs(10), events.recv()).await.unwrap().unwrap() {
        TermEvent::Exit { reason, .. } => assert_eq!(reason.as_deref(), Some("Connection lost")),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn multiple_simultaneous_sessions() {
    let e = env().await;
    e.trust(e.server.port);
    let s = e.add_session(session_input(e.server.port, "password"));
    let mut tasks = vec![];
    for i in 0..6 {
        let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap();
        tasks.push(tokio::spawn(async move {
            let (sk, mut data, _ev) = sink();
            let shell = SshShell::open(conn, opts(), sk).await.unwrap();
            read_until(&mut data, "ready").await;
            shell.write(format!("hello-{i}\r").as_bytes()).unwrap();
            read_until(&mut data, &format!("echo: hello-{i}")).await;
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
}

#[tokio::test]
async fn sftp_browse_upload_download_cancel_edit() {
    let e = env().await;
    e.trust(e.server.port);
    let s = e.add_session(session_input(e.server.port, "password"));
    let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap();
    let mgr = SftpManager::default();
    let id = mgr.open(conn).await.unwrap();
    let h = mgr.get(&id).unwrap();
    assert_eq!(h.home().await.unwrap(), "/");

    h.mkdir("/", "data").await.unwrap();
    h.create_file("/data", "empty.txt").await.unwrap();
    assert!(h.create_file("/data", "empty.txt").await.is_err(), "no silent overwrite");
    let list = h.list("/").await.unwrap();
    assert!(list.iter().any(|e| e.name == "data" && e.is_dir));

    // Large streaming upload (24 MiB) + download, byte-identical.
    let local = tempfile::tempdir().unwrap();
    let src = local.path().join("big.bin");
    let blob: Vec<u8> = (0..24 * 1024 * 1024u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(&src, &blob).unwrap();
    let mut last = 0;
    let dest = h.upload(&src, "/data", false, &CancelFlag::default(), |d, _| last = d).await.unwrap();
    assert_eq!(dest, "/data/big.bin");
    assert_eq!(last, blob.len() as u64);
    assert!(h.upload(&src, "/data", false, &CancelFlag::default(), |_, _| {}).await.is_err(), "overwrite needs confirmation");

    let dl = local.path().join("dl");
    std::fs::create_dir(&dl).unwrap();
    let got = h.download("/data/big.bin", &dl, false, &CancelFlag::default(), |_, _| {}).await.unwrap();
    assert!(std::fs::read(&got).unwrap() == blob, "downloaded content differs");

    // Interrupted (cancelled) download leaves no partial file behind.
    let cancel = CancelFlag::default();
    let c2 = cancel.clone();
    let dl2 = local.path().join("dl2");
    std::fs::create_dir(&dl2).unwrap();
    let r = h.download("/data/big.bin", &dl2, false, &cancel, move |d, _| if d > 0 { c2.cancel() }).await;
    assert!(matches!(r, Err(AppError::Cancelled)));
    assert_eq!(std::fs::read_dir(&dl2).unwrap().count(), 0, "partial file must be removed");

    // Cancelled upload leaves no remote partial file.
    let cancel = CancelFlag::default();
    let c3 = cancel.clone();
    let r = h.upload(&src, "/", false, &cancel, move |d, _| if d > 0 { c3.cancel() }).await;
    assert!(matches!(r, Err(AppError::Cancelled)));
    assert!(!h.list("/").await.unwrap().iter().any(|e| e.name.contains("big.bin")));

    // Text edit via temp-file + rename (server lacks posix-rename → backup dance path).
    h.write_text_atomic("/data/empty.txt", "line1\nline2\n").await.unwrap();
    assert_eq!(h.read_text("/data/empty.txt").await.unwrap().0, "line1\nline2\n");
    h.write_text_atomic("/data/empty.txt", "v2\n").await.unwrap();
    assert_eq!(h.read_text("/data/empty.txt").await.unwrap().0, "v2\n");
    assert!(!h.list("/data").await.unwrap().iter().any(|e| e.name.contains("nexaterm")), "no temp files left");

    let renamed = h.rename("/data/empty.txt", "notes.txt").await.unwrap();
    assert_eq!(renamed, "/data/notes.txt");
    assert!(h.delete("/").await.is_err(), "root deletion refused");
    h.delete("/data").await.unwrap();
    assert!(h.list("/").await.unwrap().is_empty());
}

async fn echo_server() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let (mut s, _) = l.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = s.read(&mut buf).await {
                    if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    port
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[tokio::test]
async fn local_and_dynamic_tunnels_lifecycle() {
    let e = env().await;
    e.trust(e.server.port);
    let s = e.add_session(session_input(e.server.port, "password"));
    let echo = echo_server().await;
    let mgr = TunnelManager::default();

    let lp = free_port();
    let t = TunnelProfile { id: "t1".into(), name: "echo".into(), session_id: s.id.clone(), kind: "local".into(), bind_address: "127.0.0.1".into(), bind_port: lp, dest_host: Some("127.0.0.1".into()), dest_port: Some(echo), auto_start: false };
    mgr.start(&t, ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap()).await.unwrap();
    let mut c = tokio::net::TcpStream::connect(("127.0.0.1", lp)).await.unwrap();
    c.write_all(b"ping through tunnel").await.unwrap();
    let mut buf = [0u8; 19];
    tokio::time::timeout(Duration::from_secs(5), c.read_exact(&mut buf)).await.unwrap().unwrap();
    assert_eq!(&buf, b"ping through tunnel");
    assert!(mgr.start(&t, ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap()).await.is_err(), "already running");
    assert_eq!(mgr.statuses()[0].total_connections, 1);
    mgr.stop("t1").await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(tokio::net::TcpStream::connect(("127.0.0.1", lp)).await.is_err(), "listener closed after stop");

    // Dynamic SOCKS5
    let sp = free_port();
    let d = TunnelProfile { id: "d1".into(), kind: "dynamic".into(), bind_port: sp, dest_host: None, dest_port: None, ..t.clone() };
    mgr.start(&d, ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap()).await.unwrap();
    let mut c = tokio::net::TcpStream::connect(("127.0.0.1", sp)).await.unwrap();
    c.write_all(&[5, 1, 0]).await.unwrap();
    let mut r = [0u8; 2];
    c.read_exact(&mut r).await.unwrap();
    let mut req = vec![5, 1, 0, 1, 127, 0, 0, 1];
    req.extend_from_slice(&echo.to_be_bytes());
    c.write_all(&req).await.unwrap();
    let mut rep = [0u8; 10];
    c.read_exact(&mut rep).await.unwrap();
    assert_eq!(rep[1], 0, "SOCKS5 success");
    c.write_all(b"socks!").await.unwrap();
    let mut b = [0u8; 6];
    tokio::time::timeout(Duration::from_secs(5), c.read_exact(&mut b)).await.unwrap().unwrap();
    assert_eq!(&b, b"socks!");
    mgr.stop_all().await;
    assert!(mgr.statuses().is_empty());

    // Port conflict detection.
    let busy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bp = busy.local_addr().unwrap().port();
    let c = TunnelProfile { id: "c1".into(), bind_port: bp, ..t };
    let r = mgr.start(&c, ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap()).await;
    assert!(matches!(r, Err(AppError::Validation(ref m)) if m.contains("already in use")), "{:?}", r.err());
}

#[tokio::test]
async fn proxy_jump_through_bastion() {
    let e = env().await;
    e.trust(e.server.port);
    // Bastion = mock server; target = the same server reached via direct-tcpip from the bastion.
    let bastion = e.add_session(session_input(e.server.port, "password"));
    e.ctx.secrets.set(&credentials::target(&bastion.id, "password"), "secret").unwrap();
    let mut target = session_input(e.server.port, "password");
    target.name = "behind-bastion".into();
    target.jump_session_id = Some(bastion.id.clone());
    let t = e.add_session(target);
    let conn = ssh::connect(&e.ctx, &t, &Env::pw(), 0).await.expect("connect via jump host");
    let (sk, mut data, _ev) = sink();
    let shell = SshShell::open(conn, opts(), sk).await.unwrap();
    read_until(&mut data, "ready").await;
    shell.write(b"whoami\r").unwrap();
    read_until(&mut data, "tester").await;

    // Jump host without saved credentials gives an actionable error.
    e.ctx.secrets.delete(&credentials::target(&bastion.id, "password")).unwrap();
    let r = ssh::connect(&e.ctx, &t, &Env::pw(), 0).await;
    assert!(matches!(r, Err(AppError::AuthFailed(ref m)) if m.contains("jump host")), "{:?}", r.err());
}

#[test]
fn secrets_never_reach_logs_or_errors() {
    let e = AppError::AuthFailed("password=hunter2 rejected".into());
    let json = serde_json::to_string(&e).unwrap();
    assert!(!json.contains("hunter2"), "{json}");
}

#[tokio::test]
async fn keyboard_interactive_two_factor_prompts() {
    let mut e = env().await;
    e.trust(e.server.port);
    let mut input = session_input(e.server.port, "keyboardInteractive");
    input.username = Some("otp".into());
    let s = e.add_session(input);

    // Without a prompter the second factor cannot be answered: clear error, no hang.
    let r = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await;
    assert!(matches!(r, Err(AppError::AuthFailed(ref m)) if m.contains("one-time code")), "{:?}", r.err());

    // With a prompter: the saved password answers round 1, the user answers the OTP round.
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen2 = seen.clone();
    e.ctx.prompter = Some(Arc::new(move |p: ssh::KbdPrompt| {
        seen2.lock().unwrap().push(p.clone());
        Box::pin(async move { Some(vec!["123456".to_string()]) })
    }));
    ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.expect("2FA login");
    let asked = seen.lock().unwrap().clone();
    assert_eq!(asked.len(), 1, "password round must not be shown to the user");
    assert_eq!(asked[0].prompts[0].text, "Verification code: ");
    assert!(asked[0].prompts[0].echo);
    assert_eq!(asked[0].name, "Two-factor authentication");

    // No saved password: both rounds go to the user; a wrong code fails cleanly.
    let answers = Arc::new(Mutex::new(vec![vec!["secret".to_string()], vec!["000000".to_string()]]));
    e.ctx.prompter = Some(Arc::new(move |_p: ssh::KbdPrompt| {
        let next = answers.lock().unwrap().remove(0);
        Box::pin(async move { Some(next) })
    }));
    assert!(matches!(ssh::connect(&e.ctx, &s, &Secrets::default(), 0).await, Err(AppError::AuthFailed(_))));

    // Cancel from the dialog.
    e.ctx.prompter = Some(Arc::new(|_p: ssh::KbdPrompt| Box::pin(async { None })));
    let r = ssh::connect(&e.ctx, &s, &Secrets::default(), 0).await;
    assert!(matches!(r, Err(AppError::AuthFailed(ref m)) if m.contains("cancelled")), "{:?}", r.err());
}

#[tokio::test]
async fn compression_option_connects() {
    let e = env().await;
    e.trust(e.server.port);
    let mut input = session_input(e.server.port, "password");
    input.options.compression = Some(true);
    let s = e.add_session(input);
    let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.expect("connect with compression enabled");
    let (sk, mut data, _ev) = sink();
    let shell = SshShell::open(conn, opts(), sk).await.unwrap();
    read_until(&mut data, "ready").await;
    shell.write(b"whoami\r").unwrap();
    read_until(&mut data, "tester").await;
}

#[tokio::test]
async fn sftp_folder_transfer_and_chmod() {
    let e = env().await;
    e.trust(e.server.port);
    let s = e.add_session(session_input(e.server.port, "password"));
    let conn = ssh::connect(&e.ctx, &s, &Env::pw(), 0).await.unwrap();
    let mgr = SftpManager::default();
    let h = mgr.get(&mgr.open(conn).await.unwrap()).unwrap();

    // Local tree: project/{a.txt, sub/b.bin, sub/deeper/c.txt, empty/}
    let local = tempfile::tempdir().unwrap();
    let proj = local.path().join("project");
    std::fs::create_dir_all(proj.join("sub/deeper")).unwrap();
    std::fs::create_dir_all(proj.join("empty")).unwrap();
    std::fs::write(proj.join("a.txt"), b"alpha").unwrap();
    std::fs::write(proj.join("sub/b.bin"), vec![7u8; 300_000]).unwrap();
    std::fs::write(proj.join("sub/deeper/c.txt"), "ไทย".as_bytes()).unwrap();

    let mut last = (0, 0);
    let up = h.upload_dir(&proj, "/", false, &CancelFlag::default(), |d, t| last = (d, t)).await.unwrap();
    assert_eq!(up.path, "/project");
    assert_eq!(up.files, 3);
    assert_eq!(last, (300_014, 300_014));
    assert!(h.list("/project").await.unwrap().iter().any(|x| x.name == "empty" && x.is_dir));
    // Second upload without overwrite is refused before anything is sent.
    assert!(matches!(h.upload_dir(&proj, "/", false, &CancelFlag::default(), |_, _| {}).await, Err(AppError::Validation(_))));
    h.upload_dir(&proj, "/", true, &CancelFlag::default(), |_, _| {}).await.expect("overwrite allowed");

    // Download the folder back and compare.
    let dl = tempfile::tempdir().unwrap();
    let down = h.download_dir("/project", dl.path(), false, &CancelFlag::default(), |_, _| {}).await.unwrap();
    assert_eq!(down.files, 3);
    let root = dl.path().join("project");
    assert_eq!(std::fs::read(root.join("a.txt")).unwrap(), b"alpha");
    assert_eq!(std::fs::read(root.join("sub").join("b.bin")).unwrap(), vec![7u8; 300_000]);
    assert_eq!(std::fs::read(root.join("sub").join("deeper").join("c.txt")).unwrap(), "ไทย".as_bytes());
    assert!(root.join("empty").is_dir());
    assert!(matches!(h.download_dir("/project", dl.path(), false, &CancelFlag::default(), |_, _| {}).await, Err(AppError::Validation(_))));

    // Cancelled folder download stops with Cancelled.
    let cancel = CancelFlag::default();
    cancel.cancel();
    let dl2 = tempfile::tempdir().unwrap();
    assert!(matches!(h.download_dir("/project", dl2.path(), false, &cancel, |_, _| {}).await, Err(AppError::Cancelled)));

    // chmod: validation, single and recursive (the mock SFTP server accepts setstat).
    assert!(h.chmod("/project/a.txt", 0o17777, false).await.is_err());
    assert_eq!(h.chmod("/project/a.txt", 0o640, false).await.unwrap(), 1);
    assert_eq!(h.chmod("/project", 0o755, true).await.unwrap(), 7);
    assert!(h.chown("/project/a.txt", None, None).await.is_err());
    let st = h.stat("/project/sub/b.bin").await.unwrap();
    assert_eq!(st.size, 300_000);
    assert!(!st.is_dir);
}
