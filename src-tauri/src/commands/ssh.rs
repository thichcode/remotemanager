use std::process::Command;
use crate::security::input::{validate_host, validate_username};

/// If the caller did not provide a username but a credential is attached,
/// fill in the username from the credential vault. This avoids making the
/// operator re-type a username they already saved.
fn resolve_username(
    state: &tauri::State<crate::db::AppState>,
    username: String,
    credential_id: Option<&str>,
) -> Result<String, String> {
    if !username.trim().is_empty() {
        return Ok(username);
    }
    if let Some(cid) = credential_id {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(meta) = crate::db::operations::get_credential_meta(&conn, cid)
            .map_err(|e| e.to_string())?
        {
            return Ok(meta.username);
        }
    }
    Ok(username)
}

#[tauri::command]
pub fn cmd_launch_ssh(
    state: tauri::State<crate::db::AppState>,
    host: String,
    port: i32,
    username: String,
    server_id: Option<String>,
    server_name: Option<String>,
    ssh_key_id: Option<String>,
    credential_id: Option<String>,
) -> Result<(), String> {
    validate_host(&host)?;
    if port < 1 || port > 65535 {
        return Err("Port must be between 1 and 65535".to_string());
    }

    let username = resolve_username(&state, username, credential_id.as_deref())?;
    validate_username(&username)?;

    // Resolve key path if attached
    let mut extra_args: Vec<String> = Vec::new();
    if let Some(kid) = ssh_key_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(key_path) = crate::sshkeys::get_private_key_path(&conn, kid)? {
            // Re-apply restrictive ACL on every launch so pre-existing keys
            // imported by older builds (group access left behind by
            // `/inheritance:r`) no longer trigger "too open" and are not
            // silently ignored.
            crate::sshkeys::ensure_key_permissions(&key_path);
            extra_args.push("-i".to_string());
            extra_args.push(key_path);
        }
    }

    // Record session history
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let name = server_name.unwrap_or_else(|| host.clone());
        let _ = crate::history::record(&conn, server_id.as_deref(), &name, &host, Some(port), "ssh", &username, ssh_key_id.as_deref());
        if let Some(sid) = server_id.as_deref() {
            let _ = crate::db::operations::touch_last_connected(&conn, sid);
        }
    }

    // Build the ssh command. Windows Terminal forwards argv directly to ssh;
    // this path never passes through a shell, so the whitelist above is the
    // only command-injection surface and it is fully closed.
    let mut cmd = Command::new("wt.exe");
    cmd.arg("ssh");
    cmd.args(&extra_args);
    cmd.arg("-o");
    cmd.arg("IdentitiesOnly=yes");
    cmd.args(["-p", &port.to_string()]);
    cmd.arg(format!("{}@{}", username, host));

    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(_) => {
            // Fallback for systems without Windows Terminal: spawn ssh.exe
            // directly. We do NOT use `cmd /C start` because that is a shell
            // and would reintroduce the injection surface.
            let mut fallback = Command::new("ssh");
            fallback.args(&extra_args);
            fallback.args(["-o", "IdentitiesOnly=yes", "-p", &port.to_string()]);
            fallback.arg(format!("{}@{}", username, host));
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
                fallback.creation_flags(CREATE_NEW_CONSOLE);
            }
            fallback.spawn().map_err(|e| format!("Failed to launch SSH: {}", e))?;
            Ok(())
        }
    }
}

#[tauri::command]
pub fn cmd_launch_rdp(
    state: tauri::State<crate::db::AppState>,
    host: String,
    username: String,
    fullscreen: bool,
    admin_mode: bool,
    server_id: Option<String>,
    server_name: Option<String>,
    credential_id: Option<String>,
) -> Result<(), String> {
    validate_host(&host)?;
    let username = resolve_username(&state, username, credential_id.as_deref())?;
    validate_username(&username)?;

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let name = server_name.unwrap_or_else(|| host.clone());
        let _ = crate::history::record(&conn, server_id.as_deref(), &name, &host, Some(3389), "rdp", &username, None);
        if let Some(sid) = server_id.as_deref() {
            let _ = crate::db::operations::touch_last_connected(&conn, sid);
        }
    }

    let mut rdp_content = format!(
        "full address:s:{}\r\nusername:s:{}\r\nscreen mode id:i:{}\r\n",
        host,
        username,
        if fullscreen { 2 } else { 1 }
    );

    if admin_mode {
        rdp_content.push_str("administrative session:i:1\r\n");
    }

    // Build a safe temp filename from the validated host (host may contain
    // IPv6 colons and brackets, which are invalid in Windows filenames).
    let safe_host: String = host
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let temp_path = std::env::temp_dir().join(format!("rm_{}.rdp", safe_host));
    std::fs::write(&temp_path, rdp_content)
        .map_err(|e| format!("Failed to create RDP file: {}", e))?;

    Command::new("mstsc.exe")
        .arg(temp_path.to_str().unwrap())
        .spawn()
        .map_err(|e| format!("Failed to launch RDP: {}", e))?;

    // Schedule cleanup of temp file (mstsc reads it on launch)
    let cleanup_path = temp_path.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let _ = std::fs::remove_file(&cleanup_path);
    });

    Ok(())
}

#[tauri::command]
pub fn cmd_ping(host: String) -> Result<String, String> {
    validate_host(&host)?;

    let output = Command::new("ping")
        .args(["-n", "1", "-w", "3000", &host])
        .output()
        .map_err(|e| format!("Ping failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if output.status.success() {
        if let Some(pos) = stdout.find("time=") {
            let start = pos + 5;
            if let Some(end) = stdout[start..].find("ms") {
                let latency = &stdout[start..start + end];
                return Ok(format!("Reachable ({}ms)", latency.trim()));
            }
        }
        if stdout.contains("time<1ms") {
            return Ok("Reachable (<1ms)".to_string());
        }
        Ok("Reachable".to_string())
    } else {
        Ok("Unreachable".to_string())
    }
}
