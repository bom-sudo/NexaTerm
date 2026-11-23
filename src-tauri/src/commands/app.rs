use super::S;
use crate::database::{self, models::HistoryEntry, repo};
use crate::error::{AppError, AppResult};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
    portable: bool,
    data_notice: Option<String>,
    schema_version: i64,
    os: String,
    /// Dev builds only: scene name for README screenshots (NEXATERM_SHOWCASE).
    showcase: Option<String>,
}

#[tauri::command]
pub fn app_info(state: S) -> AppResult<AppInfo> {
    let db = state.db.lock().unwrap();
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: state.data_dir.display().to_string(),
        portable: state.portable,
        data_notice: state.data_notice.clone(),
        schema_version: database::schema_version(&db)?,
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        showcase: if cfg!(debug_assertions) { std::env::var("NEXATERM_SHOWCASE").ok() } else { None },
    })
}

#[tauri::command]
pub fn settings_get(state: S) -> AppResult<serde_json::Map<String, serde_json::Value>> {
    repo::get_settings(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn settings_set(state: S, key: String, value: serde_json::Value) -> AppResult<()> {
    repo::set_setting(&state.db.lock().unwrap(), &key, &value)
}

#[tauri::command]
pub fn db_backup(state: S, path: String) -> AppResult<()> {
    let dest = PathBuf::from(&path);
    if dest.extension().is_none() {
        return Err(AppError::Validation("Backup file needs an extension (e.g. .db)".into()));
    }
    database::backup_to(&state.db.lock().unwrap(), &dest)
}

/// Restore replaces the live database content after validating the file. Secrets in Credential
/// Manager are untouched; restored sessions keep working on the same Windows account.
#[tauri::command]
pub fn db_restore(state: S, path: String) -> AppResult<()> {
    let src = PathBuf::from(&path);
    database::validate_backup(&src)?;
    let mut db = state.db.lock().unwrap();
    // Safety copy of the current DB before restore.
    let safety = state.data_dir.join(format!("pre-restore-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    database::backup_to(&db, &safety)?;
    db.restore(rusqlite::MAIN_DB, &src, None::<fn(rusqlite::backup::Progress)>)?;
    database::migrate(&db)?;
    Ok(())
}

#[tauri::command]
pub fn history_list(state: S, limit: Option<i64>) -> AppResult<Vec<HistoryEntry>> {
    repo::list_history(&state.db.lock().unwrap(), limit.unwrap_or(50).clamp(1, 500))
}

#[tauri::command]
pub fn history_clear(state: S) -> AppResult<()> {
    repo::clear_history(&state.db.lock().unwrap())
}

#[tauri::command]
pub fn job_cancel(state: S, job_id: String) -> bool {
    state.cancel_job(&job_id)
}
