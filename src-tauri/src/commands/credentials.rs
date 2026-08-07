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
pub fn cmd_update_credential(
    state: State<AppState>,
    id: String,
    name: String,
    username: String,
    password: Option<String>,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let encrypted = match password {
        Some(p) if !p.is_empty() => Some(security::encrypt(&p)?),
        _ => None,
    };
    operations::update_credential(&conn, &id, &name, &username, encrypted.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_test_credential(
    state: State<AppState>,
    id: String,
    host: String,
    port: Option<i32>,
) -> Result<String, String> {
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let encrypted = operations::get_credential_password(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or("Credential not found")?;
    let row = operations::get_credential_meta(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or("Credential not found")?;
    let password = security::decrypt(&encrypted)?;
    crate::security::net::test_ssh_auth(&host, port.unwrap_or(22), &row.username, Some(&password), None)
}
