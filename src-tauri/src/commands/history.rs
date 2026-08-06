use tauri::State;
use crate::db::{AppState, operations};
use crate::history;

#[tauri::command]
pub fn cmd_list_history(state: State<AppState>) -> Result<Vec<operations::HistoryRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    history::list(&conn)
}

#[tauri::command]
pub fn cmd_clear_history(state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    history::clear(&conn)
}
