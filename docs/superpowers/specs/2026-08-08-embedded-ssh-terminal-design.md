# Embedded SSH Terminal Design

Date: 2026-08-08

## Goal

Replace the external `wt.exe` SSH window with an embedded terminal running inside the Remote Manager app shell, using a tab system. External terminal launch remains available as a secondary option. RDP stays unchanged (`mstsc.exe`).

## Scope

- SSH sessions run inside the app in tabs
- Tab bar shown in the Servers view; each tab is one SSH session
- SSH key auth automatic via `-i`; passwords typed manually in the terminal
- Keep `launchSsh` (wt.exe) as "Open in external terminal"
- RDP unchanged
- Sessions stay alive when switching between views; killed only on tab close or app exit

## Architecture

### Backend (Rust)

New module `src-tauri/src/sessions.rs` managing ssh subprocesses.

- `SessionManager` — `Mutex<HashMap<String, Child>>` keyed by `session_id` (UUID)
- Commands registered in `lib.rs`:
  - `cmd_open_ssh_session(host, port, username, server_id, server_name, ssh_key_id, credential_id) -> String (session_id)`
    - Reuses existing validation (`validate_host`, `validate_username`, port range), key path resolution (`get_private_key_path` + `ensure_key_permissions` + `IdentitiesOnly=yes`), username resolution from credential, history record + `touch_last_connected` (same logic as current `cmd_launch_ssh`)
    - Spawns `ssh.exe` with `-i <key>`, `-p <port>`, `-t`, `user@host`, all three stdio handles pipelined
    - Spawns two threads per session: stdout reader and stderr reader, each emitting `ssh://output` `{ sessionId, data }`; exit reader emits `ssh://exit` `{ sessionId, code }`
  - `cmd_ssh_write(sessionId, data)` — writes bytes to stdin; no-op/ignored if process already exited
  - `cmd_ssh_resize(sessionId, cols, rows)` — best-effort (no real PTY on Windows spawn); keeps xterm layout consistent
  - `cmd_ssh_close(sessionId)` — kills process, removes from map
  - `cmd_ssh_close_all()` — called on app exit (`RunEvent::ExitRequested`) to clean up
- `cmd_launch_ssh` unchanged for external terminal use

Event payloads (camelCase keys, matching Tauri event convention on frontend):
- `ssh://output` → `{ sessionId: string, data: number[] }` (Uint8Array bytes)
- `ssh://exit` → `{ sessionId: string, code: number }`

### Frontend

- `src/components/Terminal.tsx` — xterm.js wrapper
  - `import '@xterm/xterm/css/xterm.css'` (currently not imported anywhere)
  - Creates xterm + `FitAddon`, mounts into a div, listens to `ssh://output` and `ssh://exit` events via `@tauri-apps/api/event`
  - `term.onData` → `cmd_ssh_write`
  - `ResizeObserver` → `fit()` + `cmd_ssh_resize`
  - On unmount → `cmd_ssh_close`
  - On exit event → render "Connection closed (code X)", set terminal readonly, keep tab open until user closes it
- `src/store/useStore.ts` — terminal tab state
  - `terminalTabs: { id, title, sessionId, serverId, status }[]`
  - `activeTerminalTabId: string | null`
  - Actions: `openTerminalTab(server)`, `closeTerminalTab(id)`, `focusTerminalTab(id)`
  - `openTerminalTab` calls `cmd_open_ssh_session`, then adds the tab; on error shows notification and removes the tab
- `src/services/tauri.ts` — new wrappers:
  - `openSshSession(args)`, `sshWrite(sessionId, data)`, `sshResize(sessionId, cols, rows)`, `sshClose(sessionId)`, `sshCloseAll()`
  - `listSshSessions()` not required
- `src/components/Layout.tsx` — tab bar
  - Terminal section (tab bar + active terminal) lives in `AppShell.Main`, mounted once and hidden/shown by CSS based on view — **never unmounted** while tabs exist, so `cmd_ssh_close` is not triggered by a view switch
  - When `terminalTabs.length > 0`: tab bar renders at top of main area, and the active tab's `Terminal` fills the main content (replacing `ServerList`)
  - When no tabs exist: tab bar hidden, `ServerList` fills the main area
  - One active tab at all times while tabs exist
  - Switching to Keys/Credentials/Settings hides the terminal section via `display:none` but keeps sessions alive (component stays mounted)
- `src/components/ServerList.tsx` — connect integration
  - `handleConnect` for SSH → `openTerminalTab(server)` (instead of `launchSsh`)
  - Add "Open in external terminal" to the actions menu → `launchSsh`
  - RDP unchanged

## Data Flow

1. Click Connect on SSH server
2. `openTerminalTab(server)` adds tab (status `connecting`), calls `cmd_open_ssh_session`
3. Returns `sessionId`, tab updated, `Terminal` mounts
4. Terminal creates xterm, subscribes to `ssh://output` / `ssh://exit`
5. User input → `cmd_ssh_write`; resize → `cmd_ssh_resize`
6. Close tab / exit → `cmd_ssh_close` kills process

## Error Handling

- Spawn failure → notification (red) + tab removed
- Exit code 255 (auth failure) → "Connection closed (code 255)" shown, screen preserved for reading
- Write to dead stdin → silently ignored (expected during teardown)
- App exit → `cmd_ssh_close_all` via `RunEvent::ExitRequested`

## Testing

- **Rust unit tests** (`sessions.rs`): session map add/remove, close kills process, write-after-exit is safe
- **E2E Playwright** (`e2e/app.spec.ts` + `tauri-mock.ts`):
  - Mock `cmd_open_ssh_session` returns a sessionId; mock emits `ssh://output` to render text in xterm
  - Typing sends `cmd_ssh_write` with expected bytes
  - Tab close calls `cmd_ssh_close`
  - Tab title shows `user@host`
- **Manual:** real SSH connection, resize, exit, multi-tab, key auth, view switching

## Out of Scope

- Embedded RDP (keeps `mstsc.exe`)
- Password auto-fill for SSH (typed manually)
- Copy/paste special handling beyond xterm defaults
- Session reconnect
