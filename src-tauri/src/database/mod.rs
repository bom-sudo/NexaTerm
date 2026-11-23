//! SQLite persistence: connection setup, versioned migrations, backup/restore.

pub mod models;
pub mod repo;

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Ordered list of migrations. Append only — never edit a shipped migration.
const MIGRATIONS: &[(i64, &str, &str)] = &[(1, "initial", include_str!("../../migrations/0001_initial.sql"))];

pub fn open(path: &Path) -> AppResult<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let conn = Connection::open(path)?;
    configure(&conn)?;
    check_integrity(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn check_integrity(conn: &Connection) -> AppResult<()> {
    let res: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| AppError::Database(format!("Database file is unreadable or corrupted: {e}")))?;
    if res != "ok" {
        return Err(AppError::Database(format!(
            "Database integrity check failed ({res}). Restore a backup or move the file aside."
        )));
    }
    Ok(())
}

pub fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS database_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at TEXT NOT NULL);",
    )?;
    let current: i64 = conn.query_row("SELECT COALESCE(MAX(version),0) FROM database_migrations", [], |r| r.get(0))?;
    for (version, name, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)
            .map_err(|e| AppError::Database(format!("migration {version} ({name}) failed: {e}")))?;
        tx.execute(
            "INSERT INTO database_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![version, name, now()],
        )?;
        tx.commit()?;
    }
    Ok(())
}

pub fn schema_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(version),0) FROM database_migrations", [], |r| r.get(0))?)
}

/// Online backup to a standalone file (VACUUM INTO produces a consistent, compact copy).
pub fn backup_to(conn: &Connection, dest: &Path) -> AppResult<()> {
    if dest.exists() {
        std::fs::remove_file(dest)?;
    }
    let dest_str = dest.to_str().ok_or_else(|| AppError::Validation("Invalid backup path".into()))?;
    conn.execute("VACUUM INTO ?1", [dest_str])?;
    Ok(())
}

/// Validate a backup file before it replaces the live database.
pub fn validate_backup(src: &Path) -> AppResult<()> {
    let c = Connection::open_with_flags(src, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    check_integrity(&c)?;
    let has: i64 = c.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('sessions','database_migrations')",
        [],
        |r| r.get(0),
    )?;
    if has != 2 {
        return Err(AppError::Validation("File is not a NexaTerm database backup".into()));
    }
    Ok(())
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Where NexaTerm keeps its database, logs and backups.
#[derive(Debug, Clone)]
pub struct DataLocation {
    pub dir: PathBuf,
    pub portable: bool,
    /// Shown to the user when the preferred location could not be used.
    pub notice: Option<String>,
}

pub const PORTABLE_DIR: &str = "portable-data";

/// Portable mode is active when `portable-data\` (or the legacy `portable.flag`) sits next to the
/// executable. Installed mode uses `%APPDATA%\NexaTerm`.
pub fn resolve_data_location() -> DataLocation {
    // Explicit override for automation, testing and screenshots: NEXATERM_DATA_DIR=<folder>.
    if let Some(dir) = std::env::var_os("NEXATERM_DATA_DIR").filter(|d| !d.is_empty()) {
        return DataLocation { dir: PathBuf::from(dir), portable: false, notice: None };
    }
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from));
    if let Some(exe_dir) = exe_dir {
        if let Some(loc) = portable_location(&exe_dir) {
            return loc;
        }
    }
    DataLocation { dir: dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("NexaTerm"), portable: false, notice: None }
}

/// Portable resolution for a given executable directory (separated for tests).
pub fn portable_location(exe_dir: &Path) -> Option<DataLocation> {
    let pd = exe_dir.join(PORTABLE_DIR);
    if !(pd.is_dir() || exe_dir.join("portable.flag").exists()) {
        return None;
    }
    match ensure_writable(&pd) {
        Ok(()) => Some(DataLocation { dir: pd, portable: true, notice: None }),
        Err(e) => {
            // ตำแหน่งพกพาเขียนไม่ได้ (เช่น อยู่บนแผ่น/โฟลเดอร์อ่านอย่างเดียว) — ใช้ LocalAppData แทนและแจ้งผู้ใช้
            let fallback = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("NexaTerm").join("portable-fallback");
            Some(DataLocation {
                notice: Some(format!(
                    "The portable data folder \"{}\" is not writable ({e}). NexaTerm is storing data in \"{}\" instead.",
                    pd.display(),
                    fallback.display()
                )),
                dir: fallback,
                portable: true,
            })
        }
    }
}

/// Create the directory if needed and prove it is writable with a probe file.
pub fn ensure_writable(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    std::fs::write(&probe, b"ok")?;
    std::fs::remove_file(&probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_and_are_idempotent() {
        let c = open_in_memory().unwrap();
        assert_eq!(schema_version(&c).unwrap(), 1);
        migrate(&c).unwrap();
        assert_eq!(schema_version(&c).unwrap(), 1);
    }

    #[test]
    fn portable_detection() {
        let d = tempfile::tempdir().unwrap();
        assert!(portable_location(d.path()).is_none(), "no marker = installed mode");
        std::fs::create_dir(d.path().join(PORTABLE_DIR)).unwrap();
        let loc = portable_location(d.path()).unwrap();
        assert!(loc.portable && loc.notice.is_none());
        assert_eq!(loc.dir, d.path().join(PORTABLE_DIR));
    }

    #[test]
    fn backup_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("a.db");
        let c = open(&db).unwrap();
        let bk = dir.path().join("b.db");
        backup_to(&c, &bk).unwrap();
        validate_backup(&bk).unwrap();
        let junk = dir.path().join("junk.db");
        std::fs::write(&junk, b"not a database at all, definitely not").unwrap();
        assert!(validate_backup(&junk).is_err());
    }
}
