use tauri::State;
use crate::db::{AppState, operations};

fn validate_port(port: i32) -> Result<(), String> {
    if port < 1 || port > 65535 {
        return Err("Port must be between 1 and 65535".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn cmd_create_server(
    state: State<AppState>,
    name: String,
    host: String,
    port: i32,
    protocol: String,
    username: String,
    group_id: Option<String>,
    tags: String,
    notes: String,
    description: String,
    credential_id: Option<String>,
    ssh_key_id: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    if protocol != "ssh" && protocol != "rdp" {
        return Err("Protocol must be ssh or rdp".to_string());
    }
    validate_port(port)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::create_server(
        &conn, &name, &host, port, &protocol, &username,
        group_id.as_deref(), &tags, &notes, &description, credential_id.as_deref(), ssh_key_id.as_deref(),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_server(
    state: State<AppState>,
    id: String,
    name: String,
    host: String,
    port: i32,
    protocol: String,
    username: String,
    group_id: Option<String>,
    tags: String,
    notes: String,
    description: String,
    credential_id: Option<String>,
    ssh_key_id: Option<String>,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    validate_port(port)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::update_server(
        &conn, &id, &name, &host, port, &protocol, &username,
        group_id.as_deref(), &tags, &notes, &description, credential_id.as_deref(), ssh_key_id.as_deref(),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_clone_server(state: State<AppState>, id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::clone_server(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Server not found".to_string())
}

#[tauri::command]
pub fn cmd_delete_server(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::delete_server(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_server(
    state: State<AppState>,
    id: String,
) -> Result<Option<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::get_server(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_servers(
    state: State<AppState>,
    group_id: Option<String>,
) -> Result<Vec<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_servers(&conn, group_id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_toggle_favorite(state: State<AppState>, id: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::toggle_favorite(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_search_servers(
    state: State<AppState>,
    query: String,
) -> Result<Vec<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::search_servers(&conn, &query).map_err(|e| e.to_string())
}
