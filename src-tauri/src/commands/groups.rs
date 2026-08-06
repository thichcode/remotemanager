use tauri::State;
use crate::db::{AppState, operations};

#[tauri::command]
pub fn cmd_create_group(
    state: State<AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Group name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::create_group(&conn, &name, parent_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_group(state: State<AppState>, id: String, name: String) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Group name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::update_group(&conn, &id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_group(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::delete_group(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_groups(state: State<AppState>) -> Result<Vec<operations::GroupRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_groups(&conn).map_err(|e| e.to_string())
}
