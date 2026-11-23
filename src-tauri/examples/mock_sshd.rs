//! Development SSH/SFTP server for manually testing NexaTerm without a real server.
//!
//!   cargo run --example mock_sshd -- [port] [root-dir]
//!
//! Login: tester / secret (password). Default port 2222, root = a temp directory.
//! Not a secure server — for local testing only (binds 127.0.0.1).

#[path = "../tests/common/mock_server.rs"]
mod mock_server;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(2222);
    let root = args.next().map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("nexaterm-mock-sftp"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("welcome.txt"), "Hello from the NexaTerm mock SFTP server.\n").ok();
    let key = mock_server::random_host_key();
    println!("host key fingerprint: {}", key.public_key().fingerprint(russh::keys::HashAlg::Sha256));
    let srv = mock_server::start(mock_server::MockConfig { host_key: key, authorized_keys: Default::default(), root: root.clone() }, port).await;
    println!("mock sshd listening on 127.0.0.1:{} (user tester / secret), sftp root {}", srv.port, root.display());
    tokio::signal::ctrl_c().await.ok();
}
