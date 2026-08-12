pub mod operations;
pub mod schema;

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub sessions: Arc<crate::backend::sessions::SessionManager>,
    pub rdp_sessions: Mutex<HashMap<u16, tokio::sync::oneshot::Sender<()>>>,
    pub terminal_sessions: Mutex<HashMap<u16, tokio::sync::oneshot::Sender<()>>>,
    pub upload_jobs: crate::backend::sftp::SftpBrowserManager,
}

pub fn get_db_path() -> PathBuf {
    crate::backend::paths::db_path()
}

pub fn init_connection() -> Result<Connection, String> {
    let path = get_db_path();
    eprintln!("[init] DB path: {}", path.display());

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create DB directory {}: {}", parent.display(), e))?;
    }

    let conn = Connection::open(&path)
        .map_err(|e| format!("Failed to open database at {}: {}", path.display(), e))?;
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "foreign_keys", "ON").ok();
    schema::create_tables(&conn).map_err(|e| format!("Failed to create tables: {}", e))?;
    schema::migrate(&conn).map_err(|e| format!("Failed to migrate database: {}", e))?;
    Ok(conn)
}
