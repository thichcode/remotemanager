use tauri::State;
use crate::db::{AppState, operations};

#[tauri::command]
pub fn cmd_list_tags(state: State<AppState>) -> Result<Vec<operations::TagRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_set_server_tags(state: State<AppState>, host_id: String, names: Vec<String>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::set_server_tags(&conn, &host_id, &names).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_tags_for_server(state: State<AppState>, host_id: String) -> Result<Vec<operations::TagRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_tags_for_server(&conn, &host_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_recent_servers(state: State<AppState>, limit: usize) -> Result<Vec<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_recent_servers(&conn, limit).map_err(|e| e.to_string())
}
