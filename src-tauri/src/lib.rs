mod backup;
mod commands;
mod db;
mod history;
mod paths;
mod security;
mod sshkeys;

use db::{AppState, init_connection};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let conn = init_connection();
    // Best-effort daily auto-backup (retain last 7). Never blocks startup on failure.
    let _ = backup::auto_backup(&conn, 7);
    let state = AppState {
        db: std::sync::Mutex::new(conn),
    };

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::servers::cmd_create_server,
            commands::servers::cmd_update_server,
            commands::servers::cmd_clone_server,
            commands::servers::cmd_delete_server,
            commands::servers::cmd_get_server,
            commands::servers::cmd_list_servers,
            commands::servers::cmd_toggle_favorite,
            commands::servers::cmd_search_servers,
            commands::groups::cmd_create_group,
            commands::groups::cmd_update_group,
            commands::groups::cmd_delete_group,
            commands::groups::cmd_list_groups,
            commands::ssh::cmd_launch_ssh,
            commands::ssh::cmd_launch_rdp,
            commands::ssh::cmd_ping,
            commands::credentials::cmd_create_credential,
            commands::credentials::cmd_update_credential,
            commands::credentials::cmd_delete_credential,
            commands::credentials::cmd_list_credentials,
            commands::credentials::cmd_test_credential,
            commands::import_export::cmd_import_csv,
            commands::import_export::cmd_export_csv,
            commands::import_export::cmd_export_json,
            commands::import_export::cmd_import_json,
            commands::settings::cmd_get_settings,
            commands::settings::cmd_update_settings,
            commands::settings::cmd_is_portable,
            commands::backup::cmd_backup,
            commands::backup::cmd_restore,
            commands::history::cmd_list_history,
            commands::history::cmd_clear_history,
            commands::sshkeys::cmd_import_ssh_key,
            commands::sshkeys::cmd_list_ssh_keys,
            commands::sshkeys::cmd_delete_ssh_key,
            commands::sshkeys::cmd_attach_key,
            commands::tags::cmd_list_tags,
            commands::tags::cmd_set_server_tags,
            commands::tags::cmd_list_tags_for_server,
            commands::tags::cmd_list_recent_servers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
