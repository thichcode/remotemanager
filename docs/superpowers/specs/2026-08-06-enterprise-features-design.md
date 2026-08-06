# Enterprise Features Design Spec

> **Feature:** DPAPI secret encryption, Portable mode, Backup/Restore, Session history, SSH key management, MSI installer, Auto-update via GitHub Releases
> **Date:** 2026-08-06
> **Status:** Approved

## Goal

Add seven enterprise features to the Remote Manager Tauri 2 desktop app while maintaining full backward compatibility with the existing schema and commands.

## Architecture

Modular integration (Approach B). New dedicated Rust modules per feature, official Tauri plugins for updater/dialog/fs. Existing modules (`commands/`, `db/`, `security/`) preserved so all current commands keep working unchanged. All path resolution goes through a new `paths.rs` so portable mode is transparent to every consumer.

## Tech Stack Additions

| Purpose | Crate / Plugin |
|---|---|
| Backup zip | `zip` (Rust) |
| File dialogs | `tauri-plugin-dialog` |
| Auto-update | `tauri-plugin-updater` |
| File access (frontend) | `tauri-plugin-fs` |
| Signing | `tauri signer` (CLI) |

---

## 1. DPAPI Secret Encryption

**Status:** Already implemented in `security/dpapi.rs` (working, `LocalFree` FFI fix applied). Reused unchanged for SSH key passphrase encryption.

- `security::encrypt(plaintext)` / `security::decrypt(ciphertext)` API stays.
- New consumer: SSH key passphrases stored via `encrypted_password` column.

## 2. Portable Mode (file detection)

**New file:** `src-tauri/src/paths.rs`

- If a file named `portable` exists next to the running executable → data root = `<exe_dir>/data`.
- Otherwise → data root = `%APPDATA%/remote-manager` (existing behavior, via `dirs::data_dir()`).
- All consumers use `paths::data_dir()`: DB connection, key storage, backup.
- DB file remains `data/data.db` in both modes → no data migration needed.
- Settings UI shows a read-only badge: `Portable` or `Installed`.

Functions:
```rust
pub fn is_portable() -> bool
pub fn data_dir() -> PathBuf        // <exe_dir>/data OR %APPDATA%/remote-manager
pub fn keys_dir() -> PathBuf        // data_dir()/keys
pub fn db_path() -> PathBuf         // data_dir()/data.db
pub fn backup_dir() -> PathBuf      // data_dir()/backups
```

## 3. Backup/Restore (manual zip)

**New file:** `src-tauri/src/backup.rs`

- `backup::create(path: &str) -> Result<BackupSummary, String>`:
  - Write `manifest.json` (version, timestamp, schema_version) into a temp dir.
  - Zip: `data.db`, `data.db-wal`, `data.db-shm` (if present), `keys/`, `manifest.json`.
  - Zip written to user-chosen path; extension `.rmbackup`.
- `backup::restore(path: &str) -> Result<(), String>`:
  - Validate zip contains `manifest.json` + `data.db`.
  - Move current data dir to `data-backup-pre-restore-<ts>/`.
  - Extract zip into fresh data dir.
  - Re-open DB connection (re-init) after restore.
- Frontend: Backup/Restore buttons in Settings using `tauri-plugin-dialog` to pick files.

Commands:
```rust
#[tauri::command] fn cmd_backup(path: String) -> Result<BackupSummary, String>
#[tauri::command] fn cmd_restore(path: String) -> Result<(), String>
```

**Backward compat:** Restore from an old version's backup (no `keys/`) must still work — treat missing dirs as empty.

## 4. Session History (track + reconnect, 200 cap)

**New table:**
```sql
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
```

**New file:** `src-tauri/src/history.rs`
- `record(...)` called at the start of `cmd_launch_ssh` and `cmd_launch_rdp`.
- After insert, prune: `DELETE FROM session_history WHERE id NOT IN (SELECT id FROM session_history ORDER BY connected_at DESC, id DESC LIMIT 200)`.
- `list()` → most recent 200.
- `clear()` → delete all.

Commands:
```rust
#[tauri::command] fn cmd_list_history() -> Result<Vec<HistoryRow>, String>
#[tauri::command] fn cmd_clear_history() -> Result<(), String>
```

**Frontend:** New "Recent Connections" section in Sidebar (top, below Quick Access):
- Each row: icon + server name + host:port + time ago.
- Click → re-connect using the recorded server + optional attached key.
- Clear button (trash icon).

## 5. SSH Key Management (store + reference)

