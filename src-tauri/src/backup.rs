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

pub fn restore(target_path: &str) -> Result<String, String> {
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
            let manifest: serde_json::Value =
                serde_json::from_str(&content).map_err(|e| format!("Bad manifest: {}", e))?;
            if manifest.get("schema_version").map(|v| v.as_i64().unwrap_or(0) > 3).unwrap_or(true) {
                return Err("Backup schema is newer than this app version. Update Remote Manager first.".to_string());
            }
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

    // Extract to a staging directory first, so we never touch live data until
    // the archive is fully validated.
    let staging_dir = backup_parent.join(format!("data-restore-staging-{}", ts));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&staging_dir).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name == "manifest.json" {
            continue;
        }
        let clean = name.trim_start_matches('/');
        let dest = staging_dir.join(clean);

        // CRITICAL: prevent zip-slip / path traversal. The destination must
        // stay strictly inside the staging directory.
        let canonical_base = fs::canonicalize(&staging_dir).map_err(|e| e.to_string())?;
        let candidate = if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf())
        } else {
            canonical_base.clone()
        };
        if !candidate.starts_with(&canonical_base) {
            return Err(format!("Backup contains an unsafe path: {}", name));
        }
        if dest.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))) {
            return Err(format!("Backup contains an unsafe path: {}", name));
        }

        if entry.is_dir() {
            continue;
        }
        let mut out = fs::File::create(&dest).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }

    // Validate the restored DB before swapping: open it and run integrity check.
    let staged_db = staging_dir.join("data.db");
    if !staged_db.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err("Backup does not contain data.db".to_string());
    }
    {
        let check = rusqlite::Connection::open(&staged_db)
            .and_then(|c| c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0)));
        match check {
            Ok(s) if s.to_lowercase().starts_with("ok") => {}
            Ok(s) => {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(format!("Restored database failed integrity check: {}", s));
            }
            Err(e) => {
                let _ = fs::remove_dir_all(&staging_dir);
                return Err(format!("Restored database could not be opened: {}", e));
            }
        }
    }

    // Swap: move current data dir aside (safety copy) and promote staging.
    if data_dir.exists() {
        if safety_dir.exists() {
            fs::remove_dir_all(&safety_dir).map_err(|e| e.to_string())?;
        }
        fs::rename(&data_dir, &safety_dir).map_err(|e| format!("Failed to preserve current data: {}", e))?;
    }
    fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(&staging_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = data_dir.join(entry.file_name());
        if target.exists() {
            fs::remove_file(&target).map_err(|e| e.to_string())?;
        }
        fs::rename(entry.path(), &target).map_err(|e| e.to_string())?;
    }
    fs::remove_dir_all(&staging_dir).map_err(|e| e.to_string())?;

    // The pre-restore copy is intentionally KEPT so a bad restore can be
    // reverted by the operator. Surface its location in the message.
    Ok(safety_dir.to_string_lossy().to_string())
}

/// Auto-backup: creates one backup per day into `data_dir/backups/` and
/// retains at most `retain` most recent. Best-effort, never fails startup.
pub fn auto_backup(conn: &rusqlite::Connection, retain: usize) -> Result<String, String> {
    let backup_root = crate::paths::data_dir().join("backups");
    fs::create_dir_all(&backup_root).map_err(|e| e.to_string())?;

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let target = backup_root.join(format!("auto-{}.rmbackup", date));

    if target.exists() {
        // A backup for today already exists.
        prune_auto_backups(&backup_root, retain);
        return Ok(target.to_string_lossy().to_string());
    }

    let result = create(conn, target.to_str().unwrap());
    prune_auto_backups(&backup_root, retain);
    result.map(|_| target.to_string_lossy().to_string())
}

fn prune_auto_backups(root: &Path, retain: usize) {
    if retain == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<std::path::PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "rmbackup").unwrap_or(false))
        .map(|e| e.path())
        .collect();
    files.sort();
    // `files` sorted lexicographically == chronologically (YYYY-MM-DD prefix).
    let overflow = files.len().saturating_sub(retain);
    for f in files.iter().take(overflow) {
        let _ = fs::remove_file(f);
    }
}
