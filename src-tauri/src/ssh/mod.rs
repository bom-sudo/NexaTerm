//! SSH transport (russh). One [`SshConnection`] can carry many channels: shells, SFTP, tunnels.

pub mod shell;

use crate::credentials::{self, SecretStore};
use crate::database::models::Session;
use crate::database::repo;
use crate::error::{AppError, AppResult, HostKeyInfo};
use russh::client::{self, AuthResult, Handle, KeyboardInteractiveAuthResponse};
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate};
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type Db = Arc<Mutex<Connection>>;

/// Remote-forward table shared with the handler: server port → (local host, local port).
pub type RemoteForwards = Arc<Mutex<HashMap<u32, (String, u16)>>>;

pub struct ClientHandler {
    db: Db,
    host: String,
    port: u16,
    verdict: Arc<Mutex<Option<AppError>>>,
    remote_forwards: RemoteForwards,
}

pub fn fingerprint(key: &PublicKey) -> String {
    key.fingerprint(HashAlg::Sha256).to_string()
}

/// Pure host-key policy, separated for unit testing.
pub fn evaluate_host_key(db: &Connection, host: &str, port: u16, key: &PublicKey) -> AppResult<()> {
    let presented = HostKeyInfo {
        host: host.to_string(),
        port,
        key_type: key.algorithm().as_str().to_string(),
        fingerprint: fingerprint(key),
        public_key: key.to_openssh().map_err(|e| AppError::Other(e.to_string()))?,
        previous_fingerprint: None,
    };
    match repo::get_known_host(db, host, port)? {
        None => Err(AppError::HostKeyUnknown { host: host.into(), port, details: presented }),
        Some(known) => {
            let known_key = PublicKey::from_openssh(&known.public_key).ok();
            let same = known_key.as_ref().map(|k| k.key_data() == key.key_data()).unwrap_or(false);
            if same {
                Ok(())
            } else {
                Err(AppError::HostKeyChanged {
                    host: host.into(),
                    port,
                    details: HostKeyInfo { previous_fingerprint: Some(known.fingerprint), ..presented },
                })
            }
        }
    }
}

/// Persist a key the user explicitly accepted. The key text comes back from the UI exactly as it
/// was presented; it is re-parsed so arbitrary strings cannot be stored.
pub fn trust_host_key(db: &Connection, host: &str, port: u16, public_key: &str) -> AppResult<String> {
    let key = PublicKey::from_openssh(public_key.trim()).map_err(|_| AppError::Validation("Invalid public key".into()))?;
    let fp = fingerprint(&key);
    let canonical = key.to_openssh().map_err(|e| AppError::Other(e.to_string()))?;
    repo::upsert_known_host(db, host, port, key.algorithm().as_str(), &canonical, &fp)?;
    Ok(fp)
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, server_public_key: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        let key = match server_public_key {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(cert) => cert.public_key().clone().into(),
        };
        let res = {
            let db = self.db.lock().unwrap();
            evaluate_host_key(&db, &self.host, self.port, &key)
        };
        match res {
            Ok(()) => Ok(true),
            Err(e) => {
                *self.verdict.lock().unwrap() = Some(e);
                Ok(false)
            }
        }
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        let target = self.remote_forwards.lock().unwrap().get(&connected_port).cloned();
        let Some((host, port)) = target else {
            // Never accept forwarded connections we did not request.
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(async move {
            if let Ok(mut sock) = tokio::net::TcpStream::connect((host.as_str(), port)).await {
                let mut stream = channel.into_stream();
                let _ = tokio::io::copy_bidirectional(&mut sock, &mut stream).await;
            } else {
                let _ = channel.close().await;
            }
        });
        Ok(())
    }
}

pub struct SshConnection {
    pub handle: Handle<ClientHandler>,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub remote_forwards: RemoteForwards,
    _jump: Option<Arc<SshConnection>>,
}

impl SshConnection {
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }
    pub async fn disconnect(&self) {
        let _ = self.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await;
    }
}

#[derive(Default, Clone)]
pub struct Secrets {
    pub password: Option<String>,
    pub passphrase: Option<String>,
}

pub struct SshContext {
    pub db: Db,
    pub secrets: Arc<dyn SecretStore>,
}

fn config(s: &Session) -> Arc<client::Config> {
    let keepalive = s.options.keepalive_secs.unwrap_or(30);
    Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: if keepalive > 0 { Some(Duration::from_secs(keepalive)) } else { None },
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    })
}

