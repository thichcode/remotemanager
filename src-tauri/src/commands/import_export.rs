use tauri::State;
use crate::db::{AppState, operations};
use std::fs;

fn validate_import_host(host: &str) -> Result<(), String> {
    if host.contains(';') || host.contains('|') || host.contains('&') || host.contains('`') {
        return Err("Host contains invalid characters".to_string());
    }
    Ok(())
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
                let protocol = record.get(2).unwrap_or("ssh").trim().to_string();
                let username = record.get(3).unwrap_or("").trim().to_string();

                if name.is_empty() || host.is_empty() {
                    errors.push(format!("Row {}: name and host required", i + 2));
                    continue;
                }
                if let Err(e) = validate_import_host(&host) {
                    errors.push(format!("Row {}: {}", i + 2, e));
                    continue;
                }
                if protocol != "ssh" && protocol != "rdp" {
                    errors.push(format!("Row {}: invalid protocol '{}'", i + 2, protocol));
                    continue;
                }

                let port = if protocol == "rdp" { 3389 } else { 22 };
                match operations::create_server(&conn, &name, &host, port, &protocol, &username, None, "", "", None) {
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
            let port = s["port"].as_i64().unwrap_or(22) as i32;
            let tags = s["tags"].as_str().unwrap_or("").to_string();
            let notes = s["notes"].as_str().unwrap_or("").to_string();

            if name.is_empty() || host.is_empty() {
                errors.push(format!("Server {}: name and host required", i + 1));
                continue;
            }
            if let Err(e) = validate_import_host(&host) {
                errors.push(format!("Server {}: {}", i + 1, e));
                continue;
            }

            match operations::create_server(&conn, &name, &host, port, &protocol, &username, None, &tags, &notes, None) {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("Server {}: {}", i + 1, e)),
            }
        }
    }

    Ok((imported, errors))
}
