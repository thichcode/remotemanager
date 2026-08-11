# Design: SFTP Drag/Drop File Upload

**Date:** 2026-08-11
**Status:** Approved by user

## Problem

SysAdmins need to quickly copy files from their Windows PC to Linux servers. Today
the app has SSH terminals and RDP sessions but no file transfer. This adds
drag-and-drop upload to the server.

## Scope

- Both builds: **Sciter** (`sciter-app/`, lightweight) and **Tauri** (`src-tauri/`, main app).
- Replace the "Recent" area in the left sidebar with a drop zone (Tauri). The Sciter
  sidebar has no "Recent" yet, so a drop zone is added at the bottom of the left panel.
- Files only (no folders) — single or multi-file.
- Upload target: home directory (`~`) of the logged-in user.
- Progress UI: progress bar + cancel button.

## Key Decisions (from user Q&A)

1. **Library:** `russh` + `russh-sftp` (pure Rust, async, supports both password and SSH key auth).
2. **Server targeting:** the drop zone follows the currently active SSH tab. Dropping uploads
   to the server of the active SSH tab.
3. **No active SSH tab:** drop zone is disabled with a hint ("Open an SSH terminal to upload") and ignores drops.
4. **Destination:** always `~` (home directory).
5. **Files only** — directories are not supported in this iteration.

## Architecture

### Shared backend module — `backend/sftp.rs` (both apps)

Pure Rust SFTP uploader built on `russh`/`russh-sftp`:

- `upload_file(host, port, username, auth, local_path, remote_dir) -> UploadStream`
  - `auth` is an enum: `Password(String)` or `Key(String)` (private key path).
- Establishes a fresh SSH connection per upload batch (matches the current per-request
  connection model; no long-lived sessions).
- Streams progress (bytes sent / total) through an mpsc/oneshot channel.
- Supports cancellation: a `CancellationToken`-style flag checked during the upload loop;
  on cancel, aborts the file transfer and closes the SSH session.
- Returns: success, partial failure (e.g. file 3 of 5 failed — report which), or cancelled.

### Credential reuse

Reuse existing resolvers:
- Sciter: `handler.rs` `resolve_password` / `resolve_ssh_key` / `resolve_username`
  (already decrypt DPAPI-encrypted credentials).
- Tauri: equivalent helpers in `src-tauri/src` (verify names at implementation time).

### Native command surface

**Sciter** (`on_script_call` + `handler.rs`):
- `upload_files(server_id, local_paths: [])` → returns an upload job id (or null on failure).
- `get_upload_progress(upload_id)` → `{ total, done, current_file, state }` (polled or pushed).
- `cancel_upload(upload_id)`.
- Progress is surfaced to the UI via a push from the backend thread; in Sciter this uses
  a callback registered from JS (e.g. `view.on_upload_progress = fn` pattern already
  explored) — simplest reliable path: **poll** `get_upload_progress` every ~250ms.

**Tauri** (commands in `src-tauri/src`):
- `upload_files(server_id, local_paths)` → upload job id.
- `get_upload_progress(upload_id)`.
- `cancel_upload(upload_id)`.
- UI polls progress similarly.

### UI

**Tauri — `src/components/Sidebar.tsx`**
- Replace the "Recent" section (currently lines ~104-108 with the clear-history button)
  with a drop zone component:
  - Reads the active SSH tab server from app state.
  - If no active SSH tab: disabled state + hint text, drop ignored.
  - Drop event: gets dropped file paths from the DOM event, calls `upload_files`.
  - Shows per-job progress bar + cancel button.
- The clear-history action moves elsewhere or is kept small (decide during implementation;
  prefer keeping it as a tiny icon button near the drop zone header).

**Sciter — `sciter-app/ui/index.html`**
- Add a drop zone at the bottom of the left panel.
- Same behavior: follows active SSH tab, disabled without one, progress bar + cancel.
- Uses Sciter DOM events for drag/drop (confirm exact event name for file drop in
  Sciter/TIScript — `ondrop`; verify at implementation).

## Data flow

1. User drags files from Explorer onto the drop zone.
2. UI resolves server from active SSH tab; calls native `upload_files`.
3. Native spawns a background task: connect via russh → open SFTP channel → for each file,
   open remote file at `~/<basename>` → stream chunks → report progress.
4. UI polls progress; renders progress bar; cancel sends `cancel_upload`.
5. On finish: toast/summary (success, failed list, or cancelled).

## Error handling

- Connection/auth failure → job state `error` with message; UI shows toast.
- Per-file failure → mark file failed, continue remaining files, report failed list.
- Cancel → abort current file, close session, state `cancelled`.
- Validate host/port before connect (reuse `validate_host` in Sciter backend).

## Testing

- Unit tests: SFTP uploader with a **local sshd-less harness** is hard; instead:
  - Test auth resolution + argument validation (pure functions).
  - Integration test optional against a real SSH server if one is available on the dev machine.
- Manual test checklist (documented in plan):
  1. Open SSH terminal → drop 1 file → lands in `~`.
  2. Drop 3 files incl. one large → progress advances, cancel works.
  3. No SSH tab → zone disabled.
  4. Password auth and key auth both work.
  5. Wrong password → clear error toast.

## Out of scope (YAGNI)

- Directory/folder upload.
- Upload to a configured per-server destination path.
- SFTP download / drag-out.
- Resume interrupted transfers.
