//! Repository functions. All take a borrowed connection so they are trivially testable.

use super::models::*;
use super::now;
use crate::error::{AppError, AppResult};
use crate::security::{validate_host, validate_port};
use rusqlite::{params, Connection, OptionalExtension, Row};

const PROTOCOLS: &[&str] = &["ssh", "local", "telnet", "rdp", "vnc", "serial"];
const AUTH: &[&str] = &["password", "key", "agent", "keyboardInteractive"];

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ---------------------------------------------------------------- sessions

const SESSION_COLS: &str = "id,name,group_id,protocol,host,port,username,auth_method,key_path,has_password,\
has_passphrase,jump_session_id,options_json,favorite,sort_order,notes,created_at,updated_at,last_connected_at";

fn row_to_session(r: &Row) -> rusqlite::Result<Session> {
    let opts: String = r.get(12)?;
    Ok(Session {
        id: r.get(0)?,
        name: r.get(1)?,
        group_id: r.get(2)?,
        protocol: r.get(3)?,
        host: r.get(4)?,
        port: r.get::<_, Option<i64>>(5)?.map(|p| p as u16),
        username: r.get(6)?,
        auth_method: r.get(7)?,
        key_path: r.get(8)?,
        has_password: r.get(9)?,
        has_passphrase: r.get(10)?,
        jump_session_id: r.get(11)?,
        options: serde_json::from_str(&opts).unwrap_or_default(),
        favorite: r.get(13)?,
        sort_order: r.get(14)?,
        notes: r.get(15)?,
        tags: vec![],
        created_at: r.get(16)?,
        updated_at: r.get(17)?,
        last_connected_at: r.get(18)?,
    })
}

fn load_tags(conn: &Connection, s: &mut Session) -> AppResult<()> {
    let mut st = conn.prepare_cached("SELECT tag FROM session_tags WHERE session_id=?1 ORDER BY tag")?;
    s.tags = st.query_map([&s.id], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(())
}

pub fn validate_session(input: &SessionInput) -> AppResult<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("Session name is required".into()));
    }
    if !PROTOCOLS.contains(&input.protocol.as_str()) {
        return Err(AppError::Validation(format!("Unsupported protocol '{}'", input.protocol)));
    }
    if let Some(a) = &input.auth_method {
        if !AUTH.contains(&a.as_str()) {
            return Err(AppError::Validation(format!("Unsupported auth method '{a}'")));
        }
    }
    if input.protocol == "ssh" {
        validate_host(input.host.as_deref().unwrap_or(""))?;
        validate_port(input.port.unwrap_or(22))?;
        if input.username.as_deref().unwrap_or("").trim().is_empty() {
            return Err(AppError::Validation("Username is required for SSH".into()));
        }
        if input.auth_method.as_deref() == Some("key") && input.key_path.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::Validation("Private key path is required for key authentication".into()));
        }
    }
    Ok(())
}

pub fn list_sessions(conn: &Connection) -> AppResult<Vec<Session>> {
    let mut st = conn.prepare(&format!("SELECT {SESSION_COLS} FROM sessions ORDER BY sort_order, name COLLATE NOCASE"))?;
    let mut out: Vec<Session> = st.query_map([], row_to_session)?.collect::<Result<_, _>>()?;
    for s in out.iter_mut() {
        load_tags(conn, s)?;
    }
    Ok(out)
}

pub fn get_session(conn: &Connection, id: &str) -> AppResult<Session> {
    let mut s = conn
        .query_row(&format!("SELECT {SESSION_COLS} FROM sessions WHERE id=?1"), [id], row_to_session)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("session {id}")))?;
    load_tags(conn, &mut s)?;
    Ok(s)
}

fn write_tags(conn: &Connection, id: &str, tags: &[String]) -> AppResult<()> {
    conn.execute("DELETE FROM session_tags WHERE session_id=?1", [id])?;
    for t in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        conn.execute("INSERT OR IGNORE INTO session_tags(session_id, tag) VALUES (?1, ?2)", params![id, t])?;
    }
    Ok(())
}

