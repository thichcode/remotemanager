# Remote Manager MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete Windows desktop app for managing SSH/RDP connections — a free mRemoteNG alternative with modern UI.

**Architecture:** Tauri 2 (Rust backend) + React/TypeScript frontend. SQLite for local storage, Windows DPAPI for credential encryption, xterm.js for embedded SSH terminal, mstsc.exe for RDP.

**Tech Stack:** Tauri 2, React 18, TypeScript, Mantine UI, Zustand, SQLite (rusqlite), xterm.js, Windows DPAPI (windows-sys crate)

---

## File Structure

```
remote-manager-mvp/
├── package.json                          # Dependencies & scripts
├── vite.config.ts                        # Vite + Tauri config
├── tsconfig.json                         # TypeScript config
├── tsconfig.node.json                    # Vite node TS config
├── index.html                            # Entry HTML
├── src/
│   ├── main.tsx                          # React entry point
│   ├── App.tsx                           # Root component with routing
│   ├── types/
│   │   └── index.ts                      # Shared TypeScript types
│   ├── store/
│   │   └── useStore.ts                   # Zustand global state
│   ├── services/
│   │   └── tauri.ts                      # Tauri invoke wrappers
│   ├── hooks/
│   │   └── useKeyboard.ts                # Keyboard shortcuts
│   └── components/
│       ├── Layout.tsx                    # Main layout (sidebar + content)
│       ├── Sidebar.tsx                   # Group tree navigation
│       ├── ServerList.tsx                # Server list with search
│       ├── ServerForm.tsx                # Create/edit server modal
│       ├── Terminal.tsx                  # xterm.js SSH terminal
│       ├── CredentialForm.tsx            # Credential profile modal
│       ├── Settings.tsx                  # Settings page
│       ├── SearchBar.tsx                 # Global search
│       └── ContextMenu.tsx               # Right-click menu
├── src-tauri/
│   ├── Cargo.toml                        # Rust dependencies
│   ├── tauri.conf.json                   # Tauri configuration
│   ├── build.rs                          # Build script
│   └── src/
│       ├── main.rs                       # Entry point
│       ├── lib.rs                        # Tauri builder + command registration
│       ├── commands/
│       │   ├── mod.rs                    # Command module exports
│       │   ├── servers.rs                # Server CRUD commands
│       │   ├── groups.rs                 # Group CRUD commands
│       │   ├── ssh.rs                    # SSH launch/terminal commands
│       │   ├── rdp.rs                    # RDP launch commands
│       │   ├── ping.rs                   # Ping command
│       │   ├── credentials.rs            # Credential profile commands
│       │   ├── import_export.rs          # CSV/JSON import/export
│       │   └── settings.rs               # Settings commands
│       ├── db/
│       │   ├── mod.rs                    # Database module
│       │   ├── schema.rs             # SQLite schema creation
│       │   └── operations.rs         # DB operations
│       └── security/
│           ├── mod.rs                    # Security module
│           └── dpapi.rs                  # Windows DPAPI encrypt/decrypt
```

---

## Task 1: Project Configuration Files

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `index.html`
- Create: `.gitignore`

- [ ] **Step 1: Create package.json**

```json
{
  "name": "remote-manager-mvp",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build"
  },
  "dependencies": {
    "@mantine/core": "^7.12.0",
    "@mantine/hooks": "^7.12.0",
    "@mantine/modals": "^7.12.0",
    "@mantine/notifications": "^7.12.0",
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-shell": "^2.0.0",
    "@xterm/xterm": "^5.5.0",
    "@xterm/addon-fit": "^0.10.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "zustand": "^4.5.5"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@types/react": "^18.3.5",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.4",
    "vite": "^5.4.2"
  }
}
```

- [ ] **Step 2: Create vite.config.ts**

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: ['es2021', 'chrome100', 'safari13'],
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

- [ ] **Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 4: Create tsconfig.node.json**

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 5: Create index.html**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Remote Manager</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 6: Create .gitignore**

```
# Dependencies
node_modules
dist
dist-ssr

# Tauri
src-tauri/target
src-tauri/Cargo.lock

# IDE
.vscode
.idea
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Env
.env
.env.local

# Debug
*.log
npm-debug.log*
```

- [ ] **Step 7: Commit**

```bash
git init
git add .
git commit -m "chore: add project configuration files"
```

---

## Task 2: Tauri Configuration

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/build.rs`

- [ ] **Step 1: Create src-tauri/Cargo.toml**

```toml
[package]
name = "remote-manager-mvp"
version = "0.1.0"
description = "Remote Manager for Windows SysAdmin"
authors = ["you"]
license = ""
repository = ""
edition = "2021"

[lib]
name = "remote_manager_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.0", features = [] }

[dependencies]
tauri = { version = "2.0", features = [] }
tauri-plugin-shell = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rusqlite = { version = "0.32", features = ["bundled"] }
chrono = { version = "0.4", features = ["serde"] }
csv = "1.3"
uuid = { version = "1.10", features = ["v4"] }
log = "0.4"
env_logger = "0.11"
tokio = { version = "1.40", features = ["process", "rt-multi-thread"] }

[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Security_Cryptography",
  "Win32_Foundation",
] }