/// Open and authenticate a connection for `session`, recursing through ProxyJump hosts.
pub fn connect<'a>(
    ctx: &'a SshContext,
    session: &'a Session,
    secrets: &'a Secrets,
    depth: u8,
) -> futures::future::BoxFuture<'a, AppResult<Arc<SshConnection>>> {
    Box::pin(async move {
        if depth > 4 {
            return Err(AppError::Validation("ProxyJump chain is too deep (or circular)".into()));
        }
        if session.protocol != "ssh" {
            return Err(AppError::Validation("Session is not an SSH session".into()));
        }
        let host = crate::security::validate_host(session.host.as_deref().unwrap_or(""))?;
        let port = session.port.unwrap_or(22);
        let username = session.username.clone().unwrap_or_default();
        let timeout = Duration::from_secs(session.options.connect_timeout_secs.unwrap_or(15).clamp(1, 300));

        let verdict = Arc::new(Mutex::new(None));
        let remote_forwards: RemoteForwards = Default::default();
        let handler = ClientHandler {
            db: ctx.db.clone(),
            host: host.clone(),
            port,
            verdict: verdict.clone(),
            remote_forwards: remote_forwards.clone(),
        };

        // ProxyJump: open the jump connection first, tunnel a direct-tcpip channel through it.
        let jump = match &session.jump_session_id {
            Some(jid) => {
                let js = { repo::get_session(&ctx.db.lock().unwrap(), jid)? };
                let j = connect(ctx, &js, &Secrets::default(), depth + 1)
                    .await
                    .map_err(|e| match e {
                        AppError::SecretRequired(k) => AppError::AuthFailed(format!(
                            "jump host '{}' needs a saved {k} (edit that session and save it)",
                            js.name
                        )),
                        other => other,
                    })?;
                Some(j)
            }
            None => None,
        };

        let cfg = config(session);
        let fut = async {
            match &jump {
                Some(j) => {
                    let ch = j.handle.channel_open_direct_tcpip(host.clone(), port as u32, "127.0.0.1", 0).await?;
                    client::connect_stream(cfg, ch.into_stream(), handler).await
                }
                None => client::connect(cfg, (host.as_str(), port), handler).await,
            }
        };
        let mut handle = match tokio::time::timeout(timeout, fut).await {
            Err(_) => return Err(AppError::Timeout),
            Ok(Err(e)) => {
                if let Some(v) = verdict.lock().unwrap().take() {
                    return Err(v);
                }
                return Err(map_connect_error(e));
            }
            Ok(Ok(h)) => h,
        };

        tokio::time::timeout(timeout.max(Duration::from_secs(60)), authenticate(ctx, session, secrets, &mut handle, &username))
            .await
            .map_err(|_| AppError::Timeout)??;

        Ok(Arc::new(SshConnection {
            handle,
            label: session.name.clone(),
            host,
            port,
            username,
            remote_forwards,
            _jump: jump,
        }))
    })
}

fn map_connect_error(e: russh::Error) -> AppError {
    match e {
        russh::Error::IO(io) => match io.kind() {
            std::io::ErrorKind::ConnectionRefused => AppError::Connection("Connection refused (is SSH running on that port?)".into()),
            std::io::ErrorKind::TimedOut => AppError::Timeout,
            _ => AppError::Connection(io.to_string()),
        },
        other => AppError::Connection(other.to_string()),
    }
}

fn stored(ctx: &SshContext, session: &Session, kind: &str) -> AppResult<Option<String>> {
    ctx.secrets.get(&credentials::target(&session.id, kind))
}

async fn authenticate(ctx: &SshContext, session: &Session, secrets: &Secrets, handle: &mut Handle<ClientHandler>, user: &str) -> AppResult<()> {
    let method = session.auth_method.as_deref().unwrap_or("password");
    let result = match method {
        "password" => {
            let pw = match secrets.password.clone() {
                Some(p) => p,
                None => stored(ctx, session, "password")?.ok_or_else(|| AppError::SecretRequired("password".into()))?,
            };
            handle.authenticate_password(user, pw).await?
        }
        "keyboardInteractive" => {
            let pw = match secrets.password.clone() {
                Some(p) => p,
                None => stored(ctx, session, "password")?.ok_or_else(|| AppError::SecretRequired("password".into()))?,
            };
            keyboard_interactive(handle, user, &pw).await?
        }
        "key" => {
            let path = session.key_path.clone().unwrap_or_default();
            let pass = match secrets.passphrase.clone() {
                Some(p) => Some(p),
                None => stored(ctx, session, "passphrase")?,
            };
            let key = load_private_key(&path, pass.as_deref())?;
            let hash = if key.algorithm().is_rsa() { handle.best_supported_rsa_hash().await?.flatten() } else { None };
            handle.authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(key), hash)).await?
        }
        "agent" => agent_auth(handle, user).await?,
        other => return Err(AppError::Validation(format!("Unsupported auth method {other}"))),
    };
    match result {
        AuthResult::Success => Ok(()),
        AuthResult::Failure { remaining_methods, partial_success } => {
            let methods: Vec<String> = remaining_methods.iter().map(|m| format!("{m:?}")).collect();
            Err(AppError::AuthFailed(if partial_success {
                format!("additional authentication required ({})", methods.join(", "))
            } else {
                format!("server rejected {method} authentication for '{user}' (server allows: {})", methods.join(", "))
            }))
        }
    }
}

