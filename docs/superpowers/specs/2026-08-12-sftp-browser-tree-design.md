# SFTP Browser Tree Design

Date: 2026-08-12

## Summary

Build a full SFTP file browser (WinSCP-style) embedded in the app sidebar. It
shows a lazy-loaded folder tree starting at the remote home directory
(`/home/<user>`), lists files and folders, supports drag/drop upload from
Windows Explorer (including recursive folder upload), and download of
multi-selected files/folders to a local directory chosen via an OS folder
picker. Built for both apps (Tauri + Sciter) with a shared backend core.

This replaces the simple "drop to ~" upload box built previously.

## Goals

- Browse the remote filesystem in a sidebar tree, defaulting to `/home/<user>`.
- Upload by dragging files/folders onto any folder node (recursive for folders).
- Download by multi-selecting files/folders and choosing a local destination
  via OS folder picker.
- Follow the active SSH tab: the tree tracks the server of the currently
  active SSH session and reloads when the tab changes.
- Show file size on hover, hide dotfiles by default (toggleable), refresh
  automatically after transfers and via a manual refresh button.
- Work in both Tauri and Sciter apps, sharing the backend SFTP logic.

## Non-Goals

- No rename/delete/mkdir/rm UI.
- No resume of partial transfers.
- No conflict dialogs — name collisions are handled by overwriting.
- No per-file detailed progress scaling beyond a transfer-level progress bar.
- No local-machine pane (single-pane remote only).

## Architecture

Approach A: a persistent SFTP session per active server.

`SftpBrowserManager` owns at most one live `russh_sftp::client::SftpSession`
keyed by `server_id`. The session is kept alive while that server is the
active SSH tab; switching tabs or closing the session disconnects the old one
(with optional lazy reconnect on the next operation).

The existing `UploadManager` progress/cancel job machinery is generalized into
a shared job store (`jobs: Arc<Mutex<HashMap<String, Job>>>`, `AtomicBool`
cancel flags, `Arc<Mutex<UploadProgress>>`). Upload and download batches both
run on background threads via a current-thread tokio runtime, exactly like the
current `run_upload` pattern.

### Backend API (shared by both apps)

- `open(server_id, auth, host, port, username)` -> `home` (resolves `~` via
  `canonicalize(".")`). Discards any existing session for that server.
- `close(server_id)` — drop session (called when tab changes).
- `list_dir(path)` -> `Vec<RemoteEntry { name, is_dir, size, mtime, is_hidden }>`.
  Dot-prefixed names are marked `is_hidden`; filtering happens in the UI.
- `get_home(server_id)` -> remote home path.
- `start_upload(server_id, remote_dir, local_paths)` -> `job_id` (recursive for
  folders; overwrites existing remote files; streamed progress as today).
- `start_download(server_id, remote_paths, local_dir)` -> `job_id` (recursive
  for remote folders; creates missing local dirs; overwrites).
- `get_progress(job_id)`, `cancel(job_id)` — unchanged semantics.

Authentication resolution is reused: `resolve_username`,
`resolve_password`/`resolve_ssh_key`, DPAPI `security::decrypt` on Tauri.

### Multiplexing

One logger session is used for everything. Each operation opens its own SFTP
channel against the shared session (russh supports concurrent channels).
Locking: `list_dir` takes a short-lived mutex on the session; transfer jobs
copy the session handle and run on their own channels, so navigation is not
blocked by an active transfer.

### Error handling

- Session drop between operations → auto-reconnect once on the next call; a
  second failure surfaces a "disconnected" toast and marks the tree state.
- Transfer/list errors → red toast with the russh error text.
- Writing to a non-existent remote dir → clear error ("No such file or
  directory") surfaced via the job error field.
- Cancellation honored at the file and chunk level (as today).

## Data Model

```rust
struct RemoteEntry {
    name: String,
    is_dir: bool,
    size: u64,
    mtime: u64,
    is_hidden: bool,
}
```

`UploadProgress` (existing) is reused for both directions:
`state, current_file, file_index, total_files, bytes_sent, total_bytes, error`.

## UI

### Tauri (React + Mantine + Tauri v2)

- `Sidebar.tsx`: replace the current drop-zone block with a new
  `SftpBrowser.tsx` component (keeps `activeServerId`/`activeServerHost`,
  clear-history button).
- Tree: folder nodes expand/collapse with `+`/`−`; file nodes show an
  icon, size tooltip, and a download button (context menu or per-file icon).
  Hidden files filtered by a small toggle.
- Drag/drop via `getCurrentWebview().onDragDropEvent` (existing pattern);
  dropping on a folder node uploads into that node's dir (recursive).
- Multi-select (Ctrl/Shift) then Download → `tauri-plugin-dialog` folder
  picker → `cmd_sftp_download`.
- Transfer progress bar + Cancel (reuse the 250 ms polling pattern).
- Auto-refresh the open dir after a transfer; manual refresh button.
- Tauri commands (all `rename_all = "snake_case"`):
  `cmd_sftp_open`, `cmd_sftp_list`, `cmd_sftp_get_home`, `cmd_sftp_upload`,
  `cmd_sftp_download`, `cmd_get_upload_progress`, `cmd_cancel_upload`.

### Sciter (TIScript in `sciter-app/ui/index.html`)

- Replace the current drop-zone block with `#sftp-browser` tree (same
  structure: folder expand/collapse, file download button, size tooltip,
  hidden toggle).
- Native calls via `view.*` (existing pattern):
  `view.sftp_open`, `view.sftp_list`, `view.sftp_upload`,
  `view.sftp_download`, `view.get_progress`, `view.cancel`.
- Drag/drop via existing `dragover`/`dragleave`/`drop` bindings.
- Folder picker: try `view.select_folder()` / `frame.select_folder()`
  (verify API during implementation); fall back to a local-path text input
  if the Sciter runtime lacks an OS folder dialog.
- Progress bar + Cancel, auto-refresh, manual refresh — as Tauri.

### Blocking risk: Sciter native dispatch

The Sciter app previously failed to dispatch `view`/`on_script_call` (the
old upload feature depends on `view.*`). Verify this first. If unresolved,
Tauri is implemented and shipped first; Sciter is deferred until the runtime
issue is fixed. Do not sink effort into Sciter UI while native calls cannot
reach the backend.

## Testing

- `cargo check` clean for both `src-tauri` and `sciter-app`.
- `npm run build` clean.
- Add `cargo test` unit tests for the recursive upload/download walkers
  (pure functions computing file lists / remote path mapping).
- Manual integration against a real SSH server: open tree, expand folders,
  upload file folder (recursive), download multi-selected items, cancel
  mid-transfer, verify refresh after transfer, verify overwrite behavior.

## Files

- `src-tauri/src/sftp.rs` — generalize to browser manager + download/upload jobs.
- `src-tauri/src/commands/uploads.rs` — add SFTP browser commands.
- `src-tauri/src/lib.rs`, `src-tauri/src/db/mod.rs` — AppState, registrations.
- `src-tauri/Cargo.toml` — add `tauri-plugin-dialog`.
- `src/components/SftpBrowser.tsx` — new tree component (replaces DropZone usage).
- `src/components/Sidebar.tsx` — mount SftpBrowser.
- `src/services/tauri.ts` — new command wrappers.
- `src/types/index.ts` — `RemoteEntry` type.
- `sciter-app/src/backend/sftp.rs` — shared logic (copied to Sciter).
- `sciter-app/src/handler.rs` — new match arms for sftp commands.
- `sciter-app/ui/index.html` — tree UI replacing the drop zone.