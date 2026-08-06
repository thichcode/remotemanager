use tauri::State;
use crate::backup;
use crate::db::AppState;

#[tauri::command]
pub fn cmd_backup(state: State<AppState>, path: String) -> Result<backup::BackupSummary, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    backup::create(&conn, &path)
}

#[tauri::command]
pub fn cmd_restore(path: String) -> Result<(), String> {
    backup::restore(&path)
}
