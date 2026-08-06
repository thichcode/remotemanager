pub mod operations;
pub mod schema;

use rusqlite::Connection;
use std::sync::Mutex;
use std::path::PathBuf;

pub struct AppState {
    pub db: Mutex<Connection>,
}

pub fn get_db_path() -> PathBuf {
    let mut path = dirs::data_dir().expect("Failed to get data directory");
    path.push("remote-manager");
    std::fs::create_dir_all(&path).ok();
    path.push("data.db");
    path
}

pub fn init_connection() -> Connection {
    let path = get_db_path();
    let conn = Connection::open(&path).expect("Failed to open database");
    conn.pragma_update(None, "journal_mode", "WAL").ok();
    conn.pragma_update(None, "foreign_keys", "ON").ok();
    schema::create_tables(&conn).expect("Failed to create tables");
    conn
}
