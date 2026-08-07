# Remote Manager — Project Audit Report

**Date:** 2026-08-08
**Head:** `25a4783` (v0.3.3)
**Scope:** Tauri 2 · React 18 · TypeScript · Rust · SQLite · Windows

---

## 1. Executive Summary

Remote Manager is a functional Tauri 2 desktop MVP for Windows sysadmins to manage
SSH/RDP hosts. The core skeleton (host CRUD backend, groups, credentials with DPAPI,
SSH keys, session history, backup/restore, CI/CD release pipeline) works and is
released through v0.3.3. However, the **frontend is substantially incomplete**:

- The Settings page and Update panel are **unreachable**.
- Group creation is a **no-op** (broken UI wiring).
- There is **no delete/clone host UI**, no bulk import/export UI, no credential
  management UI, no nested folder tree, no favorites section, no working global
  search, and **no keyboard shortcuts**.
- The theme setting is persisted but **never applied** (dark mode hardcoded).
- Error handling is missing across most async paths — failures become silent or
  surface as raw Rust error strings.
- **Zero tests**, no lint config, no frontend build step in CI, and
  `src-tauri/Cargo.lock` is not committed.

The gaps below are organized by priority and severity. Fixes are implemented as
part of this engagement in `docs/superpowers/plans/2026-08-08-stabilization.md`.

---

## 2. Architecture Issues

| # | Severity | Issue | Location |
|---|----------|-------|----------|
| A1 | **High** | Settings page + Updater panel unreachable (no view/route imports them). | `Layout.tsx`, `Settings.tsx` |
| A2 | **High** | Group creation "+" button is a no-op: `newGroupName` state exists but no input is rendered. | `Sidebar.tsx:11,16-21,94` |
| A3 | High | Favorites quick-access row has `cursor:pointer` but no `onClick`. | `Sidebar.tsx:49-58` |
| A4 | Medium | Store actions (`deleteServer`, `createCredential`, `updateGroup`, `deleteGroup`) exist but have no UI callers. | `useStore.ts:85,109-128` |
| A5 | Medium | Nested groups never rendered (only `!g.parent_id`); no rename/delete group UI. | `Sidebar.tsx:99-110` |
| A6 | Medium | `searchQuery`/`setSearchQuery`/`selectedServerId`/`isLoading` store state is dead. | `useStore.ts:12,14,15,37,39,49,51,52,163,168` |
| A7 | Low | Type-unsafe `server as Server` cast in store create. | `useStore.ts:76` |
| A8 | Low | `@/*` path alias defined in tsconfig but never used. | `tsconfig.json:19-21` |

---

## 3. Security Issues

| # | Severity | Issue | Location |
|---|----------|-------|----------|
| S1 | **High** | `relaunch()` from `@tauri-apps/plugin-process` is called but `tauri-plugin-process` is **not registered** in Rust — guaranteed runtime failure after an update. | `UpdaterPanel.tsx:36`; `lib.rs:20-23` |
| S2 | Medium | `"csp": null` in production config — no Content Security Policy. | `tauri.conf.json:22` |
| S3 | Medium | `cmd_get_credential_password` returns plaintext password over IPC; frontend never uses it (dead), but it is a plaintext-exposure surface. | `commands/credentials.rs:34-43` |
| S4 | Medium | `catch (e: any)` + `e.toString()` surfaces raw Rust/OS error strings in UI (info-leak surface). | 9 call sites across `src/` |
| S5 | Low | `#![allow(...)]` DPAPI fallback on non-Windows encodes plaintext base64 (dev-only; acceptable). | `security/dpapi.rs:92-103` |
| S6 | Low | No validation on `port` bounds in `cmd_create_server`/`cmd_update_server` (negative/huge ports pass to OpenSSH). | `commands/servers.rs` |
| S7 | Low | Import/export commands accept arbitrary `path` without restriction (a malicious local actor could read/write anywhere the app can). | `commands/import_export.rs` |

**Good:** DPAPI is used for credential encryption; signing key is gitignored and
never tracked; private keys are stored encrypted in the app data dir with ACLs
restricted (per recent fix).

---

## 4. Performance Issues

| # | Severity | Issue | Location |
|---|----------|-------|----------|
| P1 | Medium | `searchServers` fires a DB query on **every keystroke** with no debounce. | `SearchBar.tsx:10-13` |
| P2 | Medium | Every store mutation re-fetches the **entire** server/group list; rapid favorite toggles fan out into many queries. | `useStore.ts:54-161` |
| P3 | Low | `React.StrictMode` + empty dep array causes all 6 startup loads to run twice in dev. | `App.tsx:8-15`, `main.tsx:16` |
| P4 | Low | `handleAddGroup`/`createGroup` re-fetches all groups after a single insert. | `useStore.ts:104-107` |

---

## 5. Missing Features

