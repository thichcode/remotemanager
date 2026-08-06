use rusqlite::{Connection, params, OptionalExtension};
use uuid::Uuid;

#[derive(Debug, serde::Serialize)]
pub struct ServerRow {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub protocol: String,
    pub username: String,
    pub group_id: Option<String>,
    pub tags: String,
    pub notes: String,
    pub favorite: bool,
    pub credential_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, serde::Serialize)]
pub struct GroupRow {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, serde::Serialize)]
pub struct CredentialRow {
    pub id: String,
    pub name: String,
    pub username: String,
    pub created_at: String,
}

#[derive(Debug, serde::Serialize)]
pub struct SettingsRow {
    pub id: i32,
    pub theme: String,
    pub font_size: i32,
    pub ssh_port: i32,
    pub rdp_fullscreen: bool,
    pub rdp_admin_mode: bool,
}

fn map_server_row(row: &rusqlite::Row) -> rusqlite::Result<ServerRow> {
    Ok(ServerRow {
        id: row.get(0)?,
        name: row.get(1)?,
        host: row.get(2)?,
        port: row.get(3)?,
        protocol: row.get(4)?,
        username: row.get(5)?,
        group_id: row.get(6)?,
        tags: row.get(7)?,
        notes: row.get(8)?,
        favorite: row.get::<_, i32>(9)? != 0,
        credential_id: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

// Server operations
pub fn create_server(
    conn: &Connection,
    name: &str,
    host: &str,
    port: i32,
    protocol: &str,
    username: &str,
    group_id: Option<&str>,
    tags: &str,
    notes: &str,
    credential_id: Option<&str>,
) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO servers (id, name, host, port, protocol, username, group_id, tags, notes, credential_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, name, host, port, protocol, username, group_id, tags, notes, credential_id],
    )?;
    Ok(id)
}

pub fn update_server(
    conn: &Connection,
    id: &str,
    name: &str,
    host: &str,
    port: i32,
    protocol: &str,
    username: &str,
    group_id: Option<&str>,
    tags: &str,
    notes: &str,
    credential_id: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE servers SET name=?2, host=?3, port=?4, protocol=?5, username=?6, group_id=?7,
         tags=?8, notes=?9, credential_id=?10, updated_at=datetime('now') WHERE id=?1",
        params![id, name, host, port, protocol, username, group_id, tags, notes, credential_id],
    )?;
    Ok(())
}

pub fn delete_server(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM servers WHERE id=?1", [id])?;
    Ok(())
}

pub fn get_server(conn: &Connection, id: &str) -> rusqlite::Result<Option<ServerRow>> {
    conn.query_row(
        "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite,
                credential_id, created_at, updated_at FROM servers WHERE id=?1",
        [id],
        map_server_row,
    ).optional()
}

pub fn list_servers(conn: &Connection, group_id: Option<&str>) -> rusqlite::Result<Vec<ServerRow>> {
    let query = match group_id {
        Some(_) => "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite,
                     credential_id, created_at, updated_at FROM servers WHERE group_id = ?1
                     ORDER BY favorite DESC, name ASC",
        None => "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite,
                 credential_id, created_at, updated_at FROM servers ORDER BY favorite DESC, name ASC",
    };

    let mut stmt = conn.prepare(query)?;
    let rows = match group_id {
        Some(gid) => stmt.query_map([gid], map_server_row)?,
        None => stmt.query_map([], map_server_row)?,
    };

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn toggle_favorite(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    conn.execute(
        "UPDATE servers SET favorite = NOT favorite, updated_at=datetime('now') WHERE id=?1",
        [id],
    )?;
    let fav: i32 = conn.query_row("SELECT favorite FROM servers WHERE id=?1", [id], |row| row.get(0))?;
    Ok(fav != 0)
}

pub fn search_servers(conn: &Connection, query: &str) -> rusqlite::Result<Vec<ServerRow>> {
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite,
                credential_id, created_at, updated_at
         FROM servers
         WHERE LOWER(name) LIKE ?1 OR LOWER(host) LIKE ?1 OR LOWER(tags) LIKE ?1
         ORDER BY favorite DESC, name ASC",
    )?;
    let rows = stmt.query_map([&pattern], map_server_row)?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

// Group operations
pub fn create_group(conn: &Connection, name: &str, parent_id: Option<&str>) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO groups (id, name, parent_id) VALUES (?1, ?2, ?3)",
        params![id, name, parent_id],
    )?;
    Ok(id)
}

pub fn update_group(conn: &Connection, id: &str, name: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE groups SET name=?2 WHERE id=?1", [id, name])?;
    Ok(())
}

pub fn delete_group(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM groups WHERE id=?1", [id])?;
    Ok(())
}

pub fn list_groups(conn: &Connection) -> rusqlite::Result<Vec<GroupRow>> {
    let mut stmt = conn.prepare("SELECT id, name, parent_id, sort_order FROM groups ORDER BY sort_order, name")?;
    let rows = stmt.query_map([], |row| {
        Ok(GroupRow {
            id: row.get(0)?,
            name: row.get(1)?,
            parent_id: row.get(2)?,
            sort_order: row.get(3)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

// Credential operations
pub fn create_credential(
    conn: &Connection,
    name: &str,
    username: &str,
    encrypted_password: &str,
) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO credentials (id, name, username, encrypted_password) VALUES (?1, ?2, ?3, ?4)",
        params![id, name, username, encrypted_password],
    )?;
    Ok(id)
}

pub fn delete_credential(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM credentials WHERE id=?1", [id])?;
    Ok(())
}

pub fn list_credentials(conn: &Connection) -> rusqlite::Result<Vec<CredentialRow>> {
    let mut stmt = conn.prepare("SELECT id, name, username, created_at FROM credentials ORDER BY name")?;
    let rows = stmt.query_map([], |row| {
        Ok(CredentialRow {
            id: row.get(0)?,
            name: row.get(1)?,
            username: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get_credential_password(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT encrypted_password FROM credentials WHERE id=?1",
        [id],
        |row| row.get(0),
    ).optional()
}

// Settings operations
pub fn get_settings(conn: &Connection) -> rusqlite::Result<SettingsRow> {
    conn.query_row(
        "SELECT id, theme, font_size, ssh_port, rdp_fullscreen, rdp_admin_mode FROM settings WHERE id=1",
        [],
        |row| {
            Ok(SettingsRow {
                id: row.get(0)?,
                theme: row.get(1)?,
                font_size: row.get(2)?,
                ssh_port: row.get(3)?,
                rdp_fullscreen: row.get::<_, i32>(4)? != 0,
                rdp_admin_mode: row.get::<_, i32>(5)? != 0,
            })
        },
    )
}

pub fn update_settings(
    conn: &Connection,
    theme: &str,
    font_size: i32,
    ssh_port: i32,
    rdp_fullscreen: bool,
    rdp_admin_mode: bool,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE settings SET theme=?1, font_size=?2, ssh_port=?3, rdp_fullscreen=?4, rdp_admin_mode=?5 WHERE id=1",
        params![theme, font_size, ssh_port, rdp_fullscreen as i32, rdp_admin_mode as i32],
    )?;
    Ok(())
}
