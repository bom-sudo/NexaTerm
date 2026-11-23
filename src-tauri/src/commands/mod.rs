//! IPC command layer. Commands validate input, delegate to core modules, and map errors.

pub mod app;
pub mod sessions;
pub mod sftp;
pub mod terminal;
pub mod tools;

use crate::state::AppState;
use tauri::State;

pub type S<'a> = State<'a, AppState>;
