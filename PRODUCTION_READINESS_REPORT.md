# Production Readiness Report

**Project:** Remote Manager MVP (Tauri 2 + React/TS)
**Reviewer roles:** Senior QA Engineer · Security Auditor · SysAdmin User
**Scale assumption:** 1000 managed servers
**Version reviewed:** 0.3.4
**Date:** 2026-08-08

---

## Executive Summary

The application is functional for small fleets but is **not production-safe at 1000 servers**. The review found **4 Critical** and **6 High** severity issues. Three Criticals are security issues (command injection surface, path-traversal file write in restore, plaintext-password IPC exposure); one is a data-loss risk in the restore workflow. Several High issues affect the 1000-server scenario directly (no pagination/virtualization, full-fleet reload on every mutation, single global DB lock, tag schema divergence).

All Critical and High issues are being remediated automatically; the report is a living document and is re-checked until no Critical issues remain.

---

## Critical Issues

### C1 — Command injection / weak input validation in SSH & ping commands
- **Severity:** Critical
- **Location:** `src-tauri/src/commands/ssh.rs` (`validate_input`), `cmd_launch_ssh`, `cmd_ping`
- **Root cause:** `validate_input` only blocks `; | & \``. Host/username strings are interpolated into `wt.exe ssh ...` / `cmd /C start ssh ...` command lines. A host like `127.0.0.1 /c calc.exe` or a username containing `%COMSPEC%` / `&` variants / `\n` is not fully neutralized. `cmd /C start ssh` is a shell, so metacharacters (spaces, `(`, `)`, `<`, `>`, `%`, `^`, `\n`) reach cmd.exe. An imported CSV/JSON (or a malicious saved server) could execute arbitrary commands on the operator's machine.
- **Fix recommendation:** Replace blacklist with a strict whitelist validator for host/username (alphanumeric, `.`, `-`, `_`, `@`, `:`, `[`, `]`, `%` only where valid). Drop the `cmd /C start` fallback in favor of spawning `ssh.exe` directly via `Command` with no shell; use `cmd /K` only as a last resort with fully validated tokens. Block CR/LF globally in inputs.
- **Status:** ✔ Fixed (whitelist validator `security::net::validate_host` / `validate_username`; cmd fallback removed; CR/LF rejected).

### C2 — Path traversal / arbitrary file write in backup restore
- **Severity:** Critical
- **Location:** `src-tauri/src/backup.rs` `restore()`
- **Root cause:** Archive entry names are sanitized only by stripping leading `/` (`name.trim_start_matches('/')`), then `data_dir.join(clean)` is written with `fs::File::create`. A crafted backup containing `../evil.exe` or `..\..\Windows\...` escapes `data_dir` and writes anywhere the user can write (classic **zip-slip**). Because restore trusts the archive manifest and lacks per-entry path validation, a malicious `.rmbackup` file can overwrite arbitrary files.
- **Fix recommendation:** Reject any entry whose normalized path escapes the target directory (reject `..`, absolute paths, drive letters, and backslashes converted to forward slashes). Validate the DB is a valid SQLite file *before* replacing live data.
- **Status:** ✔ Fixed (canonical path containment check before every extraction; DB integrity check before swap).

### C3 — Restore deletes the only pre-restore copy on success → data loss
- **Severity:** Critical
- **Location:** `src-tauri/src/backup.rs` `restore()`
- **Root cause:** The pre-restore `data_dir` is renamed to a `data-backup-pre-restore-*` folder, and after extraction that safety folder is **deleted** (`remove_dir_all`). If the restored backup is corrupt, an older version, or the restore partially fails later (e.g., an entry errors mid-extraction), the operator has **no copy of their original database** — permanent data loss. The app only re-creates the safety dir on restore; there is no redundant copy.
- **Fix recommendation:** Never delete the pre-restore copy automatically. Keep the `data-backup-pre-restore-*` folder (surface its path in the UI), and only remove it on explicit user confirmation. Also validate the restored DB (open + `PRAGMA integrity_check`) before swapping so a bad restore is rolled back.
- **Status:** ✔ Fixed (safety copy retained with a marker; restored DB integrity-checked; rollback on failure; clear message to user).

### C4 — Plaintext credential password exposed over IPC
- **Severity:** Critical
- **Location:** `src-tauri/src/commands/credentials.rs` `cmd_get_credential_password`, registered in `src-tauri/src/lib.rs`
- **Root cause:** A Tauri command returns the decrypted password as plaintext to the renderer over the IPC channel. Any XSS in the webview (or a compromised renderer, or a malicious Tauri plugin) can exfiltrate every stored password. The frontend does not even use this command today (`grep` finds no caller) — it is pure attack surface.
- **Fix recommendation:** Remove the command from the invoke handler (no caller). Keep decryption backend-only for launch-time credential injection.
- **Status:** ✔ Fixed (command removed from `invoke_handler`; no frontend caller).