/// Insert a session. `has_password`/`has_passphrase` are set by the caller after the secret store
/// accepted the secret.
pub fn insert_session(conn: &Connection, input: &SessionInput, id: Option<String>) -> AppResult<Session> {
    validate_session(input)?;
    let id = id.unwrap_or_else(new_id);
    let ts = now();
    let host = input.host.as_deref().map(str::trim).filter(|h| !h.is_empty());
    conn.execute(
        "INSERT INTO sessions (id,name,group_id,protocol,host,port,username,auth_method,key_path,jump_session_id,\
         options_json,favorite,notes,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)",
        params![
            id,
            input.name.trim(),
            input.group_id,
            input.protocol,
            host,
            input.port,
            input.username.as_deref().map(str::trim),
            input.auth_method,
            input.key_path,
            input.jump_session_id,
            serde_json::to_string(&input.options)?,
            input.favorite,
            input.notes,
            ts
        ],
    )?;
    write_tags(conn, &id, &input.tags)?;
    get_session(conn, &id)
}

pub fn update_session(conn: &Connection, id: &str, input: &SessionInput) -> AppResult<Session> {
    validate_session(input)?;
    if input.jump_session_id.as_deref() == Some(id) {
        return Err(AppError::Validation("A session cannot use itself as jump host".into()));
    }
    let host = input.host.as_deref().map(str::trim).filter(|h| !h.is_empty());
    let n = conn.execute(
        "UPDATE sessions SET name=?2,group_id=?3,protocol=?4,host=?5,port=?6,username=?7,auth_method=?8,key_path=?9,\
         jump_session_id=?10,options_json=?11,favorite=?12,notes=?13,updated_at=?14 WHERE id=?1",
        params![
            id,
            input.name.trim(),
            input.group_id,
            input.protocol,
            host,
            input.port,
            input.username.as_deref().map(str::trim),
            input.auth_method,
            input.key_path,
            input.jump_session_id,
            serde_json::to_string(&input.options)?,
            input.favorite,
            input.notes,
            now()
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("session {id}")));
    }
    write_tags(conn, id, &input.tags)?;
    get_session(conn, id)
}

pub fn set_secret_flags(conn: &Connection, id: &str, has_password: Option<bool>, has_passphrase: Option<bool>) -> AppResult<()> {
    if let Some(v) = has_password {
        conn.execute("UPDATE sessions SET has_password=?2 WHERE id=?1", params![id, v])?;
    }
    if let Some(v) = has_passphrase {
        conn.execute("UPDATE sessions SET has_passphrase=?2 WHERE id=?1", params![id, v])?;
    }
    Ok(())
}

pub fn delete_session(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM sessions WHERE id=?1", [id])?;
    Ok(())
}

pub fn set_favorite(conn: &Connection, id: &str, fav: bool) -> AppResult<()> {
    conn.execute("UPDATE sessions SET favorite=?2 WHERE id=?1", params![id, fav])?;
    Ok(())
}

pub fn move_session(conn: &Connection, id: &str, group_id: Option<&str>) -> AppResult<()> {
    conn.execute("UPDATE sessions SET group_id=?2, updated_at=?3 WHERE id=?1", params![id, group_id, now()])?;
    Ok(())
}

pub fn touch_connected(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("UPDATE sessions SET last_connected_at=?2 WHERE id=?1", params![id, now()])?;
    Ok(())
}

pub fn session_to_input(s: &Session) -> SessionInput {
    SessionInput {
        name: s.name.clone(),
        group_id: s.group_id.clone(),
        protocol: s.protocol.clone(),
        host: s.host.clone(),
        port: s.port.map(i64::from),
        username: s.username.clone(),
        auth_method: s.auth_method.clone(),
        key_path: s.key_path.clone(),
        jump_session_id: s.jump_session_id.clone(),
        options: s.options.clone(),
        favorite: s.favorite,
        notes: s.notes.clone(),
        tags: s.tags.clone(),
        password: None,
        passphrase: None,
    }
}

// ---------------------------------------------------------------- groups

