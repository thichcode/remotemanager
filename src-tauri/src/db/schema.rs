use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    // Set schema version for future migrations (only on a fresh database so
    // existing users keep their current version and migrations run once).
    let fresh: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='groups'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n == 0)
        .unwrap_or(true);
    if fresh {
        conn.pragma_update(None, "user_version", "1")?;
    }

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

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;

    if version < 2 {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS ssh_keys (
                id            TEXT PRIMARY KEY,
                name          TEXT NOT NULL,
                private_key   TEXT NOT NULL,
                public_key    TEXT DEFAULT '',
                passphrase    TEXT DEFAULT '',
                created_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS session_history (
                id            TEXT PRIMARY KEY,
                server_id     TEXT,
                server_name   TEXT NOT NULL,
                host          TEXT NOT NULL,
                port          INTEGER,
                protocol      TEXT NOT NULL CHECK(protocol IN ('ssh','rdp')),
                username      TEXT DEFAULT '',
                ssh_key_id    TEXT,
                connected_at  TEXT NOT NULL DEFAULT (datetime('now')),
                status        TEXT DEFAULT 'success'
            );

            CREATE INDEX IF NOT EXISTS idx_history_connected ON session_history(connected_at DESC);
            ",
        )?;

        // Guarded ALTER for ssh_key_id on servers
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(servers)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<_, _>>()?;
        if !cols.iter().any(|c| c == "ssh_key_id") {
            conn.execute_batch("ALTER TABLE servers ADD COLUMN ssh_key_id TEXT;")?;
        }

        conn.pragma_update(None, "user_version", 2)?;
    }

    if version < 3 {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tags (
                id       TEXT PRIMARY KEY,
                name     TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS host_tags (
                host_id  TEXT NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
                tag_id   TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (host_id, tag_id)
            );

            CREATE INDEX IF NOT EXISTS idx_host_tags_tag ON host_tags(tag_id);
            ",
        )?;

        // Guarded ALTERs
        let cols: Vec<String> = conn
            .prepare("PRAGMA table_info(servers)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<_, _>>()?;
        if !cols.iter().any(|c| c == "description") {
            conn.execute_batch("ALTER TABLE servers ADD COLUMN description TEXT DEFAULT '';")?;
        }
        if !cols.iter().any(|c| c == "last_connected_at") {
            conn.execute_batch("ALTER TABLE servers ADD COLUMN last_connected_at TEXT;")?;
        }

        conn.pragma_update(None, "user_version", 3)?;
    }

    Ok(())
}