### C5 — CSV export/import column mismatch breaks round-trip
- **Severity:** Critical (data integrity)
- **Location:** `src-tauri/src/commands/import_export.rs`
- **Root cause:** `cmd_export_csv` writes columns `name,host,port,protocol,username,tags,notes`, but `cmd_import_csv` read them as `name,host,protocol,username`. Re-importing an exported CSV put the **port in the protocol column**, so every SSH/port row was rejected or mis-mapped — the backup/export workflow silently corrupted or dropped data.
- **Fix recommendation:** Align the importer to the exporter's column order and validate port range on import.
- **Status:** ✔ Fixed (importer reads `name,host,port,protocol,username`; port range validated; host/username whitelist-validated).

---

## High Issues

### H1 — No pagination/virtualization → unusable at 1000 servers
- **Severity:** High
- **Location:** `src/components/ServerList.tsx`, `src/store/useStore.ts`
- **Root cause:** `ServerList` renders every server as a `<Paper>` in a `<Stack>`. At 1000 servers this means ~1000 DOM cards and ~1000 network rows; with Mantine's theme hooks per component this is slow to first paint and sluggish on interaction. `listServers(groupId)` also returns the full group regardless of size.
- **Fix recommendation:** Paginate client-side (e.g., 100 per page with "Load more") or virtualize. Keep favorites/search scoped.
- **Status:** ✔ Fixed (client-side pagination with Load More, reset on filter change).

### H2 — Full-fleet reload on every mutation
- **Severity:** High
- **Location:** `src/store/useStore.ts`
- **Root cause:** `createServer/updateServer/deleteServer/toggleFavorite` call `loadServers()` (a full DB `SELECT` + full IPC transfer) after every single row mutation. At 1000 servers, favoriting one server transfers ~1MB over IPC and re-renders the whole list. Groups and favorites also trigger reloads on every click.
- **Fix recommendation:** Apply local state mutation after successful IPC and refetch only when needed (group switch/search). Batch load.
- **Status:** ✔ Fixed (optimistic local updates; `setSelectedGroup` re-uses existing `servers` when `selectedGroupId` is unchanged; delete/favorite update in place; client-side pagination avoids full-fleet DOM).

### H3 — Single global DB mutex serializes all commands
- **Severity:** High
- **Location:** `src-tauri/src/db/mod.rs` (`AppState.db: Mutex<Connection>`)
- **Root cause:** Every command locks the single `Connection`. Slow operations (`cmd_ping` runs a 3s OS ping; launch commands spawn processes while holding the lock) block all other commands, freezing the UI under concurrent load (e.g., mass ping, multi-connect). With a 1000-server fleet operators will click several servers / ping several hosts in quick succession.
- **Fix recommendation:** Use a connection pool (rusqlite `Connection` is not `Sync`; use `r2d2_sqlite` or a `Mutex` + a short-lived read connection pattern). At minimum, do not hold the lock across process spawns; spawn and record history after releasing the lock.
- **Status:** Partially mitigated — `cmd_ping` is already lock-free (no DB access). Launch commands hold the lock only for the brief history write, not across spawn. Full pool migration out of scope for MVP; documented as follow-up.

### H4 — Tag data model divergence
- **Severity:** High
- **Location:** `src-tauri/src/db/schema.rs` (v3), `src/components/ServerForm.tsx`
- **Root cause:** Two parallel tag systems: the legacy `servers.tags` TEXT column (comma-separated, still written by `ServerForm`) and the new normalized `tags`/`host_tags` tables (`cmd_set_server_tags`). They are never synced: editing tags in the form updates the column only; `cmd_set_server_tags` updates tables only. Search filters on the column; tag browsing uses the tables. State diverges and search misses tags.
- **Fix recommendation:** Single source of truth. Either drop the column and write tags via the tables on server create/update, or keep the column and drop the tables. Sync on create/update.
- **Status:** ✔ Fixed (create/update now persist tags through `set_server_tags`-style write to `host_tags`; `ServerRow.tags` is derived to stay consistent; search still matches column — tags are also normalized on write).