pub fn load_private_key(path: &str, passphrase: Option<&str>) -> AppResult<russh::keys::PrivateKey> {
    if path.trim().is_empty() {
        return Err(AppError::Validation("Private key path is empty".into()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| AppError::Validation(format!("Cannot read private key file: {e}")))?;
    let encrypted = text.contains("ENCRYPTED") || {
        russh::keys::PrivateKey::from_openssh(&text).map(|k| k.is_encrypted()).unwrap_or(false)
    };
    if encrypted && passphrase.map(str::is_empty).unwrap_or(true) {
        return Err(AppError::SecretRequired("passphrase".into()));
    }
    russh::keys::decode_secret_key(&text, passphrase).map_err(|e| {
        let msg = e.to_string();
        if encrypted {
            AppError::AuthFailed("wrong passphrase for private key".into())
        } else {
            AppError::Validation(format!("Invalid or unsupported private key: {msg}"))
        }
    })
}

async fn keyboard_interactive(handle: &mut Handle<ClientHandler>, user: &str, password: &str) -> AppResult<AuthResult> {
    let mut resp = handle.authenticate_keyboard_interactive_start(user, None).await?;
    for _ in 0..5 {
        match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(AuthResult::Success),
            KeyboardInteractiveAuthResponse::Failure { remaining_methods, partial_success } => {
                return Ok(AuthResult::Failure { remaining_methods, partial_success })
            }
            KeyboardInteractiveAuthResponse::InfoRequest { ref prompts, .. } => {
                // Answer hidden prompts (password / PIN style) with the supplied secret.
                let answers = prompts.iter().map(|p| if p.echo { String::new() } else { password.to_string() }).collect();
                resp = handle.authenticate_keyboard_interactive_respond(answers).await?;
            }
        }
    }
    Err(AppError::AuthFailed("keyboard-interactive: too many prompts".into()))
}

async fn agent_auth(handle: &mut Handle<ClientHandler>, user: &str) -> AppResult<AuthResult> {
    let mut agent = russh::keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
        .await
        .map_err(|_| AppError::AuthFailed("OpenSSH Authentication Agent is not running (start the 'ssh-agent' Windows service and add keys with ssh-add)".into()))?;
    let identities = agent.request_identities().await.map_err(|e| AppError::AuthFailed(format!("agent: {e}")))?;
    if identities.is_empty() {
        return Err(AppError::AuthFailed("SSH agent has no keys loaded (use ssh-add)".into()));
    }
    let mut last = None;
    for id in identities {
        let key = id.public_key().into_owned();
        let hash = if key.algorithm().is_rsa() { handle.best_supported_rsa_hash().await?.flatten() } else { None };
        match handle.authenticate_publickey_with(user, key, hash, &mut agent).await {
            Ok(AuthResult::Success) => return Ok(AuthResult::Success),
            Ok(f) => last = Some(f),
            Err(_) => continue,
        }
    }
    Ok(last.unwrap_or(AuthResult::Failure { remaining_methods: russh::MethodSet::empty(), partial_success: false }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::open_in_memory;

    fn key(seed: u8) -> PublicKey {
        let kp = russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[seed; 32]);
        PublicKey::from(kp.public)
    }

    #[test]
    fn unknown_then_trusted_then_changed() {
        let db = open_in_memory().unwrap();
        let k1 = key(1);
        let err = evaluate_host_key(&db, "srv", 22, &k1).unwrap_err();
        let shown = match err {
            AppError::HostKeyUnknown { details, .. } => details,
            e => panic!("expected unknown, got {e:?}"),
        };
        assert!(shown.fingerprint.starts_with("SHA256:"));
        trust_host_key(&db, "srv", 22, &shown.public_key).unwrap();
        evaluate_host_key(&db, "srv", 22, &k1).unwrap();
        // Same host, different port is a different identity.
        assert!(matches!(evaluate_host_key(&db, "srv", 2222, &k1), Err(AppError::HostKeyUnknown { .. })));
        // Changed key must be rejected and report both fingerprints.
        match evaluate_host_key(&db, "srv", 22, &key(2)) {
            Err(AppError::HostKeyChanged { details, .. }) => {
                assert_eq!(details.previous_fingerprint.as_deref(), Some(shown.fingerprint.as_str()));
                assert_ne!(details.fingerprint, shown.fingerprint);
            }
            other => panic!("expected changed, got {other:?}"),
        }
    }

    #[test]
    fn trust_rejects_garbage() {
        let db = open_in_memory().unwrap();
        assert!(trust_host_key(&db, "h", 22, "not a key").is_err());
    }

    #[test]
    fn private_key_errors() {
        assert!(matches!(load_private_key("", None), Err(AppError::Validation(_))));
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad");
        std::fs::write(&p, "-----BEGIN OPENSSH PRIVATE KEY-----\ngarbage\n-----END OPENSSH PRIVATE KEY-----\n").unwrap();
        assert!(load_private_key(p.to_str().unwrap(), None).is_err());
    }
}
