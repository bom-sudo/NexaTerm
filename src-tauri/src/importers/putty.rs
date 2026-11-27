//! PuTTY session import: the live registry (`HKCU\Software\SimonTatham\PuTTY\Sessions`) and
//! `.reg` exports of that key.

use super::{decode_text, ImportedSession, ParseResult};
use crate::database::models::SessionInput;
use std::collections::HashMap;

const SESSIONS_KEY: &str = r"Software\SimonTatham\PuTTY\Sessions";
const DEFAULT_SETTINGS: &str = "Default%20Settings";

#[derive(Debug, Clone, PartialEq)]
pub enum RegValue {
    Str(String),
    Dword(u32),
    /// Binary, multi-string, qword, ...: PuTTY does not use these for anything we import.
    Other,
}

impl RegValue {
    fn as_str(&self) -> Option<&str> {
        match self {
            RegValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Case-insensitive lookup (registry value names are case-insensitive).
fn get<'a>(values: &'a HashMap<String, RegValue>, name: &str) -> Option<&'a RegValue> {
    values.get(name).or_else(|| values.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v))
}

fn get_str<'a>(values: &'a HashMap<String, RegValue>, name: &str) -> Option<&'a str> {
    get(values, name).and_then(RegValue::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn get_num(values: &HashMap<String, RegValue>, name: &str) -> Option<i64> {
    match get(values, name)? {
        RegValue::Dword(d) => Some(*d as i64),
        RegValue::Str(s) => s.trim().parse().ok(),
        RegValue::Other => None,
    }
}

/// PuTTY stores session names percent-encoded (bytes in the system ANSI code page).
pub fn decode_session_name(raw: &str) -> String {
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    decode_text(&out)
}

/// Build a NexaTerm session from one PuTTY session's values, or return the skip reason.
pub(crate) fn session_from_values(name: &str, values: &HashMap<String, RegValue>) -> Result<SessionInput, String> {
    // PuTTY ใช้ ssh เป็นค่าเริ่มต้นเมื่อไม่ได้ระบุ Protocol
    let protocol = get_str(values, "Protocol").unwrap_or("ssh").to_ascii_lowercase();
    let default_port = match protocol.as_str() {
        "ssh" => 22,
        "telnet" => 23,
        "serial" => return Err("serial sessions not supported yet".into()),
        other => return Err(format!("{other} not supported")),
    };

    // HostName may carry the user: "user@host" (split at the last '@').
    let (host_user, host) = match get_str(values, "HostName") {
        Some(h) => match h.rsplit_once('@') {
            Some((u, h)) => (Some(u.trim().to_string()).filter(|u| !u.is_empty()), h.trim().to_string()),
            None => (None, h.to_string()),
        },
        None => (None, String::new()),
    };
    if host.is_empty() {
        return Err("no host".into());
    }
    let port = get_num(values, "PortNumber").filter(|p| (1..=65535).contains(p)).unwrap_or(default_port);
    let username = host_user.or_else(|| get_str(values, "UserName").map(String::from));

    let mut input = SessionInput {
        name: name.trim().to_string(),
        protocol: protocol.clone(),
        host: Some(host),
        port: Some(port),
        tags: vec!["putty".into()],
        ..Default::default()
    };
    if input.name.is_empty() {
        return Err("no session name".into());
    }

    let mut notes: Vec<String> = vec![];
    if let (Some(ph), Some(method)) = (get_str(values, "ProxyHost"), get_num(values, "ProxyMethod")) {
        if method != 0 {
            let pp = get_num(values, "ProxyPort").unwrap_or(0);
            notes.push(format!("PuTTY proxy {ph}:{pp} was not imported"));
        }
    }

    if protocol == "ssh" {
        input.username = Some(username.ok_or("no username")?);
        match get_str(values, "PublicKeyFile") {
            Some(k) => {
                input.auth_method = Some("key".into());
                input.key_path = Some(k.to_string());
            }
            None => input.auth_method = Some("password".into()),
        }
    } else {
        input.username = username;
    }
    if !notes.is_empty() {
        input.notes = Some(notes.join("\n"));
    }
    Ok(input)
}

fn push_session(out: &mut ParseResult, raw_name: &str, values: &HashMap<String, RegValue>) {
    if raw_name.eq_ignore_ascii_case(DEFAULT_SETTINGS) {
        return;
    }
    let name = decode_session_name(raw_name);
    match session_from_values(&name, values) {
        Ok(input) => out.sessions.push(ImportedSession { input, folder: None }),
        Err(reason) => out.skipped.push((name, reason)),
    }
}

/// Parse a `"..."` registry string starting at `s[0] == '"'`; returns (value, rest).
fn parse_quoted(s: &str) -> Option<(String, &str)> {
    let mut chars = s.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for (i, c) in chars {
        if escaped {
            out.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some((out, &s[i + 1..]));
        } else {
            out.push(c);
        }
    }
    None
}

fn parse_value(v: &str) -> RegValue {
    let v = v.trim();
    if v.starts_with('"') {
        return parse_quoted(v).map(|(s, _)| RegValue::Str(s)).unwrap_or(RegValue::Other);
    }
    if let Some(hex) = v.strip_prefix("dword:").or_else(|| v.strip_prefix("DWORD:")) {
        return u32::from_str_radix(hex.trim(), 16).map(RegValue::Dword).unwrap_or(RegValue::Other);
    }
    RegValue::Other
}

/// Join `\`-continued lines (used by `hex:` values in regedit exports).
fn logical_lines(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for raw in text.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim_end();
        if let Some(head) = line.strip_suffix('\\') {
            // A string value can legitimately end in "\\\"" but never in a bare '\'.
            if !head.trim_end().ends_with('"') {
                cur.push_str(head.trim_start());
                continue;
            }
        }
        cur.push_str(line.trim_start());
        out.push(std::mem::take(&mut cur));
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parse a regedit export (`.reg`) of the PuTTY sessions key. Decode the file with
/// [`decode_text`] first (regedit writes UTF-16LE with BOM).
pub fn parse_reg_export(text: &str) -> ParseResult {
    let mut out = ParseResult::default();
    // Preserve file order; a session key may appear more than once (values merge).
    let mut order: Vec<String> = vec![];
    let mut sessions: HashMap<String, HashMap<String, RegValue>> = HashMap::new();
    let mut current: Option<String> = None;
    let prefixes = [format!(r"HKEY_CURRENT_USER\{SESSIONS_KEY}\"), format!(r"HKCU\{SESSIONS_KEY}\")];

    for line in logical_lines(text) {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            current = None;
            let Some(path) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) else { continue };
            // "[-HKEY...]" deletes a key: nothing to import.
            for p in &prefixes {
                if path.len() > p.len() && path.is_char_boundary(p.len()) && path[..p.len()].eq_ignore_ascii_case(p) {
                    let name = &path[p.len()..];
                    // Sub-keys of a session are not part of the session itself.
                    if !name.is_empty() && !name.contains('\\') {
                        if !sessions.contains_key(name) {
                            order.push(name.to_string());
                            sessions.insert(name.to_string(), HashMap::new());
                        }
                        current = Some(name.to_string());
                    }
                    break;
                }
            }
            continue;
        }
        let Some(name) = &current else { continue };
        let Some((key, rest)) = parse_quoted(line) else { continue };
        let Some(value) = rest.trim_start().strip_prefix('=') else { continue };
        if let Some(vals) = sessions.get_mut(name) {
            vals.insert(key, parse_value(value));
        }
    }
    for name in order {
        if let Some(values) = sessions.get(&name) {
            push_session(&mut out, &name, values);
        }
    }
    out
}

/// Read PuTTY sessions from the current user's registry. A missing key is an empty result.
#[cfg(windows)]
pub fn read_registry() -> std::io::Result<ParseResult> {
    use windows::core::{HSTRING, PWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegEnumKeyExW, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_EXPAND_SZ, REG_SZ,
    };

    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by RegOpenKeyExW and is closed exactly once.
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }
    fn err(e: WIN32_ERROR) -> std::io::Error {
        std::io::Error::from_raw_os_error(e.0 as i32)
    }
    fn open(parent: HKEY, sub: &str) -> Result<Key, WIN32_ERROR> {
        let mut h = HKEY::default();
        // SAFETY: valid parent handle, NUL-terminated HSTRING, out pointer to a local.
        let r = unsafe { RegOpenKeyExW(parent, &HSTRING::from(sub), Some(0), KEY_READ, &mut h) };
        if r == ERROR_SUCCESS {
            Ok(Key(h))
        } else {
            Err(r)
        }
    }
    fn utf16_str(data: &[u8]) -> String {
        let units: Vec<u16> = data.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        String::from_utf16_lossy(&units[..end])
    }

    let mut out = ParseResult::default();
    let root = match open(HKEY_CURRENT_USER, SESSIONS_KEY) {
        Ok(k) => k,
        Err(e) if e == ERROR_FILE_NOT_FOUND => return Ok(out),
        Err(e) => return Err(err(e)),
    };

    let mut names: Vec<String> = vec![];
    let mut buf = vec![0u16; 512];
    for index in 0u32.. {
        let mut len = buf.len() as u32;
        // SAFETY: buffer and length describe the same allocation.
        let r = unsafe { RegEnumKeyExW(root.0, index, Some(PWSTR(buf.as_mut_ptr())), &mut len, None, None, None, None) };
        if r == ERROR_NO_MORE_ITEMS {
            break;
        }
        if r != ERROR_SUCCESS {
            return Err(err(r));
        }
        names.push(String::from_utf16_lossy(&buf[..len as usize]));
    }

    let mut name_buf = vec![0u16; 16384];
    let mut data = vec![0u8; 64 * 1024];
    for raw_name in names {
        let key = match open(root.0, &raw_name) {
            Ok(k) => k,
            Err(_) => {
                out.skipped.push((decode_session_name(&raw_name), "cannot open registry key".into()));
                continue;
            }
        };
        let mut values: HashMap<String, RegValue> = HashMap::new();
        for index in 0u32.. {
            let mut name_len = name_buf.len() as u32;
            let mut data_len = data.len() as u32;
            let mut kind = 0u32;
            // SAFETY: each buffer is passed with its own capacity.
            let r = unsafe {
                RegEnumValueW(
                    key.0,
                    index,
                    Some(PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    Some(&mut kind),
                    Some(data.as_mut_ptr()),
                    Some(&mut data_len),
                )
            };
            if r == ERROR_NO_MORE_ITEMS {
                break;
            }
            if r == ERROR_MORE_DATA {
                continue; // oversized value: not something we import
            }
            if r != ERROR_SUCCESS {
                break;
            }
            let vname = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let bytes = &data[..(data_len as usize).min(data.len())];
            let value = if kind == REG_SZ.0 || kind == REG_EXPAND_SZ.0 {
                RegValue::Str(utf16_str(bytes))
            } else if kind == REG_DWORD.0 && bytes.len() >= 4 {
                RegValue::Dword(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            } else {
                RegValue::Other
            };
            values.insert(vname, value);
        }
        push_session(&mut out, &raw_name, &values);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{open_in_memory, repo};
    use crate::importers::import;

    const REG: &str = "Windows Registry Editor Version 5.00\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions]\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\Default%20Settings]\r\n\
\"HostName\"=\"\"\r\n\
\"Protocol\"=\"ssh\"\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\prod%20web]\r\n\
\"Present\"=dword:00000001\r\n\
\"HostName\"=\"deploy@web.example.com\"\r\n\
\"LogFileName\"=\"putty.log\"\r\n\
\"Protocol\"=\"ssh\"\r\n\
\"PortNumber\"=dword:00000016\r\n\
\"PublicKeyFile\"=\"C:\\\\Users\\\\me\\\\keys\\\\web.ppk\"\r\n\
\"ProxyMethod\"=dword:00000000\r\n\
\"ProxyHost\"=\"proxy\"\r\n\
\"Colour0\"=\"187,187,187\"\r\n\
\"Wordness0\"=hex:00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,00,\\\r\n\
  00,00,00,00\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\db]\r\n\
\"HostName\"=\"10.1.2.3\"\r\n\
\"UserName\"=\"postgres\"\r\n\
\"PortNumber\"=dword:00000ad5\r\n\
\"Protocol\"=\"ssh\"\r\n\
\"PublicKeyFile\"=\"\"\r\n\
\"ProxyMethod\"=dword:00000003\r\n\
\"ProxyHost\"=\"bastion\"\r\n\
\"ProxyPort\"=dword:00000050\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\router]\r\n\
\"HostName\"=\"192.168.0.1\"\r\n\
\"Protocol\"=\"telnet\"\r\n\
\"PortNumber\"=dword:00000000\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\COM3%20console]\r\n\
\"HostName\"=\"\"\r\n\
\"Protocol\"=\"serial\"\r\n\
\"SerialLine\"=\"COM3\"\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\anon]\r\n\
\"HostName\"=\"anon.example.com\"\r\n\
\"Protocol\"=\"ssh\"\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\legacy]\r\n\
\"HostName\"=\"old.example.com\"\r\n\
\"Protocol\"=\"rlogin\"\r\n\
\r\n\
[HKEY_CURRENT_USER\\Software\\Other\\Thing]\r\n\
\"HostName\"=\"ignored\"\r\n";

    fn utf16le_with_bom(s: &str) -> Vec<u8> {
        let mut b = vec![0xFF, 0xFE];
        b.extend(s.encode_utf16().flat_map(|u| u.to_le_bytes()));
        b
    }

    fn find<'a>(r: &'a ParseResult, name: &str) -> &'a SessionInput {
        &r.sessions.iter().find(|s| s.input.name == name).unwrap_or_else(|| panic!("missing {name}")).input
    }

    #[test]
    fn parses_utf16_reg_export() {
        let text = decode_text(&utf16le_with_bom(REG));
        let r = parse_reg_export(&text);
        let names: Vec<_> = r.sessions.iter().map(|s| s.input.name.as_str()).collect();
        assert_eq!(names, ["prod web", "db", "router"]);

        let web = find(&r, "prod web");
        assert_eq!(web.protocol, "ssh");
        assert_eq!(web.host.as_deref(), Some("web.example.com"));
        assert_eq!(web.username.as_deref(), Some("deploy"));
        assert_eq!(web.port, Some(22));
        assert_eq!(web.auth_method.as_deref(), Some("key"));
        assert_eq!(web.key_path.as_deref(), Some(r"C:\Users\me\keys\web.ppk"));
        assert_eq!(web.notes, None, "ProxyMethod 0 means no proxy");
        assert_eq!(web.tags, ["putty"]);

        let db = find(&r, "db");
        assert_eq!(db.port, Some(2773));
        assert_eq!(db.username.as_deref(), Some("postgres"));
        assert_eq!(db.auth_method.as_deref(), Some("password"));
        assert!(db.notes.as_deref().unwrap().contains("bastion:80"));

        let router = find(&r, "router");
        assert_eq!(router.protocol, "telnet");
        assert_eq!(router.port, Some(23));

        let skipped: HashMap<_, _> = r.skipped.iter().cloned().collect();
        assert_eq!(skipped.len(), 3, "{skipped:?}");
        assert_eq!(skipped["COM3 console"], "serial sessions not supported yet");
        assert_eq!(skipped["anon"], "no username");
        assert_eq!(skipped["legacy"], "rlogin not supported");
        assert!(!skipped.contains_key("Default Settings"));
    }

    #[test]
    fn decodes_names() {
        assert_eq!(decode_session_name("a%20b%2Fc"), "a b/c");
        assert_eq!(decode_session_name("100%"), "100%");
        assert_eq!(decode_session_name("x%zz%4"), "x%zz%4");
        assert_eq!(decode_session_name("caf%E9"), "caf\u{e9}");
    }

    #[test]
    fn never_panics_on_garbage() {
        for t in ["", "[", "[]", "\"a\"=", "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\x]\n\"", "[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\x]\n\"HostName\"=dword:zz\n\\"] {
            let _ = parse_reg_export(t);
        }
    }

    #[test]
    fn imports_into_database() {
        let conn = open_in_memory().unwrap();
        let parsed = parse_reg_export(&decode_text(&utf16le_with_bom(REG)));
        let report = import(&conn, &parsed, "Imported (PuTTY)").unwrap();
        assert_eq!(report.imported, 3);
        assert_eq!(report.skipped.len(), 3);

        let groups = repo::list_groups(&conn).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Imported (PuTTY)");
        let sessions = repo::list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 3);
        for s in &sessions {
            assert_eq!(s.tags, ["putty"]);
            assert_eq!(s.group_id.as_deref(), Some(groups[0].id.as_str()));
        }
    }

    #[cfg(windows)]
    #[test]
    fn registry_read_does_not_fail() {
        // The machine may or may not have PuTTY; either way this must not error.
        let r = read_registry().unwrap();
        assert!(r.sessions.iter().all(|s| s.input.name != "Default Settings"));
    }
}