**New table:**
```sql
CREATE TABLE IF NOT EXISTS ssh_keys (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    private_key   TEXT NOT NULL,      -- stored as file in data/keys/
    public_key    TEXT DEFAULT '',
    passphrase    TEXT DEFAULT '',    -- DPAPI-encrypted, may be empty
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Migration:** add column to `servers`:
```sql
ALTER TABLE servers ADD COLUMN ssh_key_id TEXT;
```
(Guarded: only if column does not exist.)

**New file:** `src-tauri/src/sshkeys.rs`
- `import_private_key(path, name, passphrase) -> id`:
  - Copy key file bytes into `data/keys/<uuid>.key`.
  - Optional passphrase encrypted via `security::encrypt`.
  - Insert row.
- `list()`, `delete(id)` (also removes file).
- Reads: `get_private_key_path(id)` for SSH launch.

Commands:
```rust
#[tauri::command] fn cmd_import_ssh_key(path: String, name: String, passphrase: Option<String>) -> Result<String, String>
#[tauri::command] fn cmd_list_ssh_keys() -> Result<Vec<SshKeyRow>, String>
#[tauri::command] fn cmd_delete_ssh_key(id: String) -> Result<(), String>
#[tauri::command] fn cmd_attach_key(server_id: String, ssh_key_id: Option<String>) -> Result<(), String>
```

**SSH launch change** (`cmd_launch_ssh`): if server has `ssh_key_id`, insert `-i <key_path>` before `host@user`.

**Frontend:**
- `SshKeys.tsx` page: list keys, import via dialog, delete.
- `ServerForm.tsx`: key selector (optional).
- History rows carry `ssh_key_id` for reconnect.

## 6. MSI Installer

- Already `"targets": "msi"` in `tauri.conf.json`. Add metadata:
```json
{
  "publisher": "thichcode",
  "copyright": "Copyright © 2026 thichcode"
}
```
- Add `bundle.windows.wix` upgrade/downgrade defaults if required by WiX.
- Build via `npm run tauri:build` (Tauri downloads/uses WiX v4 toolchain automatically on Windows).

## 7. Auto-update (GitHub Releases)

**Target repo:** `https://github.com/thichcode/remotemanager`

- Add deps: `tauri-plugin-updater = "2"`, `tauri-plugin-dialog = "2"`.
- `tauri.conf.json`:
```json
"plugins": {
  "updater": {
    "pubkey": "<public key>",
    "endpoints": ["https://github.com/thichcode/remotemanager/releases/latest/download/latest.json"],
    "windows": { "installMode": "passive" }
  }
}
```
- Add `"createUpdaterArtifacts": true` to `bundle` so `tauri build` emits update artifacts.
- Signing:
  - Run `npx @tauri-apps/cli signer generate -w <path>` to create keypair.
  - Public key → `tauri.conf.json` (committed).
  - Private key → `src-tauri/.tauri-signing.key` (gitignored) + document `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` env vars for CI.
- Frontend:
  - App startup: `check()` updater, notify if available.
  - Settings: "Check for Updates" button → `downloadAndInstall` with progress, restart prompt.

**Capabilities** (`capabilities/default.json`): add `updater:default`, `dialog:default`, `fs:allow-read-text-file` (dialog scope).

---

## Database Migration (v1 → v2)

`db/schema.rs` gains a `migrate(conn)` function reading `PRAGMA user_version`:

```
if user_version < 2:
    CREATE TABLE ssh_keys
    CREATE TABLE session_history
    ALTER TABLE servers ADD COLUMN ssh_key_id TEXT   (guarded)
    CREATE INDEX idx_history_connected
    PRAGMA user_version = 2
```

Existing rows/data untouched. All pre-existing commands unchanged.

## File Change Summary

| File | Action |
|---|---|
| `src-tauri/Cargo.toml` | add `zip`, `tauri-plugin-updater`, `tauri-plugin-dialog` |
| `src-tauri/tauri.conf.json` | updater config, publisher/copyright, createUpdaterArtifacts |
| `src-tauri/capabilities/default.json` | updater/dialog/fs permissions |
| `src-tauri/.gitignore` | ignore signing key + backups |
| `src-tauri/src/paths.rs` | NEW portable-mode path resolution |
| `src-tauri/src/backup.rs` | NEW zip backup/restore |
| `src-tauri/src/history.rs` | NEW session history |
| `src-tauri/src/sshkeys.rs` | NEW SSH key management |
| `src-tauri/src/db/schema.rs` | migration framework v1→v2 |
| `src-tauri/src/db/operations.rs` | history/keys operations |
| `src-tauri/src/db/mod.rs` | use paths.rs; re-init after restore |
| `src-tauri/src/commands/mod.rs` | register new modules |
| `src-tauri/src/commands/servers.rs` | ssh_key_id field; attach command |
| `src-tauri/src/commands/ssh.rs` | history record + `-i` key flag |
| `src-tauri/src/commands/backup.rs` | NEW commands |
| `src-tauri/src/commands/history.rs` | NEW commands |
| `src-tauri/src/commands/sshkeys.rs` | NEW commands |
| `src-tauri/src/commands/settings.rs` | backup/restore/updater wiring |
| `src-tauri/src/lib.rs` | register plugins + new commands |
| `src/types/index.ts` | new TS types (HistoryEntry, SshKey, BackupSummary) |
| `src/services/tauri.ts` | new invoke wrappers |
| `src/store/useStore.ts` | history/keys/backup actions |
| `src/components/Sidebar.tsx` | Recent Connections |
| `src/components/Settings.tsx` | portable badge, backup/restore, updater |
| `src/components/SshKeys.tsx` | NEW key management |
| `src/components/ServerForm.tsx` | key selector |
| `src/components/App.tsx` (or Layout) | nav entry for SshKeys |

## Verification

1. `cargo check` clean.
2. `npx tsc --noEmit` clean.
3. `npm run build` succeeds.
4. `npm run tauri:build` produces `.msi` + `latest.json` update artifact (WiX).
5. Manual: portable mode via marker file, backup → delete data → restore, recent connections reconnect, key attach passes `-i`.
