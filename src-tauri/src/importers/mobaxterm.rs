//! MobaXterm session import: `.mxtsessions` exports and the `[Bookmarks*]` sections of
//! `MobaXterm.ini`.
//!
//! Format (reverse engineered, best effort):
//! ```text
//! [Bookmarks_1]
//! SubRep=Production\Web
//! ImgNum=41
//! web01= #109#0%10.0.0.5%22%root%%-1%-1%%%%%0%0%0%_ProfileDir_\.ssh\id_rsa%%-1%...#MobaFont%10%...#0# #-1
//! ```
//! The value is `#`-separated: block[1] = icon number, block[2] = `<type>%field%field...`.

use super::{ImportedSession, ParseResult};
use crate::database::models::SessionInput;

/// MobaXterm session type numbers. Best effort: only 0, 1 and 7 are imported; the labels of the
/// others are used in skip reasons only.
const SESSION_TYPES: &[(u32, &str)] = &[
    (0, "SSH"),
    (1, "Telnet"),
    (2, "Rsh"),
    (3, "Xdmcp"),
    (4, "RDP"),
    (5, "VNC"),
    (6, "FTP"),
    (7, "SFTP"),
    (8, "Serial"),
    (9, "File"),
    (10, "Local shell"),
    (11, "Browser"),
    (12, "Mosh"),
    (13, "AWS S3"),
    (14, "WSL"),
];

fn type_label(t: u32) -> Option<&'static str> {
    SESSION_TYPES.iter().find(|(n, _)| *n == t).map(|(_, l)| *l)
}

/// MobaXterm escapes characters that would break its own syntax.
fn unescape(s: &str) -> String {
    s.replace("__DIEZE__", "#").replace("__PTVIRG__", ";").replace("__DBLQUO__", "\"").replace("__PIPE__", "|")
}

fn is_bookmarks_section(name: &str) -> bool {
    let n = name.trim().to_ascii_lowercase();
    n == "bookmarks" || n.strip_prefix("bookmarks_").is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

fn normalize_folder(subrep: &str) -> Option<String> {
    let parts: Vec<&str> = subrep.split(['\\', '/']).map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\\"))
    }
}

pub fn parse_mxtsessions(text: &str) -> ParseResult {
    let mut out = ParseResult::default();
    let mut in_bookmarks = false;
    // (session name, raw value) collected per section; SubRep may appear after entries in
    // hand-edited files, so resolve the folder when the section ends.
    let mut folder: Option<String> = None;
    let mut pending: Vec<(String, String)> = vec![];

    fn flush(out: &mut ParseResult, pending: &mut Vec<(String, String)>, folder: &Option<String>) {
        for (name, value) in pending.drain(..) {
            match parse_entry(&name, &value) {
                Ok(input) => out.sessions.push(ImportedSession { input, folder: folder.clone() }),
                Err(reason) => out.skipped.push((name, reason)),
            }
        }
    }

    for raw in text.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            flush(&mut out, &mut pending, &folder);
            in_bookmarks = is_bookmarks_section(&line[1..line.len() - 1]);
            folder = None;
            continue;
        }
        if !in_bookmarks {
            continue;
        }
        // Lines without '=' cannot be named, so they are ignored rather than reported.
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if key.eq_ignore_ascii_case("SubRep") {
            folder = normalize_folder(value);
        } else if key.eq_ignore_ascii_case("ImgNum") || key.is_empty() {
            // Folder icon / nameless entry: nothing to import.
        } else {
            pending.push((unescape(key), value.to_string()));
        }
    }
    flush(&mut out, &mut pending, &folder);
    out
}

fn parse_port(s: Option<&&str>, default: i64) -> i64 {
    s.and_then(|p| p.trim().parse::<i64>().ok()).filter(|p| (1..=65535).contains(p)).unwrap_or(default)
}

fn non_empty(s: Option<&&str>) -> Option<String> {
    s.map(|v| unescape(v.trim())).filter(|v| !v.is_empty())
}

