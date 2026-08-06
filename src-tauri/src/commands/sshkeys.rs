use tauri::State;
use crate::db::{AppState, operations};
use crate::sshkeys;

#[tauri::command]
pub fn cmd_import_ssh_key(
    state: State<AppState>,
    path: String,
    name: String,
    passphrase: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sshkeys::import_private_key(&conn, &path, &name, passphrase)
}

#[tauri::command]
pub fn cmd_list_ssh_keys(state: State<AppState>) -> Result<Vec<operations::SshKeyRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sshkeys::list(&conn)
}

#[tauri::command]
pub fn cmd_delete_ssh_key(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sshkeys::delete(&conn, &id)
}

#[tauri::command]
pub fn cmd_attach_key(state: State<AppState>, server_id: String, ssh_key_id: Option<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    sshkeys::attach(&conn, &server_id, ssh_key_id.as_deref())
}
