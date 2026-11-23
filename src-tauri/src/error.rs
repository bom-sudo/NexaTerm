//! Application error type. Every error that crosses the IPC boundary is serialized as
//! `{ kind, message, details? }`. Messages must never contain secrets.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Credential store error: {0}")]
    Credential(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("Terminal error: {0}")]
    Terminal(String),
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Connection timed out")]
    Timeout,
    #[error("Authentication failed: {0}")]
    AuthFailed(String),
    /// The server presented a host key we have never seen. `details` carries the key.
    #[error("Host key for {host}:{port} is not known")]
    HostKeyUnknown { host: String, port: u16, details: HostKeyInfo },
    /// The server presented a key different from the one stored in known_hosts.
    #[error("WARNING: host key for {host}:{port} has CHANGED")]
    HostKeyChanged { host: String, port: u16, details: HostKeyInfo },
    /// A secret is needed to continue; the message is the secret kind ("password" | "passphrase").
    #[error("{0}")]
    SecretRequired(String),
    #[error("SFTP error: {0}")]
    Sftp(String),
    #[error("Cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostKeyInfo {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint: String,
    /// Full OpenSSH public key line of the presented key; the UI sends it back verbatim when
    /// the user chooses to trust it, so only the key that was shown can be trusted.
    pub public_key: String,
    pub previous_fingerprint: Option<String>,
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Validation(_) => "validation",
            AppError::NotFound(_) => "notFound",
            AppError::Database(_) => "database",
            AppError::Credential(_) => "credential",
            AppError::Io(_) => "io",
            AppError::Terminal(_) => "terminal",
            AppError::Connection(_) => "connection",
            AppError::Timeout => "timeout",
            AppError::AuthFailed(_) => "authFailed",
            AppError::HostKeyUnknown { .. } => "hostKeyUnknown",
            AppError::HostKeyChanged { .. } => "hostKeyChanged",
            AppError::SecretRequired(_) => "secretRequired",
            AppError::Sftp(_) => "sftp",
            AppError::Cancelled => "cancelled",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &crate::security::redact(&self.to_string()))?;
        match self {
            AppError::HostKeyUnknown { details, .. } | AppError::HostKeyChanged { details, .. } => {
                st.serialize_field("details", details)?
            }
            _ => st.serialize_field("details", &Option::<()>::None)?,
        }
        st.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => AppError::NotFound("record".into()),
            other => AppError::Database(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<russh::Error> for AppError {
    fn from(e: russh::Error) -> Self {
        AppError::Connection(e.to_string())
    }
}

impl From<russh_sftp::client::error::Error> for AppError {
    fn from(e: russh_sftp::client::error::Error) -> Self {
        AppError::Sftp(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
