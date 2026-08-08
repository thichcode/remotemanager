use tauri::State;
use crate::db::{AppState, operations};
use crate::security::input::{validate_host, validate_username};
use std::fs;

fn validate_import_port(port: i64) -> Result<i32, String> {
    if port < 1 || port > 65535 {
        return Err("Port must be between 1 and 65535".to_string());
    }
    Ok(port as i32)
}

#[tauri::command]
pub fn cmd_import_csv(state: State<AppState>, path: String) -> Result<(usize, Vec<String>), String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let mut reader = csv::Reader::from_reader(content.as_bytes());
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut imported = 0;
    let mut errors = Vec::new();

    for (i, result) in reader.records().enumerate() {
        match result {
            Ok(record) => {
                let name = record.get(0).unwrap_or("").trim().to_string();
                let host = record.get(1).unwrap_or("").trim().to_string();
                // Export format: name,host,port,protocol,username,tags,notes
                let raw_port = record.get(2).unwrap_or("").trim().to_string();
                let protocol = record.get(3).unwrap_or("ssh").trim().to_string();
                let username = record.get(4).unwrap_or("").trim().to_string();

                if name.is_empty() || host.is_empty() {
                    errors.push(format!("Row {}: name and host required", i + 2));
                    continue;
                }
                if let Err(e) = validate_host(&host) {
                    errors.push(format!("Row {}: {}", i + 2, e));
                    continue;
                }
                if let Err(e) = validate_username(&username) {
                    errors.push(format!("Row {}: {}", i + 2, e));
                    continue;
                }
                if protocol != "ssh" && protocol != "rdp" {
                    errors.push(format!("Row {}: invalid protocol '{}'", i + 2, protocol));
                    continue;
                }

                let port = if !raw_port.is_empty() {
                    match raw_port.parse::<i64>() {
                        Ok(p) => match validate_import_port(p) {
                            Ok(valid) => valid,
                            Err(e) => {
                                errors.push(format!("Row {}: {}", i + 2, e));
                                continue;
                            }
                        },
                        Err(_) => {
                            errors.push(format!("Row {}: invalid port '{}'", i + 2, raw_port));
                            continue;
                        }
                    }
                } else if protocol == "rdp" {
                    3389
                } else {
                    22
                };

                // Round-trip safety: if a server with the same identity already
                // exists, skip it instead of duplicating the row and detaching
                // the credential/ssh-key reference it may already hold.
                match operations::find_server_by_identity(&conn, &name, &host, port, &protocol) {
                    Ok(Some(existing_id)) => {
                        errors.push(format!("Row {}: '{}' already exists, skipped", i + 2, existing_id));
                        continue;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        errors.push(format!("Row {}: database error - {}", i + 2, e));
                        continue;
                    }
                }

                match operations::create_server(&conn, &name, &host, port, &protocol, &username, None, "", "", "", None, None) {
                    Ok(_) => imported += 1,
                    Err(e) => errors.push(format!("Row {}: {}", i + 2, e)),
                }
            }
            Err(e) => errors.push(format!("Row {}: parse error - {}", i + 2, e)),
        }
    }

    Ok((imported, errors))
}

#[tauri::command]
pub fn cmd_export_csv(state: State<AppState>, path: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let servers = operations::list_servers(&conn, None).map_err(|e| e.to_string())?;

    let mut writer = csv::Writer::from_path(&path).map_err(|e| e.to_string())?;
    writer.write_record(&["name", "host", "port", "protocol", "username", "tags", "notes"])
        .map_err(|e| e.to_string())?;

    for s in servers {
        writer.write_record(&[&s.name, &s.host, &s.port.to_string(), &s.protocol, &s.username, &s.tags, &s.notes])
            .map_err(|e| e.to_string())?;
    }

    writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn cmd_export_json(state: State<AppState>, path: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let servers = operations::list_servers(&conn, None).map_err(|e| e.to_string())?;
    let groups = operations::list_groups(&conn).map_err(|e| e.to_string())?;
    let settings = operations::get_settings(&conn).map_err(|e| e.to_string())?;

    let export = serde_json::json!({
        "version": "0.1.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "servers": servers,
        "groups": groups,
        "settings": settings,
    });

    fs::write(&path, serde_json::to_string_pretty(&export).unwrap())
        .map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn cmd_import_json(state: State<AppState>, path: String) -> Result<(usize, Vec<String>), String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;
    let data: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {}", e))?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;

    let mut imported = 0;
    let mut errors = Vec::new();

    if let Some(servers) = data["servers"].as_array() {
        for (i, s) in servers.iter().enumerate() {
            let name = s["name"].as_str().unwrap_or("").trim().to_string();
            let host = s["host"].as_str().unwrap_or("").trim().to_string();
            let protocol = s["protocol"].as_str().unwrap_or("ssh").trim().to_string();
            let username = s["username"].as_str().unwrap_or("").trim().to_string();
            let tags = s["tags"].as_str().unwrap_or("").to_string();
            let notes = s["notes"].as_str().unwrap_or("").to_string();
            let credential_id = s["credential_id"].as_str().map(|v| v.to_string());
            let ssh_key_id = s["ssh_key_id"].as_str().map(|v| v.to_string());
            let description = s["description"].as_str().unwrap_or("").to_string();

            if name.is_empty() || host.is_empty() {
                errors.push(format!("Server {}: name and host required", i + 1));
                continue;
            }
            if let Err(e) = validate_host(&host) {
                errors.push(format!("Server {}: {}", i + 1, e));
                continue;
            }
            if let Err(e) = validate_username(&username) {
                errors.push(format!("Server {}: {}", i + 1, e));
                continue;
            }
            if protocol != "ssh" && protocol != "rdp" {
                errors.push(format!("Server {}: invalid protocol '{}'", i + 1, protocol));
                continue;
            }
            let port = match validate_import_port(s["port"].as_i64().unwrap_or(if protocol == "rdp" { 3389 } else { 22 })) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!("Server {}: {}", i + 1, e));
                    continue;
                }
            };

            // Round-trip safety: preserve the credential/ssh-key references that
            // export_json writes, and do not duplicate an existing server row
            // (duplicating would detach the auth config from the row the user
            // actually sees and edits).
            match operations::find_server_by_identity(&conn, &name, &host, port, &protocol) {
                Ok(Some(existing_id)) => {
                    errors.push(format!("Server {}: '{}' already exists, skipped", i + 1, existing_id));
                    continue;
                }
                Ok(None) => {}
                Err(e) => {
                    errors.push(format!("Server {}: database error - {}", i + 1, e));
                    continue;
                }
            }

            match operations::create_server(
                &conn, &name, &host, port, &protocol, &username, None, &tags, &notes, &description,
                credential_id.as_deref(), ssh_key_id.as_deref(),
            ) {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("Server {}: {}", i + 1, e)),
            }
        }
    }

    Ok((imported, errors))
}