### H5 — `cmd_launch_rdp` ignores credential vault; `cmd_launch_ssh` only uses keys
- **Severity:** High
- **Location:** `src-tauri/src/commands/ssh.rs`
- **Root cause:** Stored credentials (`credential_id`) are never used at launch time. RDP is launched without the saved password (only `username` is written to the `.rdp` file; `mstsc` will still prompt). SSH uses only keys; a server with a saved password credential does not auto-authenticate. The vault is therefore "display only" — operators will still type passwords manually for every server, defeating the vault's purpose at 1000 servers.
- **Fix recommendation:** For RDP, do not embed passwords in `.rdp` (mstsc does not reliably honor them and it's a security anti-pattern); instead surface that the password is remembered and use `cmdkey`/CredSSP-friendly approach, or clearly pass through. For SSH, an interactive prompt is unavoidable without agent keying; at minimum, launch with the correct `-l` username resolved from the credential, and for RDP inject `prompt for credentials` config so the operator just confirms. Document that vault is for reference + username resolution.
- **Status:** Partially addressed — launch commands now resolve `username` from the credential vault when a credential is attached; honest messaging added. Embedding RDP/SSH passwords in command files is intentionally avoided (security); documented as known limitation.

### H6 — No automatic backup; backup retention absent
- **Severity:** High
- **Location:** `src-tauri/src/backup.rs`, Settings UI
- **Root cause:** Backup is manual-only. A sysadmin managing 1000 servers will lose data on disk failure; there is no scheduled/auto backup or retention policy, and no warning when the DB or keys change. The updater/release pipeline also never triggers a backup before migrations.
- **Fix recommendation:** Auto-backup on startup (daily, keep last N, e.g., 7) into the data dir `backups/` folder; surface last-backup time in Settings; offer backup-before-restore (already exists).
- **Status:** ✔ Fixed (startup auto-backup, max 7 retained, path surfaced).

### H7 — `delete_credential` / `delete_ssh_key` leave dangling references
- **Severity:** High
- **Location:** `src-tauri/src/db/operations.rs`
- **Root cause:** Deleting a credential or SSH key removes the row, but `servers.credential_id` / `servers.ssh_key_id` columns keep pointing at the now-missing id. The UI then shows a "Password"/"Key" badge whose lookup fails silently, and reconnect attempts pass a stale `ssh_key_id` (launch falls back to interactive auth without warning). Over time the fleet accumulates dead references.
- **Fix recommendation:** `ON DELETE SET NULL` semantics — before deleting, `UPDATE servers SET credential_id=NULL WHERE credential_id=?` (and same for ssh_key_id), or run it inside the delete transaction.
- **Status:** ✔ Fixed (delete ops now NULL-out dependent server rows).

---

## Medium / Low findings (documented, not auto-fixed in this pass)

| ID | Issue | Severity | Notes |
|----|-------|----------|-------|
| M1 | No tests (unit/integration/e2e) for core flows | Medium | Must reach ≥80% coverage per MVP target; see TEST_PLAN |
| M2 | `cmd_is_portable` reads marker dir each call | Low | Trivial; acceptable |
| M3 | History prune `DELETE ... NOT IN (SELECT ...)` is O(n²)-ish at scale | Medium | Only 200 kept; acceptable but re-check at scale |
| M4 | Search uses `LIKE '%q%'` full scan | Medium | At 1000 rows fine; FTS5 if fleet grows to 10k+ |
| M5 | No CSRF/scope separation between Tauri commands | Low | Local app; webview CSP present; acceptable for MVP |
| M6 | `.rmbackup` has no encryption | Medium | Keys + encrypted creds are DPAPI-protected inside DB; backup zip is plaintext container — acceptable for MVP, document for v1.1 |
| M7 | `settings` schema lacks validation on `update_settings` values | Low | theme/font_size unchecked server-side |
| M8 | No duplicate-host detection on create/import | Medium | Import allows 1000 dupes silently; consider dedupe hint |
| M10 | `list_recent_servers` limit unbounded from caller | Low | Caller is trusted; cap server-side anyway |

---

## Critical-only re-review (final gate)

After remediation:

- **C1 command injection:** ✔ host/username now pass whitelist validator; no shell fallback. **Resolved.**
- **C2 zip-slip:** ✔ extraction path-containment enforced. **Resolved.**
- **C3 restore data loss:** ✔ pre-restore copy retained + integrity rollback. **Resolved.**
- **C4 plaintext IPC password:** ✔ command removed, no caller. **Resolved.**
- **C5 CSV round-trip mismatch:** ✔ importer aligned to exporter + port validated. **Resolved.**

**Result: No Critical issues remain.** High-severity items H1–H7 remediated; H3 (lock scope) and H5 (full password auto-fill) partially addressed with documented limitations.

---

## Recommended follow-ups (next sprint)
1. Unit + integration tests for backup/restore, validation, tag sync (≥80%).
2. `r2d2_sqlite` connection pool to fully de-serialize DB access.
3. Encrypted backup format (AEAD) for `.rmbackup`.
4. FTS5 search index; dedupe import.
5. Multi-select bulk operations (tag/delete/connect) for fleet management.
