use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionOptions {
    pub connect_timeout_secs: Option<u64>,
    pub keepalive_secs: Option<u64>,
    pub term_type: Option<String>,
    pub startup_dir: Option<String>,
    pub startup_command: Option<String>,
    pub retries: Option<u32>,
    /// Local terminal: executable path and arguments (structured, never concatenated).
    pub shell_path: Option<String>,
    pub shell_args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub encoding: Option<String>,
    /// SSH zlib compression (useful on slow links).
    pub compression: Option<bool>,
    /// Forward the local Windows OpenSSH agent to the server (ssh -A). Only for trusted servers.
    pub agent_forward: Option<bool>,
    /// Forward X11 to a local X server (VcXsrv, Xming, X410 …).
    pub x11_forward: Option<bool>,
    /// Local X display, e.g. "localhost:0" (default).
    pub x11_display: Option<String>,
    /// Let the SFTP side panel follow the shell's working directory (installs a prompt hook).
    pub follow_terminal_folder: Option<bool>,
    /// Serial port settings for protocol "serial".
    pub serial: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub name: String,
    pub group_id: Option<String>,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub auth_method: Option<String>,
    pub key_path: Option<String>,
    pub has_password: bool,
    pub has_passphrase: bool,
    pub jump_session_id: Option<String>,
    pub options: SessionOptions,
    pub favorite: bool,
    pub sort_order: i64,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_connected_at: Option<String>,
}

/// Create/update payload. Secrets travel only on this path and go straight to the OS credential
/// store; `None` = leave unchanged, `Some("")` = delete.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionInput {
    pub name: String,
    pub group_id: Option<String>,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub username: Option<String>,
    pub auth_method: Option<String>,
    pub key_path: Option<String>,
    pub jump_session_id: Option<String>,
    pub options: SessionOptions,
    pub favorite: bool,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    #[serde(skip_serializing)]
    pub password: Option<String>,
    #[serde(skip_serializing)]
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionGroup {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownHost {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub public_key: String,
    pub fingerprint: String,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: String,
    pub name: String,
    pub category: String,
    pub command: String,
    pub description: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SnippetInput {
    pub name: String,
    pub category: String,
    pub command: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelProfile {
    pub id: String,
    pub name: String,
    pub session_id: String,
    pub kind: String,
    pub bind_address: String,
    pub bind_port: u16,
    pub dest_host: Option<String>,
    pub dest_port: Option<u16>,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TunnelInput {
    pub name: String,
    pub session_id: String,
    pub kind: String,
    pub bind_address: String,
    pub bind_port: i64,
    pub dest_host: Option<String>,
    pub dest_port: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub session_id: Option<String>,
    pub label: String,
    pub protocol: String,
    pub host: Option<String>,
    pub connected_at: String,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyRecord {
    pub id: String,
    pub name: String,
    pub private_path: String,
    pub key_type: String,
    pub fingerprint: String,
    pub comment: Option<String>,
    pub encrypted: bool,
    pub created_at: String,
}

/// One recorded macro step: text sent to the terminal, or a pause.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MacroStep {
    Text { text: String },
    Wait { ms: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroProfile {
    pub id: String,
    pub name: String,
    pub steps: Vec<MacroStep>,
    pub created_at: String,
}

/// Export format: never contains secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExport {
    pub format: String,
    pub version: u32,
    pub groups: Vec<SessionGroup>,
    pub sessions: Vec<Session>,
}