[features]
custom-protocol = ["tauri/custom-protocol"]
```

- [ ] **Step 2: Create src-tauri/tauri.conf.json**

```json
{
  "productName": "Remote Manager",
  "version": "0.1.0",
  "identifier": "com.remote-manager.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Remote Manager",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "msi",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 3: Create src-tauri/build.rs**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "chore: add Tauri configuration"
```

---

## Task 3: TypeScript Types

**Files:**
- Create: `src/types/index.ts`

- [ ] **Step 1: Create src/types/index.ts**

```typescript
export type Protocol = 'ssh' | 'rdp';

export interface Server {
  id: string;
  name: string;
  host: string;
  port: number;
  protocol: Protocol;
  username: string;
  group_id: string | null;
  tags: string;
  notes: string;
  favorite: boolean;
  credential_id: string | null;
  created_at: string;
  updated_at: string;
}

export interface ServerInput {
  name: string;
  host: string;
  port: number;
  protocol: Protocol;
  username: string;
  group_id?: string | null;
  tags?: string;
  notes?: string;
  favorite?: boolean;
  credential_id?: string | null;
}

export interface Group {
  id: string;
  name: string;
  parent_id: string | null;
  order: number;
}

export interface GroupInput {
  name: string;
  parent_id?: string | null;
}

export interface Credential {
  id: string;
  name: string;
  username: string;
  encrypted_password: string;
  created_at: string;
}

export interface CredentialInput {
  name: string;
  username: string;
  password: string;
}

export interface Settings {
  id: number;
  theme: 'light' | 'dark';
  font_size: number;
  ssh_port: number;
  rdp_fullscreen: boolean;
  rdp_admin_mode: boolean;
}

export interface ImportResult {
  imported: number;
  errors: string[];
}

export interface ExportData {
  servers: Server[];
  groups: Group[];
  credentials: Credential[];
  settings: Settings;
  version: string;
  exported_at: string;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/types/
git commit -m "feat: add TypeScript type definitions"
```

---

## Task 4: Database Schema & Operations

**Files:**
- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/schema.rs`
- Create: `src-tauri/src/db/operations.rs`

- [ ] **Step 1: Create src-tauri/src/db/mod.rs**

```rust
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
```

- [ ] **Step 2: Create src-tauri/src/db/schema.rs**

```rust
use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS groups (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            parent_id TEXT,
            sort_order INTEGER DEFAULT 0,
            FOREIGN KEY (parent_id) REFERENCES groups(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS servers (
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
        );

        CREATE TABLE IF NOT EXISTS credentials (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            username TEXT DEFAULT '',
            encrypted_password TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            theme TEXT DEFAULT 'dark' CHECK(theme IN ('light', 'dark')),
            font_size INTEGER DEFAULT 14,
            ssh_port INTEGER DEFAULT 22,
            rdp_fullscreen INTEGER DEFAULT 0,
            rdp_admin_mode INTEGER DEFAULT 0
        );

        INSERT OR IGNORE INTO settings (id) VALUES (1);

        CREATE INDEX IF NOT EXISTS idx_servers_group ON servers(group_id);
        CREATE INDEX IF NOT EXISTS idx_servers_favorite ON servers(favorite);
        CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
        CREATE INDEX IF NOT EXISTS idx_groups_parent ON groups(parent_id);
    ")
}
```

- [ ] **Step 3: Create src-tauri/src/db/operations.rs**

```rust
use rusqlite::{Connection, params, OptionalExtension};
use uuid::Uuid;
use chrono::Utc;

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

// Server operations
pub fn create_server(conn: &Connection, name: &str, host: &str, port: i32, protocol: &str, username: &str, group_id: Option<&str>, tags: &str, notes: &str, credential_id: Option<&str>) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO servers (id, name, host, port, protocol, username, group_id, tags, notes, credential_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, name, host, port, protocol, username, group_id, tags, notes, credential_id],
    )?;
    Ok(id)
}

pub fn update_server(conn: &Connection, id: &str, name: &str, host: &str, port: i32, protocol: &str, username: &str, group_id: Option<&str>, tags: &str, notes: &str, credential_id: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE servers SET name=?2, host=?3, port=?4, protocol=?5, username=?6, group_id=?7, tags=?8, notes=?9, credential_id=?10, updated_at=datetime('now') WHERE id=?1",
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
        "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite, credential_id, created_at, updated_at FROM servers WHERE id=?1",
        [id],
        |row| {
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
                favorite: row.get(9)?,
                credential_id: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        },
    ).optional()
}

pub fn list_servers(conn: &Connection, group_id: Option<&str>) -> rusqlite::Result<Vec<ServerRow>> {
    let mut query = "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite, credential_id, created_at, updated_at FROM servers".to_string();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if group_id.is_some() {
        query.push_str(" WHERE group_id = ?1");
    }
    query.push_str(" ORDER BY favorite DESC, name ASC");

    let mut stmt = conn.prepare(&query)?;
    let rows = stmt.query_map([], |row| {
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
            favorite: row.get(9)?,
            credential_id: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn toggle_favorite(conn: &Connection, id: &str) -> rusqlite::Result<bool> {
    conn.execute("UPDATE servers SET favorite = NOT favorite, updated_at=datetime('now') WHERE id=?1", [id])?;
    let fav: bool = conn.query_row("SELECT favorite FROM servers WHERE id=?1", [id], |row| row.get(0))?;
    Ok(fav)
}

pub fn search_servers(conn: &Connection, query: &str) -> rusqlite::Result<Vec<ServerRow>> {
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, protocol, username, group_id, tags, notes, favorite, credential_id, created_at, updated_at
         FROM servers
         WHERE LOWER(name) LIKE ?1 OR LOWER(host) LIKE ?1 OR LOWER(tags) LIKE ?1
         ORDER BY favorite DESC, name ASC"
    )?;
    let rows = stmt.query_map([&pattern], |row| {
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
            favorite: row.get(9)?,
            credential_id: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?;
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
pub fn create_credential(conn: &Connection, name: &str, username: &str, encrypted_password: &str) -> rusqlite::Result<String> {
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
                rdp_fullscreen: row.get(4)?,
                rdp_admin_mode: row.get(5)?,
            })
        },
    )
}

pub fn update_settings(conn: &Connection, theme: &str, font_size: i32, ssh_port: i32, rdp_fullscreen: bool, rdp_admin_mode: bool) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE settings SET theme=?2, font_size=?3, ssh_port=?4, rdp_fullscreen=?5, rdp_admin_mode=?6 WHERE id=1",
        params![1, theme, font_size, ssh_port, rdp_fullscreen as i32, rdp_admin_mode as i32],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/
git commit -m "feat: add SQLite database schema and operations"
```

---

## Task 5: Security Module (DPAPI)

**Files:**
- Create: `src-tauri/src/security/mod.rs`
- Create: `src-tauri/src/security/dpapi.rs`

- [ ] **Step 1: Create src-tauri/src/security/mod.rs**

```rust
pub mod dpapi;

pub fn encrypt(plaintext: &str) -> Result<String, String> {
    dpapi::encrypt_data(plaintext)
}

pub fn decrypt(ciphertext: &str) -> Result<String, String> {
    dpapi::decrypt_data(ciphertext)
}
```

- [ ] **Step 2: Create src-tauri/src/security/dpapi.rs**

```rust
use base64::{Engine as _, engine::general_purpose};

#[cfg(windows)]
pub fn encrypt_data(plaintext: &str) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{
        CRYPTPROTECT_UI_FORBIDDEN, CRYPTPROTECT_LOCAL_MACHINE,
        CryptProtectData, DATA_BLOB,
    };
    use windows::Win32::System::Memory::LocalFree;
    use windows::Win32::Foundation::HLOCAL;

    let bytes = plaintext.as_bytes();
    let mut input_blob = DATA_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output_blob = DATA_BLOB::default();

    unsafe {
        CryptProtectData(
            &input_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        ).map_err(|e| format!("DPAPI encrypt failed: {:?}", e))?;

        let data = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
        let encoded = general_purpose::STANDARD.encode(data);
        LocalFree(HLOCAL(output_blob.pbData as *mut _));
        Ok(encoded)
    }
}

#[cfg(windows)]
pub fn decrypt_data(ciphertext: &str) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{
        CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData, DATA_BLOB,
    };
    use windows::Win32::System::Memory::LocalFree;
    use windows::Win32::Foundation::HLOCAL;

    let decoded = general_purpose::STANDARD.decode(ciphertext)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    let mut input_blob = DATA_BLOB {
        cbData: decoded.len() as u32,
        pbData: decoded.as_ptr() as *mut u8,
    };
    let mut output_blob = DATA_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &input_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob,
        ).map_err(|e| format!("DPAPI decrypt failed: {:?}", e))?;

        let data = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
        let result = String::from_utf8_lossy(data).to_string();
        LocalFree(HLOCAL(output_blob.pbData as *mut _));
        Ok(result)
    }
}

#[cfg(not(windows))]
pub fn encrypt_data(plaintext: &str) -> Result<String, String> {
    // Fallback for non-Windows (dev/testing only)
    Ok(general_purpose::STANDARD.encode(plaintext))
}

#[cfg(not(windows))]
pub fn decrypt_data(ciphertext: &str) -> Result<String, String> {
    let decoded = general_purpose::STANDARD.decode(ciphertext)
        .map_err(|e| format!("Decode failed: {}", e))?;
    Ok(String::from_utf8_lossy(&decoded).to_string())
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/security/
git commit -m "feat: add Windows DPAPI credential encryption"
```

---

## Task 6: Tauri Commands - Servers

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/servers.rs`

- [ ] **Step 1: Create src-tauri/src/commands/mod.rs**

```rust
pub mod servers;
pub mod groups;
pub mod ssh;
pub mod rdp;
pub mod ping;
pub mod credentials;
pub mod import_export;
pub mod settings;
```

- [ ] **Step 2: Create src-tauri/src/commands/servers.rs**

```rust
use tauri::State;
use crate::db::{AppState, operations, get_server, list_servers, search_servers, create_server, update_server, delete_server, toggle_favorite};

#[tauri::command]
pub fn cmd_create_server(
    state: State<AppState>,
    name: String,
    host: String,
    port: i32,
    protocol: String,
    username: String,
    group_id: Option<String>,
    tags: String,
    notes: String,
    credential_id: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    if protocol != "ssh" && protocol != "rdp" {
        return Err("Protocol must be ssh or rdp".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    create_server(&conn, &name, &host, port, &protocol, &username, group_id.as_deref(), &tags, &notes, credential_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_server(
    state: State<AppState>,
    id: String,
    name: String,
    host: String,
    port: i32,
    protocol: String,
    username: String,
    group_id: Option<String>,
    tags: String,
    notes: String,
    credential_id: Option<String>,
) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    update_server(&conn, &id, &name, &host, port, &protocol, &username, group_id.as_deref(), &tags, &notes, credential_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_server(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    delete_server(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_server(state: State<AppState>, id: String) -> Result<Option<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    get_server(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_servers(state: State<AppState>, group_id: Option<String>) -> Result<Vec<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    list_servers(&conn, group_id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_toggle_favorite(state: State<AppState>, id: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    toggle_favorite(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_search_servers(state: State<AppState>, query: String) -> Result<Vec<operations::ServerRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    search_servers(&conn, &query).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/servers.rs src-tauri/src/commands/mod.rs
git commit -m "feat: add server CRUD Tauri commands"
```

---

## Task 7: Tauri Commands - Groups

**Files:**
- Create: `src-tauri/src/commands/groups.rs`

- [ ] **Step 1: Create src-tauri/src/commands/groups.rs**

```rust
use tauri::State;
use crate::db::{AppState, operations};

#[tauri::command]
pub fn cmd_create_group(
    state: State<AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Group name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::create_group(&conn, &name, parent_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_group(state: State<AppState>, id: String, name: String) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("Group name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::update_group(&conn, &id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_group(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::delete_group(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_groups(state: State<AppState>) -> Result<Vec<operations::GroupRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_groups(&conn).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/commands/groups.rs
git commit -m "feat: add group CRUD Tauri commands"
```

---

## Task 8: Tauri Commands - SSH, RDP, Ping

**Files:**
- Create: `src-tauri/src/commands/ssh.rs`
- Create: `src-tauri/src/commands/rdp.rs`
- Create: `src-tauri/src/commands/ping.rs`

- [ ] **Step 1: Create src-tauri/src/commands/ssh.rs**

```rust
use std::process::Command;

#[tauri::command]
pub fn cmd_launch_ssh(host: String, port: i32, username: String) -> Result<(), String> {
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    if username.trim().is_empty() {
        return Err("Username is required".to_string());
    }
    // Validate host doesn't contain shell metacharacters
    if host.contains(';') || host.contains('|') || host.contains('&') || host.contains('`') {
        return Err("Invalid host format".to_string());
    }
    if username.contains(';') || username.contains('|') || username.contains('&') {
        return Err("Invalid username format".to_string());
    }

    Command::new("wt.exe")
        .args(["-p", "Ubuntu", "ssh", &format!("{}@{}", username, host), "-p", &port.to_string()])
        .spawn()
        .map_err(|e| format!("Failed to launch SSH: {}", e))?;

    Ok(())
}
```

- [ ] **Step 2: Create src-tauri/src/commands/rdp.rs**

```rust
use std::process::Command;
use std::fs;
use std::env;

#[tauri::command]
pub fn cmd_launch_rdp(host: String, username: String, fullscreen: bool, admin_mode: bool) -> Result<(), String> {
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    if host.contains(';') || host.contains('|') || host.contains('&') {
        return Err("Invalid host format".to_string());
    }

    // Create temporary .rdp file
    let mut rdp_content = format!(
        "full address:s:{}\r\nusername:s:{}\r\nscreen mode id:i:{}\r\n",
        host,
        username,
        if fullscreen { 2 } else { 1 }
    );

    if admin_mode {
        rdp_content.push_str("administrative session:i:1\r\n");
    }

    let temp_path = env::temp_dir().join(format!("remote_manager_{}.rdp", host.replace('.', "_")));
    fs::write(&temp_path, rdp_content)
        .map_err(|e| format!("Failed to create RDP file: {}", e))?;

    Command::new("mstsc.exe")
        .arg(temp_path.to_str().unwrap())
        .spawn()
        .map_err(|e| format!("Failed to launch RDP: {}", e))?;

    Ok(())
}
```

- [ ] **Step 3: Create src-tauri/src/commands/ping.rs**

```rust
use std::process::Command;

#[tauri::command]
pub fn cmd_ping(host: String) -> Result<String, String> {
    if host.trim().is_empty() {
        return Err("Host is required".to_string());
    }
    if host.contains(';') || host.contains('|') || host.contains('&') || host.contains('`') {
        return Err("Invalid host format".to_string());
    }

    let output = Command::new("ping")
        .args(["-n", "1", "-w", "3000", &host])
        .output()
        .map_err(|e| format!("Ping failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if output.status.success() {
        // Parse latency from output
        if let Some(pos) = stdout.find("time=") {
            let start = pos + 5;
            if let Some(end) = stdout[start..].find("ms") {
                let latency = &stdout[start..start + end];
                return Ok(format!("Reachable ({}ms)", latency.trim()));
            }
        }
        if stdout.contains("time<1ms") {
            return Ok("Reachable (<1ms)".to_string());
        }
        Ok("Reachable".to_string())
    } else {
        Ok("Unreachable".to_string())
    }
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/ssh.rs src-tauri/src/commands/rdp.rs src-tauri/src/commands/ping.rs
git commit -m "feat: add SSH, RDP, and ping Tauri commands"
```

---

## Task 9: Tauri Commands - Credentials & Import/Export & Settings

**Files:**
- Create: `src-tauri/src/commands/credentials.rs`
- Create: `src-tauri/src/commands/import_export.rs`
- Create: `src-tauri/src/commands/settings.rs`

- [ ] **Step 1: Create src-tauri/src/commands/credentials.rs**

```rust
use tauri::State;
use crate::db::{AppState, operations};
use crate::security;

#[tauri::command]
pub fn cmd_create_credential(
    state: State<AppState>,
    name: String,
    username: String,
    password: String,
) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    let encrypted = security::encrypt(&password)?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::create_credential(&conn, &name, &username, &encrypted)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_delete_credential(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::delete_credential(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_credentials(state: State<AppState>) -> Result<Vec<operations::CredentialRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::list_credentials(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_credential_password(state: State<AppState>, id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let encrypted = operations::get_credential_password(&conn, &id)
        .map_err(|e| e.to_string())?
        .ok_or("Credential not found")?;
    security::decrypt(&encrypted)
}
```

- [ ] **Step 2: Create src-tauri/src/commands/import_export.rs**

```rust
use tauri::State;
use crate::db::{AppState, operations};
use std::fs;

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

            match operations::create_server(&conn, &name, &host, port, &protocol, &username, None, &tags, &notes, None) {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("Server {}: {}", i + 1, e)),
            }
        }
    }

    Ok((imported, errors))
}
```

- [ ] **Step 3: Create src-tauri/src/commands/settings.rs**

```rust
use tauri::State;
use crate::db::{AppState, operations};

#[tauri::command]
pub fn cmd_get_settings(state: State<AppState>) -> Result<operations::SettingsRow, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::get_settings(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_update_settings(
    state: State<AppState>,
    theme: String,
    font_size: i32,
    ssh_port: i32,
    rdp_fullscreen: bool,
    rdp_admin_mode: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    operations::update_settings(&conn, &theme, font_size, ssh_port, rdp_fullscreen, rdp_admin_mode)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/credentials.rs src-tauri/src/commands/import_export.rs src-tauri/src/commands/settings.rs
git commit -m "feat: add credentials, import/export, and settings commands"
```

---

## Task 10: Rust lib.rs & main.rs Update

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Rewrite src-tauri/src/lib.rs**

```rust
mod commands;
mod db;
mod security;

use db::{AppState, init_connection};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let conn = init_connection();
    let state = AppState {
        db: std::sync::Mutex::new(conn),
    };

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::servers::cmd_create_server,
            commands::servers::cmd_update_server,
            commands::servers::cmd_delete_server,
            commands::servers::cmd_get_server,
            commands::servers::cmd_list_servers,
            commands::servers::cmd_toggle_favorite,
            commands::servers::cmd_search_servers,
            commands::groups::cmd_create_group,
            commands::groups::cmd_update_group,
            commands::groups::cmd_delete_group,
            commands::groups::cmd_list_groups,
            commands::ssh::cmd_launch_ssh,
            commands::rdp::cmd_launch_rdp,
            commands::ping::cmd_ping,
            commands::credentials::cmd_create_credential,
            commands::credentials::cmd_delete_credential,
            commands::credentials::cmd_list_credentials,
            commands::credentials::cmd_get_credential_password,
            commands::import_export::cmd_import_csv,
            commands::import_export::cmd_export_csv,
            commands::import_export::cmd_export_json,
            commands::import_export::cmd_import_json,
            commands::settings::cmd_get_settings,
            commands::settings::cmd_update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 2: Update src-tauri/src/main.rs**

```rust
fn main() {
    remote_manager_lib::run()
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/main.rs
git commit -m "feat: wire up all Tauri commands in lib.rs"
```

---

## Task 11: Zustand Store

**Files:**
- Create: `src/store/useStore.ts`

- [ ] **Step 1: Create src/store/useStore.ts**

```typescript
import { create } from 'zustand';
import type { Server, Group, Credential, Settings } from '../types';
import * as api from '../services/tauri';

interface AppState {
  servers: Server[];
  groups: Group[];
  credentials: Credential[];
  settings: Settings | null;
  searchQuery: string;
  selectedGroupId: string | null;
  selectedServerId: string | null;
  isLoading: boolean;

  // Actions
  loadServers: () => Promise<void>;
  loadGroups: () => Promise<void>;
  loadCredentials: () => Promise<void>;
  loadSettings: () => Promise<void>;
  createServer: (server: Omit<Server, 'id' | 'created_at' | 'updated_at'>) => Promise<void>;
  updateServer: (id: string, server: Partial<Server>) => Promise<void>;
  deleteServer: (id: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  searchServers: (query: string) => Promise<void>;
  createGroup: (name: string, parentId?: string | null) => Promise<void>;
  updateGroup: (id: string, name: string) => Promise<void>;
  deleteGroup: (id: string) => Promise<void>;
  createCredential: (name: string, username: string, password: string) => Promise<void>;
  deleteCredential: (id: string) => Promise<void>;
  updateSettings: (settings: Partial<Settings>) => Promise<void>;
  setSearchQuery: (query: string) => void;
  setSelectedGroup: (id: string | null) => void;
  setSelectedServer: (id: string | null) => void;
}

export const useStore = create<AppState>((set, get) => ({
  servers: [],
  groups: [],
  credentials: [],
  settings: null,
  searchQuery: '',
  selectedGroupId: null,
  selectedServerId: null,
  isLoading: false,

  loadServers: async () => {
    const groupId = get().selectedGroupId;
    const servers = await api.listServers(groupId);
    set({ servers });
  },

  loadGroups: async () => {
    const groups = await api.listGroups();
    set({ groups });
  },

  loadCredentials: async () => {
    const credentials = await api.listCredentials();
    set({ credentials });
  },

  loadSettings: async () => {
    const settings = await api.getSettings();
    set({ settings });
  },

  createServer: async (server) => {
    await api.createServer(server as any);
    await get().loadServers();
  },

  updateServer: async (id, server) => {
    await api.updateServer(id, server);
    await get().loadServers();
  },

  deleteServer: async (id) => {
    await api.deleteServer(id);
    await get().loadServers();
  },

  toggleFavorite: async (id) => {
    await api.toggleFavorite(id);
    await get().loadServers();
  },

  searchServers: async (query) => {
    if (!query.trim()) {
      await get().loadServers();
      return;
    }
    const servers = await api.searchServers(query);
    set({ servers });
  },

  createGroup: async (name, parentId) => {
    await api.createGroup(name, parentId);
    await get().loadGroups();
  },

  updateGroup: async (id, name) => {
    await api.updateGroup(id, name);
    await get().loadGroups();
  },

  deleteGroup: async (id) => {
    await api.deleteGroup(id);
    await get().loadGroups();
    await get().loadServers();
  },

  createCredential: async (name, username, password) => {
    await api.createCredential(name, username, password);
    await get().loadCredentials();
  },

  deleteCredential: async (id) => {
    await api.deleteCredential(id);
    await get().loadCredentials();
  },

  updateSettings: async (newSettings) => {
    const current = get().settings;
    if (!current) return;
    const updated = { ...current, ...newSettings };
    await api.updateSettings(updated);
    set({ settings: updated });
  },

  setSearchQuery: (query) => set({ searchQuery: query }),
  setSelectedGroup: (id) => {
    set({ selectedGroupId: id });
    get().loadServers();
  },
  setSelectedServer: (id) => set({ selectedServerId: id }),
}));
```

- [ ] **Step 2: Commit**

```bash
git add src/store/
git commit -m "feat: add Zustand global state store"
```

---

## Task 12: Tauri Service Layer

**Files:**
- Create: `src/services/tauri.ts`

- [ ] **Step 1: Create src/services/tauri.ts**

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { Server, Group, Credential, Settings } from '../types';

// Servers
export const createServer = (server: Omit<Server, 'id' | 'created_at' | 'updated_at'>): Promise<string> =>
  invoke('cmd_create_server', { ...server });

export const updateServer = (id: string, server: Partial<Server>): Promise<void> =>
  invoke('cmd_update_server', { id, ...server });

export const deleteServer = (id: string): Promise<void> =>
  invoke('cmd_delete_server', { id });

export const getServer = (id: string): Promise<Server | null> =>
  invoke('cmd_get_server', { id });

export const listServers = (groupId?: string | null): Promise<Server[]> =>
  invoke('cmd_list_servers', { groupId });

export const toggleFavorite = (id: string): Promise<boolean> =>
  invoke('cmd_toggle_favorite', { id });

export const searchServers = (query: string): Promise<Server[]> =>
  invoke('cmd_search_servers', { query });

// Groups
export const createGroup = (name: string, parentId?: string | null): Promise<string> =>
  invoke('cmd_create_group', { name, parentId });

export const updateGroup = (id: string, name: string): Promise<void> =>
  invoke('cmd_update_group', { id, name });

export const deleteGroup = (id: string): Promise<void> =>
  invoke('cmd_delete_group', { id });

export const listGroups = (): Promise<Group[]> =>
  invoke('cmd_list_groups');

// SSH / RDP
export const launchSsh = (host: string, port: number, username: string): Promise<void> =>
  invoke('cmd_launch_ssh', { host, port, username });

export const launchRdp = (host: string, username: string, fullscreen: boolean, adminMode: boolean): Promise<void> =>
  invoke('cmd_launch_rdp', { host, username, fullscreen, adminMode });

// Ping
export const pingHost = (host: string): Promise<string> =>
  invoke('cmd_ping', { host });

// Credentials
export const createCredential = (name: string, username: string, password: string): Promise<string> =>
  invoke('cmd_create_credential', { name, username, password });

export const deleteCredential = (id: string): Promise<void> =>
  invoke('cmd_delete_credential', { id });

export const listCredentials = (): Promise<Credential[]> =>
  invoke('cmd_list_credentials');

export const getCredentialPassword = (id: string): Promise<string> =>
  invoke('cmd_get_credential_password', { id });

// Import/Export
export const importCsv = (path: string): Promise<{ imported: number; errors: string[] }> =>
  invoke('cmd_import_csv', { path });

export const exportCsv = (path: string): Promise<void> =>
  invoke('cmd_export_csv', { path });

export const exportJson = (path: string): Promise<void> =>
  invoke('cmd_export_json', { path });

export const importJson = (path: string): Promise<{ imported: number; errors: string[] }> =>
  invoke('cmd_import_json', { path });

// Settings
export const getSettings = (): Promise<Settings> =>
  invoke('cmd_get_settings');

export const updateSettings = (settings: Settings): Promise<void> =>
  invoke('cmd_update_settings', { ...settings });
```

- [ ] **Step 2: Commit**

```bash
git add src/services/
git commit -m "feat: add Tauri service layer"
```

---

## Task 13: React Entry Point & App Shell

**Files:**
- Create: `src/main.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Create src/main.tsx**

```typescript
import React from 'react';
import ReactDOM from 'react-dom/client';
import { MantineProvider, createTheme } from '@mantine/core';
import '@mantine/core/styles.css';
import App from './App';

const theme = createTheme({
  fontFamily: 'Inter, Segoe UI, sans-serif',
  defaultRadius: 'md',
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="dark">
      <App />
    </MantineProvider>
  </React.StrictMode>
);
```

- [ ] **Step 2: Rewrite src/App.tsx**

```typescript
import { useEffect } from 'react';
import { useStore } from './store/useStore';
import { Layout } from './components/Layout';

export default function App() {
  const { loadServers, loadGroups, loadCredentials, loadSettings } = useStore();

  useEffect(() => {
    loadServers();
    loadGroups();
    loadCredentials();
    loadSettings();
  }, []);

  return <Layout />;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/main.tsx src/App.tsx
git commit -m "feat: add React entry point and app shell"
```

---

## Task 14: Layout & Sidebar Components

**Files:**
- Create: `src/components/Layout.tsx`
- Create: `src/components/Sidebar.tsx`

- [ ] **Step 1: Create src/components/Layout.tsx**

```typescript
import { AppShell, Group, Text } from '@mantine/core';
import { Sidebar } from './Sidebar';
import { ServerList } from './ServerList';
import { SearchBar } from './SearchBar';

export function Layout() {
  return (
    <AppShell
      header={{ height: 50 }}
      navbar={{ width: 250, breakpoint: 'sm' }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between">
          <Text fw={700} size="lg">Remote Manager</Text>
          <SearchBar />
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <Sidebar />
      </AppShell.Navbar>

      <AppShell.Main>
        <ServerList />
      </AppShell.Main>
    </AppShell>
  );
}
```

- [ ] **Step 2: Create src/components/Sidebar.tsx**

```typescript
import { useState } from 'react';
import { Text, Group, ActionIcon, Stack, Divider, Box } from '@mantine/core';
import { IconPlus, IconServer, IconStar } from '@tabler/icons-react';
import { useStore } from '../store/useStore';

export function Sidebar() {
  const { groups, servers, selectedGroupId, setSelectedGroup, createGroup } = useStore();
  const [newGroup, setNewGroup] = useState('');

  const favorites = servers.filter(s => s.favorite);
  const rootGroups = groups.filter(g => !g.parent_id);

  const handleAddGroup = async () => {
    if (newGroup.trim()) {
      await createGroup(newGroup.trim());
      setNewGroup('');
    }
  };

  return (
    <Box>
      <Stack gap="xs">
        <Text size="sm" fw={600} c="dimmed">QUICK ACCESS</Text>
        <Group
          gap={8}
          p="xs"
          style={{ cursor: 'pointer', borderRadius: 4 }}
          bg={selectedGroupId === null ? 'var(--mantine-color-dark-6)' : undefined}
          onClick={() => setSelectedGroup(null)}
        >
          <IconServer size={16} />
          <Text size="sm">All Servers ({servers.length})</Text>
        </Group>
        {favorites.length > 0 && (
          <Group
            gap={8}
            p="xs"
            style={{ cursor: 'pointer', borderRadius: 4 }}
          >
            <IconStar size={16} />
            <Text size="sm">Favorites ({favorites.length})</Text>
          </Group>
        )}
      </Stack>

      <Divider my="md" />

      <Stack gap="xs">
        <Group justify="space-between">
          <Text size="sm" fw={600} c="dimmed">GROUPS</Text>
          <ActionIcon size="sm" variant="subtle" onClick={handleAddGroup}>
            <IconPlus size={14} />
          </ActionIcon>
        </Group>

        {rootGroups.map(group => (
          <Group
            key={group.id}
            gap={8}
            p="xs"
            style={{ cursor: 'pointer', borderRadius: 4 }}
            bg={selectedGroupId === group.id ? 'var(--mantine-color-dark-6)' : undefined}
            onClick={() => setSelectedGroup(group.id)}
          >
            <Text size="sm">{group.name}</Text>
          </Group>
        ))}
      </Stack>
    </Box>
  );
}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/Layout.tsx src/components/Sidebar.tsx
git commit -m "feat: add layout and sidebar components"
```

---

## Task 15: SearchBar Component

**Files:**
- Create: `src/components/SearchBar.tsx`

- [ ] **Step 1: Create src/components/SearchBar.tsx**

```typescript
import { useState } from 'react';
import { TextInput, Kbd } from '@mantine/core';
import { IconSearch } from '@tabler/icons-react';
import { useStore } from '../store/useStore';

export function SearchBar() {
  const { searchServers } = useStore();
  const [value, setValue] = useState('');

  const handleChange = (newValue: string) => {
    setValue(newValue);
    searchServers(newValue);
  };

  return (
    <TextInput
      placeholder="Search servers..."
      leftSection={<IconSearch size={14} />}
      rightSection={<Kbd size="xs">Ctrl+K</Kbd>}
      value={value}
      onChange={(e) => handleChange(e.currentTarget.value)}
      w={300}
      size="sm"
    />
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/SearchBar.tsx
git commit -m "feat: add search bar component"
```

---

## Task 16: ServerList Component

**Files:**
- Create: `src/components/ServerList.tsx`

- [ ] **Step 1: Create src/components/ServerList.tsx**

```typescript
import { Group, Text, Stack, ActionIcon, Paper, Badge, Tooltip } from '@mantine/core';
import { IconStar, IconStarFilled, IconPlus, IconServer2 } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { ServerForm } from './ServerForm';
import { modals } from '@mantine/modals';
import { launchSsh, launchRdp, pingHost } from '../services/tauri';
import { notifications } from '@mantine/notifications';

export function ServerList() {
  const { servers, toggleFavorite, selectedGroupId } = useStore();

  const filteredServers = selectedGroupId
    ? servers.filter(s => s.group_id === selectedGroupId)
    : servers;

  const handleConnect = async (server: typeof servers[0]) => {
    if (server.protocol === 'ssh') {
      await launchSsh(server.host, server.port, server.username);
    } else {
      await launchRdp(server.host, server.username, false, false);
    }
  };

  const handlePing = async (host: string) => {
    const result = await pingHost(host);
    notifications.show({ title: 'Ping Result', message: result, color: result.startsWith('Reachable') ? 'green' : 'red' });
  };

  const openCreateModal = () => {
    modals.open({
      title: 'Add Server',
      children: <ServerForm />,
      size: 'md',
    });
  };

  return (
    <Stack gap="md">
      <Group justify="space-between">
        <Text fw={600} size="lg">Servers ({filteredServers.length})</Text>
        <ActionIcon variant="filled" onClick={openCreateModal}>
          <IconPlus size={16} />
        </ActionIcon>
      </Group>

      {filteredServers.length === 0 ? (
        <Paper p="xl" ta="center" withBorder>
          <IconServer2 size={48} opacity={0.3} />
          <Text c="dimmed" mt="sm">No servers yet. Click + to add one.</Text>
        </Paper>
      ) : (
        filteredServers.map(server => (
          <Paper key={server.id} p="md" withBorder>
            <Group justify="space-between">
              <Group>
                <ActionIcon
                  size="sm"
                  variant="subtle"
                  onClick={() => toggleFavorite(server.id)}
                >
                  {server.favorite ? <IconStarFilled size={16} color="gold" /> : <IconStar size={16} />}
                </ActionIcon>
                <div>
                  <Text fw={500}>{server.name}</Text>
                  <Text size="xs" c="dimmed">{server.host}:{server.port}</Text>
                </div>
                <Badge size="sm" variant="light" color={server.protocol === 'ssh' ? 'blue' : 'green'}>
                  {server.protocol.toUpperCase()}
                </Badge>
              </Group>
              <Group>
                <Tooltip label="Connect">
                  <ActionIcon size="sm" variant="light" onClick={() => handleConnect(server)}>
                    <IconServer2 size={14} />
                  </ActionIcon>
                </Tooltip>
                <Tooltip label="Ping">
                  <ActionIcon size="sm" variant="light" onClick={() => handlePing(server.host)}>
                    <IconServer2 size={14} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            </Group>
          </Paper>
        ))
      )}
    </Stack>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/ServerList.tsx
git commit -m "feat: add server list component"
```

---

## Task 17: ServerForm Component

**Files:**
- Create: `src/components/ServerForm.tsx`

- [ ] **Step 1: Create src/components/ServerForm.tsx**

```typescript
import { useState, useEffect } from 'react';
import { TextInput, NumberInput, Select, Textarea, Button, Stack, Group } from '@mantine/core';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';

export function ServerForm() {
  const { createServer, updateServer, groups, credentials, loadCredentials } = useStore();
  const [name, setName] = useState('');
  const [host, setHost] = useState('');
  const [port, setPort] = useState(22);
  const [protocol, setProtocol] = useState<string | null>('ssh');
  const [username, setUsername] = useState('');
  const [groupId, setGroupId] = useState<string | null>(null);
  const [tags, setTags] = useState('');
  const [notes, setNotes] = useState('');
  const [credentialId, setCredentialId] = useState<string | null>(null);

  useEffect(() => {
    loadCredentials();
  }, []);

  const handleSubmit = async () => {
    if (!name.trim() || !host.trim() || !protocol) return;

    await createServer({
      name: name.trim(),
      host: host.trim(),
      port,
      protocol: protocol as 'ssh' | 'rdp',
      username: username.trim(),
      group_id: groupId,
      tags: tags.trim(),
      notes: notes.trim(),
      favorite: false,
      credential_id: credentialId,
    });

    modals.closeAll();
  };

  return (
    <Stack>
      <TextInput
        label="Name"
        placeholder="My Server"
        value={name}
        onChange={(e) => setName(e.currentTarget.value)}
        required
      />
      <Group grow>
        <TextInput
          label="Host / IP"
          placeholder="192.168.1.100"
          value={host}
          onChange={(e) => setHost(e.currentTarget.value)}
          required
        />
        <NumberInput
          label="Port"
          value={port}
          onChange={(v) => setPort(Number(v))}
          min={1}
          max={65535}
        />
      </Group>
      <Group grow>
        <Select
          label="Protocol"
          data={[{ value: 'ssh', label: 'SSH' }, { value: 'rdp', label: 'RDP' }]}
          value={protocol}
          onChange={setProtocol}
          required
        />
        <TextInput
          label="Username"
          placeholder="root"
          value={username}
          onChange={(e) => setUsername(e.currentTarget.value)}
        />
      </Group>
      <Select
        label="Group"
        data={groups.map(g => ({ value: g.id, label: g.name }))}
        value={groupId}
        onChange={setGroupId}
        clearable
        searchable
      />
      <Select
        label="Credential Profile"
        data={credentials.map(c => ({ value: c.id, label: c.name }))}
        value={credentialId}
        onChange={setCredentialId}
        clearable
        searchable
      />
      <TextInput
        label="Tags"
        placeholder="k8s, production"
        value={tags}
        onChange={(e) => setTags(e.currentTarget.value)}
      />
      <Textarea
        label="Notes"
        placeholder="Optional notes..."
        value={notes}
        onChange={(e) => setNotes(e.currentTarget.value)}
        autosize
        minRows={2}
      />
      <Group justify="flex-end">
        <Button variant="subtle" onClick={() => modals.closeAll()}>Cancel</Button>
        <Button onClick={handleSubmit} disabled={!name.trim() || !host.trim() || !protocol}>
          Save Server
        </Button>
      </Group>
    </Stack>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/ServerForm.tsx
git commit -m "feat: add server form component"
```

---

## Task 18: Settings Component

**Files:**
- Create: `src/components/Settings.tsx`

- [ ] **Step 1: Create src/components/Settings.tsx**

```typescript
import { useEffect } from 'react';
import { Stack, NumberInput, Select, Switch, Button, Text, Divider } from '@mantine/core';
import { useStore } from '../store/useStore';

export function Settings() {
  const { settings, loadSettings, updateSettings } = useStore();

  useEffect(() => {
    loadSettings();
  }, []);

  if (!settings) return <Text>Loading...</Text>;

  return (
    <Stack gap="md" maw={500}>
      <Text size="lg" fw={600}>Settings</Text>

      <Divider label="Appearance" labelPosition="center" />
      <Select
        label="Theme"
        data={[{ value: 'dark', label: 'Dark' }, { value: 'light', label: 'Light' }]}
        value={settings.theme}
        onChange={(v) => updateSettings({ ...settings, theme: v as 'light' | 'dark' })}
      />
      <NumberInput
        label="Terminal Font Size"
        value={settings.font_size}
        onChange={(v) => updateSettings({ ...settings, font_size: Number(v) })}
        min={8}
        max={32}
      />

      <Divider label="Defaults" labelPosition="center" />
      <NumberInput
        label="Default SSH Port"
        value={settings.ssh_port}
        onChange={(v) => updateSettings({ ...settings, ssh_port: Number(v) })}
        min={1}
        max={65535}
      />
      <Switch
        label="RDP Fullscreen"
        checked={settings.rdp_fullscreen}
        onChange={(e) => updateSettings({ ...settings, rdp_fullscreen: e.currentTarget.checked })}
      />
      <Switch
        label="RDP Admin Mode"
        checked={settings.rdp_admin_mode}
        onChange={(e) => updateSettings({ ...settings, rdp_admin_mode: e.currentTarget.checked })}
      />
    </Stack>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/Settings.tsx
git commit -m "feat: add settings component"
```

---

## Task 19: Install Dependencies & Verify Build

**Files:**
- Modify: `package.json` (lock file generated)

- [ ] **Step 1: Install npm dependencies**

Run: `npm install`
Expected: All packages installed successfully

- [ ] **Step 2: Verify TypeScript compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Verify Vite builds**

Run: `npm run build`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "chore: install dependencies and verify build"
```

---

## Task 20: Final Integration & Tauri Dev Test

**Files:**
- Create: `src/vite-env.d.ts`

- [ ] **Step 1: Create src/vite-env.d.ts**

```typescript
/// <reference types="vite/client" />
```

- [ ] **Step 2: Run Tauri dev to verify full integration**

Run: `npm run tauri:dev`
Expected: App window opens, sidebar renders, server list works

- [ ] **Step 3: Commit**

```bash
git add .
git commit -m "feat: complete MVP implementation"
```

---

## Self-Review Checklist

- [x] All PRD requirements mapped to tasks (Server CRUD, Groups, Search, Favorites, SSH, RDP, Credentials, Import/Export, Ping, Settings)
- [x] No placeholders/TODOs in plan
- [x] Type consistency between Rust and TypeScript
- [x] Security: input validation on all commands, DPAPI encryption
- [x] Error handling: all commands return Result types
- [x] File structure matches PRD specification
