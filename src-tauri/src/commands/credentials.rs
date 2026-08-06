use tauri::State;
use crate::db::{AppState, operations};
use crate::security;

#[tauri::command]
pub fn cmd_create_credential(
    state: State<AppState>,
    name: String,
    username: String,
    password: String,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    let encrypted = security::encrypt(&password)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::create_credential(&conn, &name, &username, &encrypted)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_credential(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::delete_credential(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_credentials(state: State<AppState>) -> Result<Vec<operations::CredentialRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_credentials(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_credential_password(state: State<AppState>, id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let encrypted = operations::get_credential_password(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or("Credential not found")?;
    // SECURITY NOTE: This returns plaintext password over IPC.
    // In production, consider using credential to auto-fill connection
    // without exposing the password to the frontend.
    security::decrypt(&encrypted)
}
