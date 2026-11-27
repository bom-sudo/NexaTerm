//! SSH transport (russh). One [`SshConnection`] can carry many channels: shells, SFTP, tunnels.

pub mod shell;
pub mod monitor;
pub mod x11fwd;

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
    /// Local X display + cookies when X11 forwarding was requested for this session.
    x11: Option<Arc<x11fwd::X11Forward>>,
    /// Accept agent channels only when the user enabled agent forwarding.
    agent_forward: bool,
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

    async fn server_channel_open_x11(
        &mut self,
        channel: russh::Channel<client::Msg>,
        _originator_address: &str,
        _originator_port: u32,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // X11 channels are only accepted when the user enabled X11 forwarding for this session.
        let Some(fwd) = self.x11.clone() else {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        };
        reply.accept().await;
        tokio::spawn(async move {
            if let Err(e) = x11fwd::proxy(&fwd, channel.into_stream()).await {
                log::warn!("X11 forwarding to {:?}: {e}", fwd.target);
            }
        });
        Ok(())
    }

    async fn server_channel_open_agent_forward(
        &mut self,
        channel: russh::Channel<client::Msg>,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        if !self.agent_forward {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;
            match ClientOptions::new().open(r"\\.\pipe\openssh-ssh-agent") {
                Ok(mut pipe) => {
                    reply.accept().await;
                    tokio::spawn(async move {
                        let mut stream = channel.into_stream();
                        let _ = tokio::io::copy_bidirectional(&mut pipe, &mut stream).await;
                    });
                }
                Err(e) => {
                    log::warn!("agent forwarding: OpenSSH agent not available: {e}");
                    reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
                }
            }
        }
        #[cfg(unix)]
        {
            let sock = std::env::var_os("SSH_AUTH_SOCK");
            match sock {
                Some(path) => match tokio::net::UnixStream::connect(&path).await {
                    Ok(mut agent) => {
                        reply.accept().await;
                        tokio::spawn(async move {
                            let mut stream = channel.into_stream();
                            let _ = tokio::io::copy_bidirectional(&mut agent, &mut stream).await;
                        });
                    }
                    Err(e) => {
                        log::warn!("agent forwarding: cannot reach SSH_AUTH_SOCK: {e}");
                        reply.reject(russh::ChannelOpenFailure::ConnectFailed).await;
                    }
                },
                None => reply.reject(russh::ChannelOpenFailure::ConnectFailed).await,
            }
        }
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
    /// Set when X11 forwarding is enabled; shells request X11 with its fake cookie.
    pub x11: Option<Arc<x11fwd::X11Forward>>,
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
    /// Asks the user to answer keyboard-interactive prompts (2FA/OTP). None = non-interactive.
    pub prompter: Option<Prompter>,
}

/// One keyboard-interactive round as shown to the user.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbdPrompt {
    pub id: String,
    pub host: String,
    pub username: String,
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<KbdPromptItem>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbdPromptItem {
    pub text: String,
    pub echo: bool,
}

/// Returns the answers, or None when the user cancelled.
pub type Prompter = Arc<dyn Fn(KbdPrompt) -> futures::future::BoxFuture<'static, Option<Vec<String>>> + Send + Sync>;

