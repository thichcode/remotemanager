use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

#[derive(serde::Serialize)]
pub struct BackupSummary {
    pub file: String,
    pub db_size: u64,
    pub keys_count: usize,
}

pub fn create(conn: &rusqlite::Connection, target_path: &str) -> Result<BackupSummary, String> {
    let data_dir = crate::paths::data_dir();
    let keys_dir = crate::paths::keys_dir();

    // Ensure latest WAL checkpoint so data.db is consistent
    conn.pragma_update(None, "wal_checkpoint", "TRUNCATE").ok();

    let db_path = crate::paths::db_path();
    if !db_path.exists() {
        return Err("Database file not found".to_string());
    }

    let file = fs::File::create(target_path).map_err(|e| format!("Failed to create backup: {}", e))?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // manifest
    let manifest = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "schema_version": 2,
        "exported_at": chrono::Utc::now().to_rfc3339(),
    });
    zip_writer.start_file("manifest.json", options).map_err(|e| e.to_string())?;
    zip_writer.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes()).map_err(|e| e.to_string())?;

    // db
    let db_bytes = fs::read(&db_path).map_err(|e| e.to_string())?;
    zip_writer.start_file("data.db", options).map_err(|e| e.to_string())?;
    zip_writer.write_all(&db_bytes).map_err(|e| e.to_string())?;

    // wal if present
    let wal_path = data_dir.join("data.db-wal");
    if wal_path.exists() {
        let wal_bytes = fs::read(&wal_path).map_err(|e| e.to_string())?;
        zip_writer.start_file("data.db-wal", options).map_err(|e| e.to_string())?;
        zip_writer.write_all(&wal_bytes).map_err(|e| e.to_string())?;
    }

    // keys
    let mut keys_count = 0;
    if keys_dir.exists() {
        for entry in fs::read_dir(&keys_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let bytes = fs::read(entry.path()).map_err(|e| e.to_string())?;
                let arc_path = format!("keys/{}", name);
                zip_writer.start_file(&arc_path, options).map_err(|e| e.to_string())?;
                zip_writer.write_all(&bytes).map_err(|e| e.to_string())?;
                keys_count += 1;
            }
        }
    }

    zip_writer.finish().map_err(|e| e.to_string())?;

    Ok(BackupSummary {
        file: target_path.to_string(),
        db_size: db_bytes.len() as u64,
        keys_count,
    })
}

pub fn restore(target_path: &str) -> Result<(), String> {
    let file = fs::File::open(target_path).map_err(|e| format!("Failed to open backup: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid backup file: {}", e))?;

    // validate manifest
    let mut manifest_ok = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name() == "manifest.json" {
            manifest_ok = true;
            let mut content = String::new();
            entry.read_to_string(&mut content).map_err(|e| e.to_string())?;
            let _: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("Bad manifest: {}", e))?;
            break;
        }
    }
    if !manifest_ok {
        return Err("Backup file missing manifest.json".to_string());
    }

    let data_dir = crate::paths::data_dir();
    let backup_parent = data_dir.parent().unwrap_or(Path::new("."));
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let safety_dir = backup_parent.join(format!("data-backup-pre-restore-{}", ts));

    // move current data dir aside
    if data_dir.exists() {
        fs::rename(&data_dir, &safety_dir).map_err(|e| format!("Failed to preserve current data: {}", e))?;
    }
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

    // extract
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name == "manifest.json" {
            continue;
        }
        // sanitize
        let clean = name.trim_start_matches('/');
        let dest = data_dir.join(clean);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if entry.is_dir() {
            continue;
        }
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }

    // delete safety dir after successful restore (data preserved only on failure)
    let _ = fs::remove_dir_all(&safety_dir);

    Ok(())
}