| Requirement | Status |
|---|---|
| Create/Edit server | ✅ (modal form) |
| **Delete server** | ❌ No UI (backend `cmd_delete_server` exists) |
| **Clone/duplicate host** | ❌ Not implemented (neither backend nor UI) |
| **Bulk import / bulk export UI** | ❌ Service wrappers exist, no UI |
| **Description field** | ❌ Not in schema or form |
| **Folder hierarchy / nested tree / drag-drop / persist expansion** | ❌ Only flat root groups, no tree |
| **Favorites section** | ❌ Toggle works; section is a dead row |
| **Recent list** | ⚠️ Top-5 in sidebar; no timestamps |
| **Tag filter / protocol filter / folder filter in search** | ❌ Plain text search only |
| **Keyboard shortcuts** | ❌ None (Ctrl+K hint is decorative) |
| **Credential vault UI** (add/edit/delete/test) | ❌ No UI |
| **Import/Export CSV + JSON UI** | ❌ No UI |
| **Settings page reachable** | ❌ Dead code |
| **Backup/Restore reachable** | ❌ Only inside unreachable Settings |
| **Theme toggle applied at runtime** | ❌ Hardcoded dark; setting never read |
| **RDP fullscreen/admin/resolution** | ⚠️ Launch works; defaults ignored; no resolution |
| **SSH key passphrase prompt / password auth via WT** | ⚠️ Key works after recent fix; password handled by OpenSSH prompt |
| **Error boundary / loading state** | ❌ None |

---

## 6. Technical Debt

- No `lint`, `test`, or `format` scripts/configs (no ESLint, Prettier, Vitest).
- CI runs only `tsc --noEmit` + `cargo check`; no `vite build`, `cargo test`, `cargo clippy`, `cargo fmt --check`.
- `src-tauri/Cargo.lock` gitignored → non-reproducible builds and broken Rust cache key.
- `tauri-apps/tauri-action@v0` floating tag; choco WiX install is slow/redundant; no `concurrency` guard on `release.yml`.
- Cargo.toml metadata placeholders (`authors=["you"]`, empty license/repository).
- `src-tauri/gen/schemas` both committed and gitignored (ambiguous).
- Release body references `Remote Manager_*_x64_en-US.zip` but asset is `portable.zip`.

---

## 7. Dead Code

| Item | Location |
|---|---|
| `Settings.tsx`, `UpdaterPanel.tsx` (unreachable) | `src/components/` |
| Store: `isLoading`, `searchQuery`, `setSearchQuery`, `selectedServerId`, `setSelectedServer` | `useStore.ts` |
| Store actions never called from UI: `deleteServer`, `createCredential`, `deleteCredential`, `updateGroup`, `deleteGroup` | `useStore.ts` |
| Services never called: `getServer`, `getCredentialPassword`, `importCsv`, `exportCsv`, `exportJson`, `importJson`, `attachKey` | `services/tauri.ts` |
| Deps unused: `@mantine/hooks`, `@xterm/xterm`, `@xterm/addon-fit`, `@tauri-apps/plugin-shell` | `package.json` |

---

## 8. Build Issues

| # | Severity | Issue | Fix |
|---|----------|-------|-----|
| B1 | **High** | Fresh clone + `tauri:build` fails without `TAURI_SIGNING_PRIVATE_KEY` (updater artifacts require signing). | Document; optionally gate artifacts on env. |
| B2 | High | `relaunch()` needs `tauri-plugin-process` (Cargo.toml + lib.rs + capability). | Register plugin or remove call. |
| B3 | Medium | No frontend build step in CI (Vite errors escape PRs). | Add `npm run build` to `ci.yml`. |
| B4 | Medium | Rust cache key hashes gitignored `Cargo.lock` → constant key, cache ineffective. | Commit lockfile; drop custom key. |
| B5 | Low | `tsc --noEmit` redundant (tsconfig already `noEmit`). | Harmless. |
| B6 | Low | Node 20 floating minor vs Vite 7 requirement (≥20.19). | Pin `20.19+` or use 22. |

---

## 9. Error Handling Audit

- **Unhandled promise rejections:** `App.tsx:8-15` (all startup loads), `useStore.ts` (all actions), `Sidebar.handleAddGroup`, `ServerList.toggleFavorite`, `ServerForm` submit + effect, `SearchBar`.
- **Good patterns:** `SshKeys.tsx`, `Settings` backup/restore, `UpdaterPanel` use try/catch + notifications.
- `Number('')` → `0` port edge case in `ServerForm.tsx:71` / `Settings.tsx:63,71`.

---

## 10. Priority Fix List

**P0 (correctness):**
1. Register `tauri-plugin-process`; wire Settings view into Layout.
2. Fix group creation UI; make Favorites clickable; show nested groups.
3. Add delete + clone host UI (backend command for clone).
4. Implement Ctrl+K / Ctrl+N / Ctrl+E / Ctrl+F shortcuts.
5. Commit `src-tauri/Cargo.lock`; remove from `.gitignore`.

**P1 (MVP completion):**
6. Apply theme setting at runtime (Mantine color scheme).
7. Credential vault UI (add/edit/delete/test).
8. Bulk import/export UI + description field + tag/folder/protocol filters.
9. Debounce search; wire `searchQuery`.
10. Add `cargo clippy`, `cargo fmt --check`, `cargo test`, `vite build` to CI.
11. Add ErrorBoundary, loading state, normalize errors.

**P2 (hardening):**
12. CSP policy; remove unused deps; port bounds validation.
13. RDP defaults applied + resolution + credential injection.
14. Add `concurrency` guard and asset-name fix in release workflow.
15. Add unit + integration tests (Rust + Vitest).
