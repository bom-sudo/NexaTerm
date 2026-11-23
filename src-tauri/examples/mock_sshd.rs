//! Development SSH/SFTP server for manually testing NexaTerm without a real server.
//!
//!   cargo run --example mock_sshd -- [port] [root-dir] [host-key-file]
//!
//! Login: tester / secret (password). Default port 2222, root = a temp directory.
//! With a host-key file the server keeps the same identity between runs (created if missing).
//! Not a secure server — for local testing only (binds 127.0.0.1).

#[path = "../tests/common/mock_server.rs"]
mod mock_server;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let root = args.next().map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("nexaterm-mock-sftp"));
    let key_file = args.next().map(std::path::PathBuf::from);
    std::fs::create_dir_all(&root).unwrap();
    if std::fs::read_dir(&root).map(|mut d| d.next().is_none()).unwrap_or(true) {
        std::fs::write(root.join("welcome.txt"), "Hello from the NexaTerm mock SFTP server.\n").ok();
    }
    let key = match &key_file {
        Some(p) if p.exists() => russh::keys::PrivateKey::read_openssh_file(p).expect("cannot read host key file"),
        Some(p) => {
            let k = mock_server::random_host_key();
            k.write_openssh_file(p, russh::keys::ssh_key::LineEnding::LF).expect("cannot write host key file");
            k
        }
        None => mock_server::random_host_key(),
    };
    println!("host key fingerprint: {}", key.public_key().fingerprint(russh::keys::HashAlg::Sha256));
    let srv = mock_server::start(mock_server::MockConfig { host_key: key, authorized_keys: Default::default(), root: root.clone() }, port).await;
    println!("mock sshd listening on 127.0.0.1:{} (user tester / secret), sftp root {}", srv.port, root.display());
    tokio::signal::ctrl_c().await.ok();
}
