use rusqlite::Connection;
use crate::db::operations;

pub fn record(
    conn: &Connection,
    server_id: Option<&str>,
    server_name: &str,
    host: &str,
    port: Option<i32>,
    protocol: &str,
    username: &str,
    ssh_key_id: Option<&str>,
) -> Result<(), String> {
    operations::record_history(conn, server_id, server_name, host, port, protocol, username, ssh_key_id)
        .map_err(|e| e.to_string())
}

pub fn list(conn: &Connection) -> Result<Vec<operations::HistoryRow>, String> {
    operations::list_history(conn).map_err(|e| e.to_string())
}

pub fn clear(conn: &Connection) -> Result<(), String> {
    operations::clear_history(conn).map_err(|e| e.to_string())
}