/// Turn one `name=value` bookmark into a session, or return the skip reason.
fn parse_entry(name: &str, value: &str) -> Result<SessionInput, String> {
    let blocks: Vec<&str> = value.split('#').collect();
    let Some(conn_block) = blocks.get(2) else {
        return Err("unrecognized MobaXterm session format".into());
    };
    let fields: Vec<&str> = conn_block.split('%').collect();
    let Ok(kind) = fields[0].trim().parse::<u32>() else {
        return Err("unrecognized MobaXterm session format".into());
    };
    let host = non_empty(fields.get(1));

    let mut input = SessionInput { name: name.trim().to_string(), tags: vec!["mobaxterm".into()], ..Default::default() };
    if input.name.is_empty() {
        return Err("no session name".into());
    }

    match kind {
        // SSH และ SFTP นำเข้าเป็นเซสชัน SSH (SFTP ของ NexaTerm เปิดจากเซสชัน SSH)
        0 | 7 => {
            let host = host.ok_or("no host")?;
            let username = non_empty(fields.get(3)).ok_or("no username")?;
            input.protocol = "ssh".into();
            input.host = Some(host);
            input.port = Some(parse_port(fields.get(2), 22));
            input.username = Some(username);
            let mut notes: Vec<String> = vec![];
            if kind == 7 {
                notes.push("Imported from a MobaXterm SFTP session".into());
            }
            match find_key_path(&fields) {
                Some((path, note)) => {
                    input.auth_method = Some("key".into());
                    input.key_path = Some(path);
                    notes.extend(note);
                }
                None => input.auth_method = Some("password".into()),
            }
            if !notes.is_empty() {
                input.notes = Some(notes.join("\n"));
            }
            Ok(input)
        }
        1 => {
            input.protocol = "telnet".into();
            input.host = Some(host.ok_or("no host")?);
            input.port = Some(parse_port(fields.get(2), 23));
            Ok(input)
        }
        other => Err(match type_label(other) {
            Some(label) => format!("{label} not supported yet"),
            None => format!("unknown MobaXterm session type {other}"),
        }),
    }
}

fn looks_like_key_path(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    let file = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
    !file.is_empty()
        && !file.contains(char::is_whitespace)
        && (file.ends_with(".ppk") || file.ends_with(".pem") || file.ends_with(".key") || file.starts_with("id_"))
}

/// Scan the SSH field list (after host/port/user) for the first value that looks like a private
/// key path. The exact field index varies between MobaXterm versions, so position is not trusted.
/// Returns the translated path and an optional note for the session.
fn find_key_path(fields: &[&str]) -> Option<(String, Option<String>)> {
    let raw = fields.iter().skip(4).map(|f| unescape(f.trim())).find(|f| looks_like_key_path(f))?;
    Some(translate_placeholders(&raw))
}

