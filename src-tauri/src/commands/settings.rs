use tauri::State;
use crate::db::{AppState, operations};

#[tauri::command]
pub fn cmd_get_settings(state: State<AppState>) -> Result<operations::SettingsRow, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::get_settings(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_settings(
    state: State<AppState>,
    theme: String,
    font_size: i32,
    ssh_port: i32,
    rdp_fullscreen: bool,
    rdp_admin_mode: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::update_settings(&conn, &theme, font_size, ssh_port, rdp_fullscreen, rdp_admin_mode)
        .map_err(|e| e.to_string())
}
