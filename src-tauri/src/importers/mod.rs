//! Session importers for other SSH clients (MobaXterm, PuTTY).
//!
//! Parsing is pure (text in, `ParseResult` out) so it can be unit tested without a database or
//! registry. `import` writes the parsed sessions in a single transaction and skips entries it
//! cannot represent instead of failing the whole import.

pub mod mobaxterm;
pub mod putty;

use crate::database::models::SessionInput;
use crate::database::repo;
use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;

/// One session produced by a parser, with the folder it lived in at the source.
#[derive(Debug, Clone)]
pub struct ImportedSession {
    pub input: SessionInput,
    /// Source folder path with `\` separators (e.g. `Production\Web`); `None` = root.
    pub folder: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    pub sessions: Vec<ImportedSession>,
    /// `(session name, reason)` for every entry that was recognised but not importable.
    pub skipped: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: Vec<(String, String)>,
}

/// Decode a session file of unknown encoding.
///
/// Order: UTF-16 with BOM (regedit exports), BOM-less UTF-16LE (heuristic), UTF-8 (BOM stripped),
/// then Windows-1252 for 0x80-0x9F and Latin-1 for everything else.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, false);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, true);
    }
    // BOM-less UTF-16LE: ASCII text has a zero high byte in every odd position.
    if bytes.len() >= 4 && bytes.len() % 2 == 0 && bytes[0] != 0 && bytes[1] == 0 && bytes[2] != 0 && bytes[3] == 0 {
        return decode_utf16(bytes, false);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1252_char(b)).collect(),
    }
}

fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| if big_endian { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Windows-1252 mapping for one byte. Bytes undefined in 1252 (0x81, 0x8D, 0x8F, 0x90, 0x9D)
/// fall back to their Latin-1 code point, like Windows' own MultiByteToWideChar does.
fn cp1252_char(b: u8) -> char {
    const HIGH: [u16; 32] = [
        0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D,
        0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A,
        0x0153, 0x009D, 0x017E, 0x0178,
    ];
    if (0x80..=0x9F).contains(&b) {
        char::from_u32(HIGH[(b - 0x80) as usize] as u32).unwrap_or(b as char)
    } else {
        b as char
    }
}

/// Validation beyond `repo::validate_session` for protocols the repo does not check in detail.
fn pre_validate(input: &SessionInput) -> AppResult<()> {
    repo::validate_session(input)?;
    if input.protocol == "telnet" {
        crate::security::validate_host(input.host.as_deref().unwrap_or(""))?;
        crate::security::validate_port(input.port.unwrap_or(23))?;
    }
    Ok(())
}

/// Write parsed sessions to the database in one transaction.
///
/// สร้างกลุ่มหลักชื่อ `parent_group_name` และกลุ่มย่อยหนึ่งกลุ่มต่อโฟลเดอร์ต้นทาง (แบบแบน ชื่อเป็น
/// path เต็มคั่นด้วย " / ") เซสชันที่ไม่ผ่านการตรวจสอบจะถูกข้ามและบันทึกเหตุผลไว้ในรายงาน
/// แทนที่จะทำให้การนำเข้าทั้งหมดล้มเหลว กลุ่มจะถูกสร้างเมื่อมีเซสชันที่นำเข้าได้จริงเท่านั้น
pub fn import(conn: &Connection, parsed: &ParseResult, parent_group_name: &str) -> AppResult<ImportReport> {
    let tx = conn.unchecked_transaction()?;
    let mut report = ImportReport { imported: 0, skipped: parsed.skipped.clone() };
    let mut parent_id: Option<String> = None;
    let mut folder_ids: HashMap<String, String> = HashMap::new();

    for s in &parsed.sessions {
        if let Err(e) = pre_validate(&s.input) {
            report.skipped.push((s.input.name.clone(), e.to_string()));
            continue;
        }
        let parent = match &parent_id {
            Some(id) => id.clone(),
            None => {
                let g = repo::create_group(&tx, parent_group_name, None)?;
                parent_id = Some(g.id.clone());
                g.id
            }
        };
        let group_id = match s.folder.as_deref().map(folder_display_name).filter(|n| !n.is_empty()) {
            None => parent,
            Some(name) => match folder_ids.get(&name) {
                Some(id) => id.clone(),
                None => {
                    let g = repo::create_group(&tx, &name, Some(&parent))?;
                    folder_ids.insert(name, g.id.clone());
                    g.id
                }
            },
        };
        let mut input = s.input.clone();
        input.group_id = Some(group_id);
        match repo::insert_session(&tx, &input, None) {
            Ok(_) => report.imported += 1,
            Err(e) => report.skipped.push((s.input.name.clone(), e.to_string())),
        }
    }
    tx.commit()?;
    Ok(report)
}

/// `Production\Web` -> `Production / Web` (empty segments dropped).
fn folder_display_name(path: &str) -> String {
    path.split(['\\', '/']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" / ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_with_bom() {
        assert_eq!(decode_text(b"\xEF\xBB\xBFabc"), "abc");
        assert_eq!(decode_text("h\u{e9}".as_bytes()), "h\u{e9}");
    }

    #[test]
    fn decodes_cp1252_and_latin1() {
        // 0x80 = euro sign in 1252, 0xE9 = e-acute (Latin-1), 0x93/0x94 = curly quotes.
        assert_eq!(decode_text(&[b'a', 0x80, 0xE9, 0x93, 0x94]), "a\u{20AC}\u{E9}\u{201C}\u{201D}");
        assert_eq!(decode_text(&[0x81]), "\u{81}");
    }

    #[test]
    fn decodes_utf16() {
        let mut le = vec![0xFF, 0xFE];
        le.extend("x=\u{0E01}".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(decode_text(&le), "x=\u{0E01}");
        let mut be = vec![0xFE, 0xFF];
        be.extend("ab".encode_utf16().flat_map(|u| u.to_be_bytes()));
        assert_eq!(decode_text(&be), "ab");
        let nobom: Vec<u8> = "abcd".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode_text(&nobom), "abcd");
    }

    #[test]
    fn folder_names_are_flattened() {
        assert_eq!(folder_display_name(r"Production\Web"), "Production / Web");
        assert_eq!(folder_display_name(r"\A\\B\"), "A / B");
    }
}