fn translate_placeholders(raw: &str) -> (String, Option<String>) {
    let mut path = raw.to_string();
    let mut note = None;
    if let Some(rest) = path.strip_prefix("_CurrentDrive_:") {
        // MobaXterm portable resolves this to the drive it runs from; we cannot know it.
        path = format!("C:{rest}");
        note = Some(format!("Key path used MobaXterm _CurrentDrive_ ({raw}); assumed drive C:"));
    }
    if path.contains("_ProfileDir_") {
        let home = dirs::home_dir().map(|h| h.display().to_string()).unwrap_or_default();
        path = path.replace("_ProfileDir_", &home);
    }
    if path.contains("_MyDocuments_") {
        if let Some(docs) = dirs::document_dir() {
            path = path.replace("_MyDocuments_", &docs.display().to_string());
        }
    }
    (path, note)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{open_in_memory, repo};
    use crate::importers::import;

    const SAMPLE: &str = "\u{feff}; exported by MobaXterm\r\n\
[Bookmarks]\r\n\
SubRep=\r\n\
ImgNum=42\r\n\
root-box= #109#0%192.168.1.10%22%root%%-1%-1%%%%%0%0%0%%%-1%0%0%0%%1080%%0%0%1#MobaFont%10%0%0%-1%15%236,236,236%30,30,30%180,180,192%0%-1%0%%xterm%-1%-1%_Std_Colors_0_%80%24%0%1%-1%<none>%%0%0%-1#0# #-1\r\n\
\r\n\
[Bookmarks_1]\r\n\
SubRep=Production\\Web\r\n\
ImgNum=41\r\n\
web01= #109#0%10.0.0.5%2222%deploy%%-1%-1%%%%%0%0%0%_ProfileDir_\\.ssh\\id_ed25519%%-1%0%0%0%%1080%%0%0%1#MobaFont%10%0%0%-1%15#0# #-1\r\n\
web02= #109#0%10.0.0.6%99999%deploy%%-1%-1%%%%%0%0%0%_CurrentDrive_:\\keys\\web.ppk%%-1#MobaFont%10#0# #-1\r\n\
windows-jump= #91#4%10.0.0.9%3389%admin%0%0%0%0%-1%0%0%-1#MobaFont%10#0# #-1\r\n\
this line is malformed\r\n\
broken= #109#notanumber%%%\r\n\
\r\n\
[Bookmarks_2]\r\n\
SubRep=Production\\DB\r\n\
ImgNum=41\r\n\
db01= #109#0%db01.internal%22%postgres%%-1%-1%%%%%0%0%0%%%-1#MobaFont%10#0# #-1\r\n\
nouser= #109#0%db02.internal%22%%%-1%-1%%%%%0%0%0%%%-1#MobaFont%10#0# #-1\r\n\
files= #140#7%files.example.com%22%ftpuser%-1%0%%0#MobaFont%10#0# #-1\r\n\
console= #131#8%-1%115200%3%0%0%1%2%COM3#MobaFont%10#0# #-1\r\n\
switch= #98#1%10.0.0.1%%%-1#MobaFont%10#0# #-1\r\n\
\r\n\
[Misc]\r\n\
Ignored=#109#0%evil%22%x\r\n";

    fn find<'a>(r: &'a ParseResult, name: &str) -> &'a ImportedSession {
        r.sessions.iter().find(|s| s.input.name == name).unwrap_or_else(|| panic!("missing {name}"))
    }

    fn reason<'a>(r: &'a ParseResult, name: &str) -> &'a str {
        &r.skipped.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("not skipped: {name}")).1
    }

    #[test]
    fn parses_sessions_folders_and_skips() {
        let r = parse_mxtsessions(SAMPLE);
        let names: Vec<_> = r.sessions.iter().map(|s| s.input.name.as_str()).collect();
        assert_eq!(names, ["root-box", "web01", "web02", "db01", "files", "switch"]);

        let root = find(&r, "root-box");
        assert_eq!(root.folder, None);
        assert_eq!(root.input.protocol, "ssh");
        assert_eq!(root.input.host.as_deref(), Some("192.168.1.10"));
        assert_eq!(root.input.port, Some(22));
        assert_eq!(root.input.username.as_deref(), Some("root"));
        assert_eq!(root.input.auth_method.as_deref(), Some("password"));
        assert_eq!(root.input.key_path, None);
        assert_eq!(root.input.tags, ["mobaxterm"]);

        let web01 = find(&r, "web01");
        assert_eq!(web01.folder.as_deref(), Some("Production\\Web"));
        assert_eq!(web01.input.port, Some(2222));
        assert_eq!(web01.input.auth_method.as_deref(), Some("key"));
        let home = dirs::home_dir().unwrap().display().to_string();
        assert_eq!(web01.input.key_path.as_deref(), Some(format!("{home}\\.ssh\\id_ed25519").as_str()));

        let web02 = find(&r, "web02");
        assert_eq!(web02.input.port, Some(22), "out-of-range port falls back to default");
        assert_eq!(web02.input.key_path.as_deref(), Some("C:\\keys\\web.ppk"));
        assert!(web02.input.notes.as_deref().unwrap().contains("_CurrentDrive_"));

        let db01 = find(&r, "db01");
        assert_eq!(db01.folder.as_deref(), Some("Production\\DB"));

        let files = find(&r, "files");
        assert_eq!(files.input.protocol, "ssh");
        assert_eq!(files.input.username.as_deref(), Some("ftpuser"));

        let sw = find(&r, "switch");
        assert_eq!(sw.input.protocol, "telnet");
        assert_eq!(sw.input.port, Some(23));
        assert_eq!(sw.input.username, None);

        assert_eq!(reason(&r, "windows-jump"), "RDP not supported yet");
        assert_eq!(reason(&r, "console"), "Serial not supported yet");
        assert_eq!(reason(&r, "nouser"), "no username");
        assert_eq!(reason(&r, "broken"), "unrecognized MobaXterm session format");
        assert_eq!(r.skipped.len(), 4);
        assert!(!r.sessions.iter().any(|s| s.input.name == "Ignored"));
    }

    #[test]
    fn never_panics_on_garbage() {
        for t in ["", "[", "]", "[Bookmarks]\n=", "[Bookmarks]\nx=#", "[Bookmarks]\nx=##", "[Bookmarks]\nx=#1#0", "[Bookmarks]\nx=#1#99%h", "[Bookmarks_]\na=#1#0%h%22%u"] {
            let _ = parse_mxtsessions(t);
        }
        let r = parse_mxtsessions("[Bookmarks]\nx=#1#99%h");
        assert_eq!(r.skipped[0].1, "unknown MobaXterm session type 99");
        // "Bookmarks_" without a number is not a bookmarks section.
        assert!(parse_mxtsessions("[Bookmarks_]\na=#1#0%h%22%u").sessions.is_empty());
    }

    #[test]
    fn key_detection() {
        assert!(looks_like_key_path(r"C:\keys\server.PEM"));
        assert!(looks_like_key_path("/home/me/.ssh/id_rsa"));
        assert!(!looks_like_key_path("xterm"));
        assert!(!looks_like_key_path("-1"));
        assert!(!looks_like_key_path(""));
        assert!(!looks_like_key_path("cat id_rsa"));
    }

    #[test]
    fn imports_into_database() {
        let conn = open_in_memory().unwrap();
        let parsed = parse_mxtsessions(SAMPLE);
        let report = import(&conn, &parsed, "Imported (MobaXterm)").unwrap();
        assert_eq!(report.imported, 6);
        assert_eq!(report.skipped.len(), 4);

        let groups = repo::list_groups(&conn).unwrap();
        let parent = groups.iter().find(|g| g.name == "Imported (MobaXterm)").unwrap();
        assert_eq!(parent.parent_id, None);
        let web = groups.iter().find(|g| g.name == "Production / Web").unwrap();
        let db = groups.iter().find(|g| g.name == "Production / DB").unwrap();
        assert_eq!(web.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(db.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(groups.len(), 3);

        let sessions = repo::list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 6);
        assert!(sessions.iter().all(|s| s.tags == ["mobaxterm"]));
        let web01 = sessions.iter().find(|s| s.name == "web01").unwrap();
        assert_eq!(web01.group_id.as_deref(), Some(web.id.as_str()));
        let root = sessions.iter().find(|s| s.name == "root-box").unwrap();
        assert_eq!(root.group_id.as_deref(), Some(parent.id.as_str()));
        let sw = sessions.iter().find(|s| s.name == "switch").unwrap();
        assert_eq!(sw.protocol, "telnet");

        let report_json = serde_json::to_value(&report).unwrap();
        assert!(report_json.get("imported").is_some() && report_json.get("skipped").is_some());
    }

    #[test]
    fn import_skips_invalid_telnet_host_and_creates_no_empty_groups() {
        let conn = open_in_memory().unwrap();
        let parsed = parse_mxtsessions("[Bookmarks_1]\nSubRep=Lab\nbad= #98#1%-oProxyCommand=x%23\n");
        let report = import(&conn, &parsed, "Imported (MobaXterm)").unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].0, "bad");
        assert!(repo::list_groups(&conn).unwrap().is_empty());
    }
}
