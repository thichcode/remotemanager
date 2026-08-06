use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    // Set schema version for future migrations
    conn.pragma_update(None, "user_version", "1")?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            parent_id TEXT,
            sort_order INTEGER DEFAULT 0,
            FOREIGN KEY (parent_id) REFERENCES groups(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS servers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            host TEXT NOT NULL,
            port INTEGER DEFAULT 22,
            protocol TEXT NOT NULL CHECK(protocol IN ('ssh', 'rdp')),
            username TEXT DEFAULT '',
            group_id TEXT,
            tags TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            favorite INTEGER DEFAULT 0,
            credential_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE SET NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS credentials (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            username TEXT DEFAULT '',
            encrypted_password TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            theme TEXT DEFAULT 'dark' CHECK(theme IN ('light', 'dark')),
            font_size INTEGER DEFAULT 14,
            ssh_port INTEGER DEFAULT 22,
            rdp_fullscreen INTEGER DEFAULT 0,
            rdp_admin_mode INTEGER DEFAULT 0
        )",
        [],
    )?;

    conn.execute("INSERT OR IGNORE INTO settings (id) VALUES (1)", [])?;

    conn.execute("CREATE INDEX IF NOT EXISTS idx_servers_group ON servers(group_id)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_servers_favorite ON servers(favorite)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name)", [])?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_groups_parent ON groups(parent_id)", [])?;

    Ok(())
}
