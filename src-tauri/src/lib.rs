//! NexaTerm core library.

pub mod applog;
pub mod commands;
pub mod credentials;
pub mod database;
pub mod error;
pub mod keys;
pub mod network;
pub mod security;
pub mod selftest;
pub mod sftp;
pub mod ssh;
pub mod state;
pub mod terminal;
pub mod tunnels;

use std::sync::Arc;
use tauri::Manager;

fn secret_store() -> Arc<dyn credentials::SecretStore> {
    #[cfg(windows)]
    {
        Arc::new(credentials::WindowsCredentialStore)
    }
    #[cfg(not(windows))]
    {
        // Non-Windows builds are for development only: secrets are kept in memory, never on disk.
        Arc::new(credentials::MemoryStore::default())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use commands::{app, sessions, sftp, terminal, tools};

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let loc = database::resolve_data_location();
            let data_dir = loc.dir.clone();
            applog::init(&data_dir);
            log::info!("NexaTerm {} starting; data dir {} (portable={})", env!("CARGO_PKG_VERSION"), data_dir.display(), loc.portable);
            if let Some(n) = &loc.notice {
                log::warn!("{n}");
            }
            let mut st = state::AppState::new(data_dir.clone(), secret_store()).map_err(|e| {
                log::error!("database open failed: {e}");
                // Show the reason instead of silently failing to start.
                let msg = format!("NexaTerm could not open its database in {}:\n\n{e}", data_dir.display());
                use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                app.dialog().message(msg.clone()).kind(MessageDialogKind::Error).title("NexaTerm").blocking_show();
                Box::<dyn std::error::Error>::from(msg)
            })?;
            st.portable = loc.portable;
            st.data_notice = loc.notice;
            app.manage(st);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Clean up child processes and network connections on exit.
                if let Some(st) = window.app_handle().try_state::<state::AppState>() {
                    log::info!("shutting down: closing terminals, SSH connections and tunnels");
                    st.terminals.close_all();
                    st.ssh_conns.lock().unwrap().clear();
                    tauri::async_runtime::block_on(st.tunnels.stop_all());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app::app_info,
            app::settings_get,
            app::settings_set,
            app::db_backup,
            app::db_restore,
            app::history_list,
            app::history_clear,
            app::job_cancel,
            sessions::sessions_list,
            sessions::session_create,
            sessions::session_update,
            sessions::session_delete,
            sessions::session_duplicate,
            sessions::session_set_favorite,
            sessions::session_move,
            sessions::groups_list,
            sessions::group_create,
            sessions::group_rename,
            sessions::group_delete,
            sessions::sessions_export,
            sessions::sessions_import,
            sessions::import_openssh_config,
            terminal::shells_list,
            terminal::terminal_open_local,
            terminal::terminal_open_ssh,
            terminal::terminal_write,
            terminal::terminal_broadcast,
            terminal::terminal_resize,
            terminal::terminal_close,
            terminal::terminals_list,
            terminal::terminal_log_start,
            terminal::terminal_log_stop,
            terminal::known_hosts_list,
            terminal::known_host_trust,
            terminal::known_host_remove,
            terminal::write_text_file,
            sftp::sftp_open,
            sftp::sftp_close,
            sftp::sftp_list,
            sftp::sftp_realpath,
            sftp::sftp_exists,
            sftp::sftp_mkdir,
            sftp::sftp_create_file,
            sftp::sftp_rename,
            sftp::sftp_delete,
            sftp::sftp_read_text,
            sftp::sftp_write_text,
            sftp::sftp_upload,
            sftp::sftp_download,
            sftp::local_exists,
            sftp::downloads_dir,
            tools::tunnels_list,
            tools::tunnel_save,
            tools::tunnel_delete,
            tools::tunnel_start,
            tools::tunnel_stop,
            tools::tunnel_statuses,
            tools::net_run_tool,
            tools::net_dns,
            tools::net_reverse,
            tools::net_tcp_probe,
            tools::net_interfaces,
            tools::net_hostname,
            tools::net_public_ip,
            tools::keys_list,
            tools::keys_default_dir,
            tools::key_generate,
            tools::key_import,
            tools::key_inspect,
            tools::key_change_passphrase,
            tools::key_forget,
            tools::snippets_list,
            tools::snippet_save,
            tools::snippet_delete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running NexaTerm");
}
