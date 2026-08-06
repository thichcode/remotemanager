use std::process::Command;

#[tauri::command]
pub fn cmd_launch_ssh(host: String, port: i32, username: String) -> Result<(), String> {
    validate_input(&host)?;
    validate_input(&username)?;

    // Use wt.exe with default profile if available, fallback to ssh directly
    let status = Command::new("wt.exe")
        .args(["ssh", &format!("{}@{}", username, host), "-p", &port.to_string()])
        .spawn();

    match status {
        Ok(_) => Ok(()),
        Err(_) => {
            // Fallback: launch ssh directly in a new cmd window
            Command::new("cmd")
                .args(["/C", "start", "ssh", &format!("{}@{}", username, host), "-p", &port.to_string()])
                .spawn()
                .map_err(|e| format!("Failed to launch SSH: {}", e))?;
            Ok(())
        }
    }
}

#[tauri::command]
pub fn cmd_launch_rdp(
    host: String,
    username: String,
    fullscreen: bool,
    admin_mode: bool,
) -> Result<(), String> {
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    validate_input(&host)?;

    let mut rdp_content = format!(
        "full address:s:{}\r\nusername:s:{}\r\nscreen mode id:i:{}\r\n",
        host,
        username,
        if fullscreen { 2 } else { 1 }
    );

    if admin_mode {
        rdp_content.push_str("administrative session:i:1\r\n");
    }

    let temp_path = std::env::temp_dir().join(format!("rm_{}.rdp", host.replace('.', "_")));
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
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    validate_input(&host)?;

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

fn validate_input(input: &str) -> Result<(), String> {
    if input.contains(';') || input.contains('|') || input.contains('&') || input.contains('`') {
        return Err("Invalid characters in input".to_string());
    }
    Ok(())
}