pub fn list_groups(conn: &Connection) -> AppResult<Vec<SessionGroup>> {
    let mut st = conn.prepare("SELECT id,name,parent_id,sort_order FROM session_groups ORDER BY sort_order, name COLLATE NOCASE")?;
    let v = st
        .query_map([], |r| Ok(SessionGroup { id: r.get(0)?, name: r.get(1)?, parent_id: r.get(2)?, sort_order: r.get(3)? }))?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

pub fn create_group(conn: &Connection, name: &str, parent_id: Option<&str>) -> AppResult<SessionGroup> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation("Group name is required".into()));
    }
    let id = new_id();
    conn.execute(
        "INSERT INTO session_groups(id,name,parent_id,created_at) VALUES (?1,?2,?3,?4)",
        params![id, name, parent_id, now()],
    )?;
    Ok(SessionGroup { id, name: name.into(), parent_id: parent_id.map(String::from), sort_order: 0 })
}

pub fn rename_group(conn: &Connection, id: &str, name: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err(AppError::Validation("Group name is required".into()));
    }
    conn.execute("UPDATE session_groups SET name=?2 WHERE id=?1", params![id, name.trim()])?;
    Ok(())
}

/// Deleting a group keeps its sessions (they move to the root).
pub fn delete_group(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM session_groups WHERE id=?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------- known hosts

pub fn get_known_host(conn: &Connection, host: &str, port: u16) -> AppResult<Option<KnownHost>> {
    Ok(conn
        .query_row(
            "SELECT host,port,key_type,public_key,fingerprint,added_at FROM known_hosts WHERE host=?1 AND port=?2",
            params![host.to_ascii_lowercase(), port],
            |r| {
                Ok(KnownHost {
                    host: r.get(0)?,
                    port: r.get::<_, i64>(1)? as u16,
                    key_type: r.get(2)?,
                    public_key: r.get(3)?,
                    fingerprint: r.get(4)?,
                    added_at: r.get(5)?,
                })
            },
        )
        .optional()?)
}

pub fn list_known_hosts(conn: &Connection) -> AppResult<Vec<KnownHost>> {
    let mut st = conn.prepare("SELECT host,port,key_type,public_key,fingerprint,added_at FROM known_hosts ORDER BY host")?;
    let v = st
        .query_map([], |r| {
            Ok(KnownHost {
                host: r.get(0)?,
                port: r.get::<_, i64>(1)? as u16,
                key_type: r.get(2)?,
                public_key: r.get(3)?,
                fingerprint: r.get(4)?,
                added_at: r.get(5)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

pub fn upsert_known_host(conn: &Connection, host: &str, port: u16, key_type: &str, public_key: &str, fingerprint: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO known_hosts(host,port,key_type,public_key,fingerprint,added_at) VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(host,port) DO UPDATE SET key_type=excluded.key_type, public_key=excluded.public_key,
         fingerprint=excluded.fingerprint, added_at=excluded.added_at",
        params![host.to_ascii_lowercase(), port, key_type, public_key, fingerprint, now()],
    )?;
    Ok(())
}

pub fn delete_known_host(conn: &Connection, host: &str, port: u16) -> AppResult<()> {
    conn.execute("DELETE FROM known_hosts WHERE host=?1 AND port=?2", params![host.to_ascii_lowercase(), port])?;
    Ok(())
}

// ---------------------------------------------------------------- settings

pub fn get_settings(conn: &Connection) -> AppResult<serde_json::Map<String, serde_json::Value>> {
    let mut st = conn.prepare("SELECT key,value FROM application_settings")?;
    let mut map = serde_json::Map::new();
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (k, v) = row?;
        map.insert(k, serde_json::from_str(&v).unwrap_or(serde_json::Value::String(v)));
    }
    Ok(map)
}

pub fn set_setting(conn: &Connection, key: &str, value: &serde_json::Value) -> AppResult<()> {
    if key.is_empty() || key.len() > 100 {
        return Err(AppError::Validation("Invalid setting key".into()));
    }
    conn.execute(
        "INSERT INTO application_settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value.to_string()],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- snippets

pub fn list_snippets(conn: &Connection) -> AppResult<Vec<Snippet>> {
    let mut st = conn.prepare("SELECT id,name,category,command,description,updated_at FROM command_snippets ORDER BY category, name COLLATE NOCASE")?;
    let v = st
        .query_map([], |r| {
            Ok(Snippet { id: r.get(0)?, name: r.get(1)?, category: r.get(2)?, command: r.get(3)?, description: r.get(4)?, updated_at: r.get(5)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

fn check_snippet(i: &SnippetInput) -> AppResult<()> {
    if i.name.trim().is_empty() || i.command.is_empty() {
        return Err(AppError::Validation("Snippet name and command are required".into()));
    }
    Ok(())
}

pub fn save_snippet(conn: &Connection, id: Option<&str>, i: &SnippetInput) -> AppResult<String> {
    check_snippet(i)?;
    let cat = if i.category.trim().is_empty() { "General" } else { i.category.trim() };
    let ts = now();
    match id {
        Some(id) => {
            conn.execute(
                "UPDATE command_snippets SET name=?2,category=?3,command=?4,description=?5,updated_at=?6 WHERE id=?1",
                params![id, i.name.trim(), cat, i.command, i.description, ts],
            )?;
            Ok(id.to_string())
        }
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO command_snippets(id,name,category,command,description,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?6)",
                params![id, i.name.trim(), cat, i.command, i.description, ts],
            )?;
            Ok(id)
        }
    }
}

pub fn delete_snippet(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM command_snippets WHERE id=?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------- tunnels

fn row_to_tunnel(r: &Row) -> rusqlite::Result<TunnelProfile> {
    Ok(TunnelProfile {
        id: r.get(0)?,
        name: r.get(1)?,
        session_id: r.get(2)?,
        kind: r.get(3)?,
        bind_address: r.get(4)?,
        bind_port: r.get::<_, i64>(5)? as u16,
        dest_host: r.get(6)?,
        dest_port: r.get::<_, Option<i64>>(7)?.map(|p| p as u16),
        auto_start: r.get(8)?,
    })
}

const TUNNEL_COLS: &str = "id,name,session_id,kind,bind_address,bind_port,dest_host,dest_port,auto_start";

pub fn list_tunnels(conn: &Connection) -> AppResult<Vec<TunnelProfile>> {
    let mut st = conn.prepare(&format!("SELECT {TUNNEL_COLS} FROM tunnel_profiles ORDER BY name COLLATE NOCASE"))?;
    let v = st.query_map([], row_to_tunnel)?.collect::<Result<_, _>>()?;
    Ok(v)
}

pub fn get_tunnel(conn: &Connection, id: &str) -> AppResult<TunnelProfile> {
    conn.query_row(&format!("SELECT {TUNNEL_COLS} FROM tunnel_profiles WHERE id=?1"), [id], row_to_tunnel)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("tunnel {id}")))
}

pub fn validate_tunnel(i: &TunnelInput) -> AppResult<()> {
    if i.name.trim().is_empty() {
        return Err(AppError::Validation("Tunnel name is required".into()));
    }
    if !["local", "remote", "dynamic"].contains(&i.kind.as_str()) {
        return Err(AppError::Validation("Tunnel type must be local, remote or dynamic".into()));
    }
    validate_port(i.bind_port)?;
    validate_host(if i.bind_address.is_empty() { "127.0.0.1" } else { &i.bind_address })?;
    if i.kind != "dynamic" {
        validate_host(i.dest_host.as_deref().unwrap_or(""))?;
        validate_port(i.dest_port.unwrap_or(0))?;
    }
    Ok(())
}

pub fn save_tunnel(conn: &Connection, id: Option<&str>, i: &TunnelInput) -> AppResult<TunnelProfile> {
    validate_tunnel(i)?;
    let bind = if i.bind_address.trim().is_empty() { "127.0.0.1" } else { i.bind_address.trim() };
    let (dh, dp) = if i.kind == "dynamic" { (None, None) } else { (i.dest_host.clone(), i.dest_port) };
    let id = match id {
        Some(id) => {
            conn.execute(
                "UPDATE tunnel_profiles SET name=?2,session_id=?3,kind=?4,bind_address=?5,bind_port=?6,dest_host=?7,dest_port=?8 WHERE id=?1",
                params![id, i.name.trim(), i.session_id, i.kind, bind, i.bind_port, dh, dp],
            )?;
            id.to_string()
        }
        None => {
            let id = new_id();
            conn.execute(
                "INSERT INTO tunnel_profiles(id,name,session_id,kind,bind_address,bind_port,dest_host,dest_port,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![id, i.name.trim(), i.session_id, i.kind, bind, i.bind_port, dh, dp, now()],
            )?;
            id
        }
    };
    get_tunnel(conn, &id)
}

pub fn delete_tunnel(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM tunnel_profiles WHERE id=?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------- history

pub fn add_history(conn: &Connection, session_id: Option<&str>, label: &str, protocol: &str, host: Option<&str>, outcome: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO connection_history(session_id,label,protocol,host,connected_at,outcome) VALUES (?1,?2,?3,?4,?5,?6)",
        params![session_id, label, protocol, host, now(), outcome],
    )?;
    // Bound the table size (not a usage limit — just housekeeping).
    conn.execute("DELETE FROM connection_history WHERE id NOT IN (SELECT id FROM connection_history ORDER BY id DESC LIMIT 500)", [])?;
    Ok(())
}

pub fn list_history(conn: &Connection, limit: i64) -> AppResult<Vec<HistoryEntry>> {
    let mut st = conn.prepare("SELECT id,session_id,label,protocol,host,connected_at,outcome FROM connection_history ORDER BY id DESC LIMIT ?1")?;
    let v = st
        .query_map([limit], |r| {
            Ok(HistoryEntry { id: r.get(0)?, session_id: r.get(1)?, label: r.get(2)?, protocol: r.get(3)?, host: r.get(4)?, connected_at: r.get(5)?, outcome: r.get(6)? })
        })?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

pub fn clear_history(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM connection_history", [])?;
    Ok(())
}

// ---------------------------------------------------------------- ssh keys

pub fn list_keys(conn: &Connection) -> AppResult<Vec<SshKeyRecord>> {
    let mut st = conn.prepare("SELECT id,name,private_path,key_type,fingerprint,comment,encrypted,created_at FROM ssh_keys ORDER BY name COLLATE NOCASE")?;
    let v = st
        .query_map([], |r| {
            Ok(SshKeyRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                private_path: r.get(2)?,
                key_type: r.get(3)?,
                fingerprint: r.get(4)?,
                comment: r.get(5)?,
                encrypted: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

pub fn upsert_key(conn: &Connection, name: &str, path: &str, key_type: &str, fp: &str, comment: Option<&str>, encrypted: bool) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ssh_keys(id,name,private_path,key_type,fingerprint,comment,encrypted,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(private_path) DO UPDATE SET name=excluded.name,key_type=excluded.key_type,fingerprint=excluded.fingerprint,
         comment=excluded.comment,encrypted=excluded.encrypted",
        params![new_id(), name, path, key_type, fp, comment, encrypted, now()],
    )?;
    Ok(())
}

pub fn delete_key_record(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM ssh_keys WHERE id=?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------- import / export

pub fn export_sessions(conn: &Connection) -> AppResult<SessionExport> {
    let mut sessions = list_sessions(conn)?;
    for s in sessions.iter_mut() {
        // Never export secret flags as true: the importing machine has no secrets for them.
        s.has_password = false;
        s.has_passphrase = false;
    }
    Ok(SessionExport { format: "nexaterm-sessions".into(), version: 1, groups: list_groups(conn)?, sessions })
}

/// Import with fresh ids; returns number of sessions imported.
pub fn import_sessions(conn: &Connection, data: &SessionExport) -> AppResult<usize> {
    if data.format != "nexaterm-sessions" {
        return Err(AppError::Validation("Not a NexaTerm session export file".into()));
    }
    let tx = conn.unchecked_transaction()?;
    let mut group_map = std::collections::HashMap::new();
    for g in &data.groups {
        let ng = create_group(&tx, &g.name, None)?;
        group_map.insert(g.id.clone(), ng.id);
    }
    for g in &data.groups {
        if let (Some(p), Some(new_id)) = (&g.parent_id, group_map.get(&g.id)) {
            tx.execute("UPDATE session_groups SET parent_id=?2 WHERE id=?1", params![new_id, group_map.get(p)])?;
        }
    }
    let mut id_map = std::collections::HashMap::new();
    let mut count = 0;
    for s in &data.sessions {
        let mut input = session_to_input(s);
        input.group_id = s.group_id.as_ref().and_then(|g| group_map.get(g).cloned());
        input.jump_session_id = None;
        let ns = insert_session(&tx, &input, None)?;
        id_map.insert(s.id.clone(), ns.id);
        count += 1;
    }
    for s in &data.sessions {
        if let (Some(j), Some(nid)) = (&s.jump_session_id, id_map.get(&s.id)) {
            if let Some(nj) = id_map.get(j) {
                tx.execute("UPDATE sessions SET jump_session_id=?2 WHERE id=?1", params![nid, nj])?;
            }
        }
    }
    tx.commit()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::open_in_memory;

    fn ssh_input(name: &str) -> SessionInput {
        SessionInput {
            name: name.into(),
            protocol: "ssh".into(),
            host: Some("example.com".into()),
            port: Some(22),
            username: Some("root".into()),
            auth_method: Some("password".into()),
            tags: vec!["prod".into(), "web".into()],
            ..Default::default()
        }
    }

    #[test]
    fn session_crud() {
        let c = open_in_memory().unwrap();
        let s = insert_session(&c, &ssh_input("web1"), None).unwrap();
        assert_eq!(s.tags, vec!["prod", "web"]);
        assert!(!s.has_password);
        let mut upd = ssh_input("web1-renamed");
        upd.tags = vec!["staging".into()];
        upd.options.keepalive_secs = Some(30);
        let s2 = update_session(&c, &s.id, &upd).unwrap();
        assert_eq!(s2.name, "web1-renamed");
        assert_eq!(s2.tags, vec!["staging"]);
        assert_eq!(s2.options.keepalive_secs, Some(30));
        set_secret_flags(&c, &s.id, Some(true), None).unwrap();
        assert!(get_session(&c, &s.id).unwrap().has_password);
        assert_eq!(list_sessions(&c).unwrap().len(), 1);
        delete_session(&c, &s.id).unwrap();
        assert!(matches!(get_session(&c, &s.id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn session_validation() {
        let c = open_in_memory().unwrap();
        let mut bad = ssh_input("x");
        bad.host = Some("bad host;rm".into());
        assert!(insert_session(&c, &bad, None).is_err());
        let mut bad = ssh_input("x");
        bad.port = Some(70000);
        assert!(insert_session(&c, &bad, None).is_err());
        let mut bad = ssh_input("x");
        bad.protocol = "ftp".into();
        assert!(insert_session(&c, &bad, None).is_err());
        let local = SessionInput { name: "PS".into(), protocol: "local".into(), ..Default::default() };
        assert!(insert_session(&c, &local, None).is_ok());
    }

    #[test]
    fn groups_and_move() {
        let c = open_in_memory().unwrap();
        let g = create_group(&c, "Prod", None).unwrap();
        let mut i = ssh_input("a");
        i.group_id = Some(g.id.clone());
        let s = insert_session(&c, &i, None).unwrap();
        rename_group(&c, &g.id, "Production").unwrap();
        assert_eq!(list_groups(&c).unwrap()[0].name, "Production");
        delete_group(&c, &g.id).unwrap();
        assert_eq!(get_session(&c, &s.id).unwrap().group_id, None);
    }

    #[test]
    fn known_hosts_roundtrip() {
        let c = open_in_memory().unwrap();
        assert!(get_known_host(&c, "Host", 22).unwrap().is_none());
        upsert_known_host(&c, "Host", 22, "ssh-ed25519", "ssh-ed25519 AAAA", "SHA256:x").unwrap();
        let k = get_known_host(&c, "host", 22).unwrap().unwrap();
        assert_eq!(k.fingerprint, "SHA256:x");
        upsert_known_host(&c, "host", 22, "ssh-ed25519", "ssh-ed25519 BBBB", "SHA256:y").unwrap();
        assert_eq!(list_known_hosts(&c).unwrap().len(), 1);
        delete_known_host(&c, "host", 22).unwrap();
        assert!(list_known_hosts(&c).unwrap().is_empty());
    }

    #[test]
    fn export_import_has_no_secrets() {
        let c = open_in_memory().unwrap();
        let g = create_group(&c, "G", None).unwrap();
        let mut i = ssh_input("a");
        i.group_id = Some(g.id);
        let s = insert_session(&c, &i, None).unwrap();
        let mut j = ssh_input("b");
        j.jump_session_id = Some(s.id.clone());
        insert_session(&c, &j, None).unwrap();
        set_secret_flags(&c, &s.id, Some(true), Some(true)).unwrap();
        let exp = export_sessions(&c).unwrap();
        let json = serde_json::to_string(&exp).unwrap();
        assert!(!json.contains("\"hasPassword\":true"));
        // No secret-bearing fields at all (authMethod:"password" is fine, a "password": key is not).
        assert!(!json.contains("\"password\":") && !json.contains("\"passphrase\":"));
        let c2 = open_in_memory().unwrap();
        assert_eq!(import_sessions(&c2, &exp).unwrap(), 2);
        let all = list_sessions(&c2).unwrap();
        let b = all.iter().find(|s| s.name == "b").unwrap();
        let a = all.iter().find(|s| s.name == "a").unwrap();
        assert_eq!(b.jump_session_id.as_deref(), Some(a.id.as_str()));
        assert!(a.group_id.is_some());
    }

    #[test]
    fn snippets_tunnels_settings_history() {
        let c = open_in_memory().unwrap();
        assert!(list_snippets(&c).unwrap().len() >= 6);
        let id = save_snippet(&c, None, &SnippetInput { name: "x".into(), category: "".into(), command: "uptime".into(), description: None }).unwrap();
        assert!(list_snippets(&c).unwrap().iter().any(|s| s.id == id && s.category == "General"));
        delete_snippet(&c, &id).unwrap();

        let s = insert_session(&c, &ssh_input("gw"), None).unwrap();
        let t = save_tunnel(
            &c,
            None,
            &TunnelInput { name: "db".into(), session_id: s.id.clone(), kind: "local".into(), bind_address: "".into(), bind_port: 15432, dest_host: Some("10.0.0.5".into()), dest_port: Some(5432) },
        )
        .unwrap();
        assert_eq!(t.bind_address, "127.0.0.1");
        let bad = TunnelInput { name: "x".into(), session_id: s.id.clone(), kind: "local".into(), bind_address: "".into(), bind_port: 1, dest_host: None, dest_port: None };
        assert!(save_tunnel(&c, None, &bad).is_err());
        let dyn_t = TunnelInput { name: "socks".into(), session_id: s.id.clone(), kind: "dynamic".into(), bind_address: "".into(), bind_port: 1080, dest_host: Some("ignored".into()), dest_port: Some(1) };
        assert_eq!(save_tunnel(&c, None, &dyn_t).unwrap().dest_host, None);

        set_setting(&c, "theme", &serde_json::json!("light")).unwrap();
        assert_eq!(get_settings(&c).unwrap()["theme"], "light");

        add_history(&c, Some(&s.id), "gw", "ssh", Some("example.com"), "connected").unwrap();
        assert_eq!(list_history(&c, 10).unwrap().len(), 1);
        delete_session(&c, &s.id).unwrap();
        assert!(list_tunnels(&c).unwrap().is_empty(), "tunnels cascade with their session");
        assert_eq!(list_history(&c, 10).unwrap()[0].session_id, None);
    }
}
