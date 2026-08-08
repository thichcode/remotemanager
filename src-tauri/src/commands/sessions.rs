use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{Emitter, State};
use uuid::Uuid;

use crate::db::AppState;

fn spawn_reader_thread(mut reader: impl Read + Send + 'static, app: tauri::AppHandle, sid: String) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let _ = app.emit("ssh://output", serde_json::json!({ "sessionId": sid, "data": data }));
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_exit_monitor(app: tauri::AppHandle, sid: String, sessions: std::sync::Arc<crate::sessions::SessionManager>) {
    std::thread::spawn(move || {
        loop {
            // Keep the lock scoped: it must be dropped before `sessions.remove`
            // (which locks again) to avoid a self-deadlock.
            let status = {
                let mut guard = sessions.sessions.lock().unwrap_or_else(|e| e.into_inner());
                let Some(child) = guard.get_mut(&sid) else { break };
                child.try_wait().ok().flatten()
            };
            match status {
                Some(status) => {
                    let code = status.code().unwrap_or(-1);
                    let _ = app.emit("ssh://exit", serde_json::json!({ "sessionId": sid, "code": code }));
                    if let Some(mut child) = sessions.remove(&sid) {
                        let _ = child.wait();
                    }
                    break;
                }
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    });
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_open_ssh_session(
    app: tauri::AppHandle,
    state: State<AppState>,
    host: String,
    port: i32,
    username: String,
    server_id: Option<String>,
    server_name: Option<String>,
    ssh_key_id: Option<String>,
    credential_id: Option<String>,
) -> Result<String, String> {
    crate::security::input::validate_host(&host)?;
    if port < 1 || port > 65535 {
        return Err("Port must be between 1 and 65535".to_string());
    }

    let username = crate::commands::ssh::resolve_username(&state, username, credential_id.as_deref())?;
    crate::security::input::validate_username(&username)?;

    let mut extra_args: Vec<String> = Vec::new();
    if let Some(kid) = ssh_key_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(key_path) = crate::sshkeys::get_private_key_path(&conn, kid)? {
            crate::sshkeys::ensure_key_permissions(&key_path);
            extra_args.push("-i".to_string());
            extra_args.push(key_path);
        }
    }

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let name = server_name.unwrap_or_else(|| host.clone());
        let _ = crate::history::record(&conn, server_id.as_deref(), &name, &host, Some(port), "ssh", &username, ssh_key_id.as_deref());
        if let Some(sid) = server_id.as_deref() {
            let _ = crate::db::operations::touch_last_connected(&conn, sid);
        }
    }

    let mut cmd = Command::new("ssh");
    cmd.args(&extra_args);
    cmd.arg("-o").arg("IdentitiesOnly=yes");
    cmd.args(["-p", &port.to_string()]);
    cmd.arg("-t");
    cmd.arg(format!("{}@{}", username, host));
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to launch SSH: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture ssh stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture ssh stderr")?;
    let session_id = Uuid::new_v4().to_string();

    state.sessions.insert(session_id.clone(), child);
    spawn_reader_thread(stdout, app.clone(), session_id.clone());
    spawn_reader_thread(stderr, app.clone(), session_id.clone());
    spawn_exit_monitor(app, session_id.clone(), state.sessions.clone());

    Ok(session_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_write(state: State<AppState>, session_id: String, data: Vec<u8>) -> Result<(), String> {
    use std::io::Write;
    use std::time::Instant;
    const CHUNK: usize = 4096;
    const BUDGET: Duration = Duration::from_millis(2000);

    state.sessions.with_child(&session_id, |child| {
        let stdin = child.stdin.as_mut().ok_or("Session has no stdin")?;
        let deadline = Instant::now() + BUDGET;
        let mut offset = 0usize;
        while offset < data.len() {
            let end = (offset + CHUNK).min(data.len());
            match stdin.write(&data[offset..end]) {
                Ok(0) => {
                    if Instant::now() >= deadline {
                        return Err("Session stdin is stalled".to_string());
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(n) => offset += n,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        if Instant::now() >= deadline {
                            return Err("Session stdin is stalled".to_string());
                        }
                        std::thread::sleep(Duration::from_millis(10));
                    } else {
                        return Err(e.to_string());
                    }
                }
            }
        }
        Ok(())
    })
    .ok_or("Session not found")?
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_resize(state: State<AppState>, session_id: String, cols: i32, rows: i32) -> Result<(), String> {
    // Best-effort: no real PTY on Windows spawn; size is kept consistent by
    // xterm fit() on the frontend. Reserved for future CONPTY integration.
    let _ = (state, session_id, cols, rows);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_close(state: State<AppState>, session_id: String) -> Result<(), String> {
    if let Some(mut child) = state.sessions.remove(&session_id) {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_close_all(state: State<AppState>) -> Result<(), String> {
    let ids: Vec<String> = {
        let guard = state.sessions.sessions.lock().unwrap_or_else(|e| e.into_inner());
        guard.keys().cloned().collect()
    };
    for id in ids {
        if let Some(mut child) = state.sessions.remove(&id) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    Ok(())
}
