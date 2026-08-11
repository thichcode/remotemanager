use tauri::State;

use crate::db::AppState;
use crate::sftp::UploadAuth;

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_upload_files(
    state: State<AppState>,
    server_id: String,
    local_paths: Vec<String>,
) -> Result<String, String> {
    let (host, port, username, credential_id, ssh_key_id) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let server = crate::db::operations::get_server(&conn, &server_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Server not found".to_string())?;
        drop(conn);
        (server.host, server.port, server.username, server.credential_id, server.ssh_key_id)
    };

    let username = crate::commands::ssh::resolve_username(&state, username, credential_id.as_deref())?;
    let password = if let Some(cid) = credential_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let encrypted = crate::db::operations::get_credential_password(&conn, cid)
            .map_err(|e| e.to_string())?
            .ok_or("Credential not found")?;
        drop(conn);
        crate::security::decrypt(&encrypted).ok()
    } else {
        None
    };
    let key_path = if let Some(kid) = ssh_key_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        crate::sshkeys::get_private_key_path(&conn, kid).ok().flatten()
    } else {
        None
    };

    let auth = match (password, key_path) {
        (Some(pw), _) => UploadAuth::Password(pw),
        (None, Some(kp)) => UploadAuth::Key(kp),
        (None, None) => return Err("No password or SSH key available for this server".into()),
    };

    state.upload_jobs.start_upload(host, port, username, auth, local_paths)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_get_upload_progress(
    state: State<AppState>,
    job_id: String,
) -> Result<Option<crate::sftp::UploadProgress>, String> {
    Ok(state.upload_jobs.get_progress(&job_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_cancel_upload(state: State<AppState>, job_id: String) -> Result<(), String> {
    state.upload_jobs.cancel(&job_id);
    Ok(())
}