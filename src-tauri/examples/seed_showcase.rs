//! Seeds a throw-away NexaTerm data folder with demo content for README screenshots.
//!
//!   cargo run --example seed_showcase -- <data-dir> <mock-port> <mock-host-key-file> <sftp-root>
//!   cargo run --example seed_showcase -- --cleanup <data-dir>
//!
//! Every SSH session points at the bundled mock server (127.0.0.1). Demo passwords are stored in
//! Windows Credential Manager under the demo session ids; `--cleanup` removes them again.

use nexaterm_lib::credentials::{self, SecretStore};
use nexaterm_lib::database::{self, models::*, repo};
use std::path::{Path, PathBuf};

#[cfg(windows)]
fn store() -> credentials::WindowsCredentialStore {
    credentials::WindowsCredentialStore
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--cleanup") {
        let dir = PathBuf::from(&args[1]);
        let conn = database::open(&dir.join("nexaterm.db")).unwrap();
        for s in repo::list_sessions(&conn).unwrap() {
            for k in ["password", "passphrase"] {
                store().delete(&credentials::target(&s.id, k)).unwrap();
            }
        }
        println!("removed demo credentials for {}", dir.display());
        return;
    }
    let dir = PathBuf::from(&args[0]);
    let port: i64 = args[1].parse().unwrap();
    let key_file = PathBuf::from(&args[2]);
    let sftp_root = PathBuf::from(&args[3]);
    std::fs::create_dir_all(&dir).unwrap();
    let conn = database::open(&dir.join("nexaterm.db")).unwrap();

    // App settings: skip onboarding, dark theme.
    repo::set_setting(&conn, "onboarded", &serde_json::json!(true)).unwrap();
    repo::set_setting(&conn, "theme", &serde_json::json!("dark")).unwrap();

    let prod = repo::create_group(&conn, "Production", None).unwrap();
    let lab = repo::create_group(&conn, "Lab", None).unwrap();
    let ssh = |name: &str, group: &str, tags: &[&str], fav: bool, startup: Option<&str>| SessionInput {
        name: name.into(),
        group_id: Some(group.into()),
        protocol: "ssh".into(),
        host: Some("127.0.0.1".into()),
        port: Some(port),
        username: Some("tester".into()),
        auth_method: Some("password".into()),
        favorite: fav,
        tags: tags.iter().map(|t| t.to_string()).collect(),
        options: SessionOptions { connect_timeout_secs: Some(10), keepalive_secs: Some(30), startup_command: startup.map(String::from), ..Default::default() },
        ..Default::default()
    };
    let mut ids = vec![];
    for (name, g, tags, fav, startup) in [
        ("web-01", &prod.id, &["web", "nginx"][..], true, Some("whoami")),
        ("web-02", &prod.id, &["web"][..], false, Some("whoami")),
        ("db-01", &prod.id, &["postgres"][..], true, Some("whoami")),
        ("bastion", &prod.id, &["jump"][..], false, None),
        ("build-agent", &lab.id, &["ci"][..], false, None),
        ("staging", &lab.id, &["web"][..], false, None),
    ] {
        let s = repo::insert_session(&conn, &ssh(name, g, tags, fav, startup), None).unwrap();
        store().set(&credentials::target(&s.id, "password"), "secret").unwrap();
        repo::set_secret_flags(&conn, &s.id, Some(true), None).unwrap();
        repo::touch_connected(&conn, &s.id).unwrap();
        ids.push(s.id);
    }
    // Two-factor login on the mock server: password (saved) + one-time code asked in a dialog.
    let mut otp = ssh("otp-login", &lab.id, &["2fa"], false, None);
    otp.username = Some("otp".into());
    otp.auth_method = Some("keyboardInteractive".into());
    let otp = repo::insert_session(&conn, &otp, None).unwrap();
    store().set(&credentials::target(&otp.id, "password"), "secret").unwrap();
    repo::set_secret_flags(&conn, &otp.id, Some(true), None).unwrap();

    // A recorded macro.
    repo::save_macro(
        &conn,
        None,
        "Daily health check",
        &[
            MacroStep::Text { text: "uptime\r".into() },
            MacroStep::Wait { ms: 1200 },
            MacroStep::Text { text: "df -h /\r".into() },
        ],
    )
    .unwrap();

    // A host that is NOT trusted yet ("localhost" is a different known_hosts entry) for the host-key dialog.
    let mut fresh = ssh("new-server", &lab.id, &["new"], false, None);
    fresh.host = Some("localhost".into());
    repo::insert_session(&conn, &fresh, None).unwrap();
    repo::insert_session(&conn, &SessionInput { name: "PowerShell".into(), protocol: "local".into(), ..Default::default() }, None).unwrap();

    // Trust the mock server's key for 127.0.0.1.
    let key = russh::keys::PrivateKey::read_openssh_file(&key_file).unwrap();
    nexaterm_lib::ssh::trust_host_key(&conn, "127.0.0.1", port as u16, &key.public_key().to_openssh().unwrap()).unwrap();

    // Tunnels.
    for (name, kind, bp, dh, dp) in [
        ("Postgres via web-01", "local", 15432, Some("10.0.0.5"), Some(5432)),
        ("SOCKS proxy", "dynamic", 18080, None, None),
        ("Expose dev server", "remote", 8081, Some("127.0.0.1"), Some(3000)),
    ] {
        repo::save_tunnel(&conn, None, &TunnelInput { name: name.into(), session_id: ids[0].clone(), kind: kind.into(), bind_address: "127.0.0.1".into(), bind_port: bp, dest_host: dh.map(String::from), dest_port: dp }).unwrap();
    }

    // SSH keys (demo keys in the throw-away folder).
    let kdir = dir.join("keys");
    for (file, kind, bits, pass) in [("id_ed25519_demo", "ed25519", 0, Some("demo-pass")), ("id_rsa_demo", "rsa", 3072, None)] {
        let p = kdir.join(file);
        if !p.exists() {
            let info = nexaterm_lib::keys::generate(&p, kind, bits, "admin@nexaterm-demo", pass).unwrap();
            repo::upsert_key(&conn, file, &info.path, &info.key_type, &info.fingerprint, Some(&info.comment), info.encrypted).unwrap();
        }
    }

    seed_sftp(&sftp_root);
    println!("seeded {}", dir.display());
}

