use super::S;
use crate::credentials::{self, SecretStore};
use crate::database::models::*;
use crate::database::repo;
use crate::error::{AppError, AppResult};
use rusqlite::Connection;

/// Apply secret changes from an input: None = unchanged, Some("") = delete, Some(x) = store.
pub fn apply_secrets(conn: &Connection, store: &dyn SecretStore, id: &str, input: &SessionInput) -> AppResult<()> {
    for (kind, val) in [("password", &input.password), ("passphrase", &input.passphrase)] {
        if let Some(v) = val {
            let key = credentials::target(id, kind);
            let has = if v.is_empty() {
                store.delete(&key)?;
                false
            } else {
                store.set(&key, v)?;
                true
            };
            if kind == "password" {
                repo::set_secret_flags(conn, id, Some(has), None)?;
            } else {
                repo::set_secret_flags(conn, id, None, Some(has))?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn sessions_list(state: S) -> AppResult<Vec<Session>> {
    repo::list_sessions(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn session_create(state: S, input: SessionInput) -> AppResult<Session> {
    let db = state.db.lock().unwrap();
    let s = repo::insert_session(&db, &input, None)?;
    apply_secrets(&db, state.secrets.as_ref(), &s.id, &input)?;
    repo::get_session(&db, &s.id)
}

#[tauri::command]
pub fn session_update(state: S, id: String, input: SessionInput) -> AppResult<Session> {
    let db = state.db.lock().unwrap();
    repo::update_session(&db, &id, &input)?;
    apply_secrets(&db, state.secrets.as_ref(), &id, &input)?;
    repo::get_session(&db, &id)
}

#[tauri::command]
pub fn session_delete(state: S, id: String) -> AppResult<()> {
    let db = state.db.lock().unwrap();
    for kind in ["password", "passphrase"] {
        state.secrets.delete(&credentials::target(&id, kind))?;
    }
    repo::delete_session(&db, &id)
}

/// Duplicate copies settings only; secrets must be re-entered for the copy (never silently cloned).
#[tauri::command]
pub fn session_duplicate(state: S, id: String) -> AppResult<Session> {
    let db = state.db.lock().unwrap();
    let s = repo::get_session(&db, &id)?;
    let mut input = repo::session_to_input(&s);
    input.name = format!("{} (copy)", s.name);
    repo::insert_session(&db, &input, None)
}

#[tauri::command]
pub fn session_set_favorite(state: S, id: String, favorite: bool) -> AppResult<()> {
    repo::set_favorite(&state.db.lock().unwrap(), &id, favorite)
}

#[tauri::command]
pub fn session_move(state: S, id: String, group_id: Option<String>) -> AppResult<()> {
    repo::move_session(&state.db.lock().unwrap(), &id, group_id.as_deref())
}

#[tauri::command]
pub fn groups_list(state: S) -> AppResult<Vec<SessionGroup>> {
    repo::list_groups(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn group_create(state: S, name: String, parent_id: Option<String>) -> AppResult<SessionGroup> {
    repo::create_group(&state.db.lock().unwrap(), &name, parent_id.as_deref())
}

#[tauri::command]
pub fn group_rename(state: S, id: String, name: String) -> AppResult<()> {
    repo::rename_group(&state.db.lock().unwrap(), &id, &name)
}

#[tauri::command]
pub fn group_delete(state: S, id: String) -> AppResult<()> {
    repo::delete_group(&state.db.lock().unwrap(), &id)
}

#[tauri::command]
pub fn sessions_export(state: S, path: String) -> AppResult<usize> {
    let exp = repo::export_sessions(&state.db.lock().unwrap())?;
    std::fs::write(&path, serde_json::to_vec_pretty(&exp)?)?;
    Ok(exp.sessions.len())
}

#[tauri::command]
pub fn sessions_import(state: S, path: String) -> AppResult<usize> {
    let data = std::fs::read(&path)?;
    let exp: SessionExport = serde_json::from_slice(&data).map_err(|e| AppError::Validation(format!("Invalid export file: {e}")))?;
    repo::import_sessions(&state.db.lock().unwrap(), &exp)
}

#[tauri::command]
pub fn import_openssh_config(state: S, path: Option<String>) -> AppResult<usize> {
    let p = path.map(std::path::PathBuf::from).unwrap_or_else(|| crate::keys::default_ssh_dir().join("config"));
    let text = std::fs::read_to_string(&p).map_err(|e| AppError::Validation(format!("Cannot read {}: {e}", p.display())))?;
    let hosts = parse_openssh_config(&text);
    let db = state.db.lock().unwrap();
    let tx = db.unchecked_transaction()?;
    let group = repo::create_group(&tx, "Imported (OpenSSH)", None)?;
    let mut ids = std::collections::HashMap::new();
    let mut n = 0;
    for h in &hosts {
        let input = SessionInput {
            name: h.alias.clone(),
            group_id: Some(group.id.clone()),
            protocol: "ssh".into(),
            host: Some(h.hostname.clone().unwrap_or_else(|| h.alias.clone())),
            port: h.port.map(i64::from).or(Some(22)),
            username: h.user.clone().or_else(|| std::env::var("USERNAME").ok()),
            auth_method: Some(if h.identity_file.is_some() { "key" } else { "password" }.into()),
            key_path: h.identity_file.clone(),
            tags: vec!["openssh".into()],
            ..Default::default()
        };
        // Skip entries we cannot represent (e.g. wildcard hosts) rather than failing the import.
        if let Ok(s) = repo::insert_session(&tx, &input, None) {
            ids.insert(h.alias.clone(), s.id);
            n += 1;
        }
    }
    for h in &hosts {
        if let (Some(j), Some(id)) = (&h.proxy_jump, ids.get(&h.alias)) {
            if let Some(jid) = ids.get(j) {
                tx.execute("UPDATE sessions SET jump_session_id=?2 WHERE id=?1", rusqlite::params![id, jid])?;
            }
        }
    }
    tx.commit()?;
    Ok(n)
}

#[derive(Debug, Default, PartialEq)]
pub struct SshConfigHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
}

pub fn parse_openssh_config(text: &str) -> Vec<SshConfigHost> {
    let home = dirs::home_dir().map(|h| h.display().to_string()).unwrap_or_default();
    let mut out: Vec<SshConfigHost> = vec![];
    let mut current: Vec<usize> = vec![];
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = match line.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((k, v)) => (k.to_ascii_lowercase(), v.trim().trim_start_matches('=').trim().trim_matches('"').to_string()),
            None => continue,
        };
        match k.as_str() {
            "host" => {
                current.clear();
                for alias in v.split_whitespace() {
                    if alias.contains(['*', '?', '!']) {
                        continue;
                    }
                    out.push(SshConfigHost { alias: alias.into(), ..Default::default() });
                    current.push(out.len() - 1);
                }
            }
            "match" => current.clear(),
            _ => {
                for &i in &current {
                    let h = &mut out[i];
                    match k.as_str() {
                        "hostname" => h.hostname = Some(v.clone()),
                        "user" => h.user = Some(v.clone()),
                        "port" => h.port = v.parse().ok(),
                        "identityfile" if h.identity_file.is_none() => {
                            let p = v.replacen('~', &home, 1).replace('/', "\\");
                            h.identity_file = Some(p);
                        }
                        "proxyjump" => h.proxy_jump = v.split(',').next().map(|s| s.split('@').last().unwrap_or(s).to_string()),
                        _ => {}
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openssh_config() {
        let cfg = "# comment\nHost bastion\n  HostName 1.2.3.4\n  User ops\n  Port 2222\n\nHost web web2\n  HostName = 10.0.0.5\n  IdentityFile ~/.ssh/id_ed25519\n  ProxyJump bastion\n\nHost *\n  ServerAliveInterval 30\n";
        let h = parse_openssh_config(cfg);
        assert_eq!(h.len(), 3);
        assert_eq!(h[0].alias, "bastion");
        assert_eq!(h[0].port, Some(2222));
        assert_eq!(h[0].user.as_deref(), Some("ops"));
        assert_eq!(h[1].hostname.as_deref(), Some("10.0.0.5"));
        assert_eq!(h[2].alias, "web2");
        assert_eq!(h[2].proxy_jump.as_deref(), Some("bastion"));
        assert!(h[1].identity_file.as_deref().unwrap().ends_with(r".ssh\id_ed25519"));
    }

    #[test]
    fn secrets_go_to_store_not_db() {
        let db = crate::database::open_in_memory().unwrap();
        let store = credentials::MemoryStore::default();
        let input = SessionInput {
            name: "a".into(),
            protocol: "ssh".into(),
            host: Some("h".into()),
            port: Some(22),
            username: Some("u".into()),
            auth_method: Some("password".into()),
            password: Some("TopSecret!".into()),
            ..Default::default()
        };
        let s = repo::insert_session(&db, &input, None).unwrap();
        apply_secrets(&db, &store, &s.id, &input).unwrap();
        assert!(repo::get_session(&db, &s.id).unwrap().has_password);
        assert_eq!(store.get(&credentials::target(&s.id, "password")).unwrap().as_deref(), Some("TopSecret!"));
        // Scan every table for the secret.
        let dump: String = db
            .query_row("SELECT group_concat(options_json || coalesce(notes,'') || coalesce(key_path,'')) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert!(!dump.contains("TopSecret!"));
        let mut cleared = input.clone();
        cleared.password = Some(String::new());
        apply_secrets(&db, &store, &s.id, &cleared).unwrap();
        assert!(!repo::get_session(&db, &s.id).unwrap().has_password);
        assert_eq!(store.get(&credentials::target(&s.id, "password")).unwrap(), None);
    }
}