fn config(s: &Session) -> Arc<client::Config> {
    let keepalive = s.options.keepalive_secs.unwrap_or(30);
    Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: if keepalive > 0 { Some(Duration::from_secs(keepalive)) } else { None },
        keepalive_max: 3,
        nodelay: true,
        preferred: russh::Preferred {
            compression: if s.options.compression.unwrap_or(false) {
                std::borrow::Cow::Borrowed(&[russh::compression::ZLIB_LEGACY, russh::compression::ZLIB, russh::compression::NONE])
            } else {
                std::borrow::Cow::Borrowed(&[russh::compression::NONE])
            },
            ..Default::default()
        },
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

        let x11 = if session.options.x11_forward.unwrap_or(false) {
            Some(Arc::new(x11fwd::X11Forward::new(session.options.x11_display.as_deref())?))
        } else {
            None
        };
        let verdict = Arc::new(Mutex::new(None));
        let remote_forwards: RemoteForwards = Default::default();
        let handler = ClientHandler {
            db: ctx.db.clone(),
            host: host.clone(),
            port,
            verdict: verdict.clone(),
            remote_forwards: remote_forwards.clone(),
            x11: x11.clone(),
            agent_forward: session.options.agent_forward.unwrap_or(false),
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
            x11,
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
            // A saved/typed password answers the first hidden prompt; anything else (OTP, PIN,
            // a second factor) is asked interactively through the prompter.
            let pw = match secrets.password.clone() {
                Some(p) => Some(p),
                None => stored(ctx, session, "password")?,
            };
            if pw.is_none() && ctx.prompter.is_none() {
                return Err(AppError::SecretRequired("password".into()));
            }
            keyboard_interactive(ctx, session, handle, user, pw.as_deref()).await?
        }
        "key" => {
            let mut path = session.key_path.clone().unwrap_or_default();
            // "~/.ssh/id_ed25519" style paths (common on Linux) resolve against the home directory.
            let expanded = match (path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")), dirs::home_dir()) {
                (Some(rest), Some(home)) => Some(home.join(rest).display().to_string()),
                _ => None,
            };
            if let Some(p) = expanded {
                path = p;
            }
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

/// PuTTY .ppk files declare their encryption in a header line ("Encryption: none" when plain).
pub fn ppk_encrypted(text: &str) -> bool {
    text.trim_start().starts_with("PuTTY-User-Key-File-")
        && text.lines().find_map(|l| l.strip_prefix("Encryption:")).map(|v| v.trim() != "none").unwrap_or(false)
}

pub fn load_private_key(path: &str, passphrase: Option<&str>) -> AppResult<russh::keys::PrivateKey> {
    if path.trim().is_empty() {
        return Err(AppError::Validation("Private key path is empty".into()));
    }
    let text = std::fs::read_to_string(path).map_err(|e| AppError::Validation(format!("Cannot read private key file: {e}")))?;
    let encrypted = text.contains("ENCRYPTED") || ppk_encrypted(&text) || {
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

async fn keyboard_interactive(ctx: &SshContext, session: &Session, handle: &mut Handle<ClientHandler>, user: &str, password: Option<&str>) -> AppResult<AuthResult> {
    let mut resp = handle.authenticate_keyboard_interactive_start(user, None).await?;
    let mut password_used = false;
    for _ in 0..8 {
        match resp {
            KeyboardInteractiveAuthResponse::Success => return Ok(AuthResult::Success),
            KeyboardInteractiveAuthResponse::Failure { remaining_methods, partial_success } => {
                return Ok(AuthResult::Failure { remaining_methods, partial_success })
            }
            KeyboardInteractiveAuthResponse::InfoRequest { ref name, ref instructions, ref prompts } => {
                let answers = match kbd_answers(ctx, session, user, name, instructions, prompts, password, &mut password_used).await? {
                    Some(a) => a,
                    None => return Err(AppError::AuthFailed("login cancelled".into())),
                };
                resp = handle.authenticate_keyboard_interactive_respond(answers).await?;
            }
        }
    }
    Err(AppError::AuthFailed("keyboard-interactive: too many prompts".into()))
}

/// Decide the answers for one keyboard-interactive round.
/// Empty rounds are answered empty; a single hidden prompt in the first round uses the password;
/// everything else is shown to the user. Without a prompter only the password case can succeed.
#[allow(clippy::too_many_arguments)]
async fn kbd_answers(
    ctx: &SshContext,
    session: &Session,
    user: &str,
    name: &str,
    instructions: &str,
    prompts: &[russh::client::Prompt],
    password: Option<&str>,
    password_used: &mut bool,
) -> AppResult<Option<Vec<String>>> {
    if prompts.is_empty() {
        return Ok(Some(vec![]));
    }
    if let Some(pw) = password {
        if !*password_used && prompts.len() == 1 && !prompts[0].echo {
            *password_used = true;
            return Ok(Some(vec![pw.to_string()]));
        }
    }
    let Some(prompter) = ctx.prompter.clone() else {
        return Err(AppError::AuthFailed("the server asked for additional information (for example a one-time code) and no prompt is available here; connect from a terminal tab".into()));
    };
    let request = KbdPrompt {
        id: uuid::Uuid::new_v4().to_string(),
        host: session.host.clone().unwrap_or_default(),
        username: user.to_string(),
        name: name.to_string(),
        instructions: instructions.to_string(),
        prompts: prompts.iter().map(|p| KbdPromptItem { text: p.prompt.clone(), echo: p.echo }).collect(),
    };
    let answers = prompter(request).await;
    if let Some(a) = &answers {
        if a.len() != prompts.len() {
            return Err(AppError::AuthFailed("wrong number of answers".into()));
        }
    }
    Ok(answers)
}

async fn agent_auth(handle: &mut Handle<ClientHandler>, user: &str) -> AppResult<AuthResult> {
    #[cfg(windows)]
    let agent = russh::keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
        .await
        .map_err(|_| AppError::AuthFailed("OpenSSH Authentication Agent is not running (start the 'ssh-agent' Windows service and add keys with ssh-add)".into()))?;
    #[cfg(unix)]
    let agent = russh::keys::agent::client::AgentClient::connect_env()
        .await
        .map_err(|_| AppError::AuthFailed("No SSH agent found (SSH_AUTH_SOCK is not set or not reachable); start ssh-agent and add keys with ssh-add".into()))?;
    agent_auth_with(handle, user, agent).await
}

async fn agent_auth_with<S>(handle: &mut Handle<ClientHandler>, user: &str, mut agent: russh::keys::agent::client::AgentClient<S>) -> AppResult<AuthResult>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
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