fn write(p: &Path, content: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

fn seed_sftp(root: &Path) {
    write(&root.join("etc/nginx/nginx.conf"), b"worker_processes auto;\nevents { worker_connections 1024; }\nhttp {\n    server {\n        listen 80;\n        server_name example.internal;\n        location / { proxy_pass http://127.0.0.1:3000; }\n    }\n}\n");
    write(&root.join("deploy.sh"), b"#!/bin/sh\nset -e\necho \"Deploying release $1\"\nsystemctl restart app\n");
    write(&root.join("README.md"), b"# Demo server\n\nFiles served by the NexaTerm test SFTP server.\n");
    let log: String = (0..400).map(|i| format!("2026-11-23T08:{:02}:{:02}Z INFO request completed status=200 path=/api/v1/items/{i}\n", (i / 60) % 60, i % 60)).collect();
    write(&root.join("var/log/app.log"), log.as_bytes());
    let blob: Vec<u8> = (0..3 * 1024 * 1024u32).map(|i| (i.wrapping_mul(2654435761) >> 11) as u8).collect();
    write(&root.join("backups/db-2026-11-22.sql.gz"), &blob);
    write(&root.join("backups/db-2026-11-23.sql.gz"), &blob[..2 * 1024 * 1024]);
    write(&root.join("releases/v1.0.0/app.tar.gz"), &blob[..1024 * 1024]);
}
