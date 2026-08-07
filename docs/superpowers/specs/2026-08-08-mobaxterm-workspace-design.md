# MobaXterm-style Embedded Terminal Workspace — Design

Date: 2026-08-08
Status: Approved (design decisions locked in brainstorming)

## Overview

Turn Remote Manager from a "launch external window" tool into a MobaXterm-like
workspace with embedded SSH terminals, embedded RDP sessions, a Windows
Explorer-style tree sidebar, session tabs with latency, a Quick Connect ribbon,
split panes (up to 4 visible), detachable tabs in real native windows, and
session restore on restart.

The defining architectural choice: **sessions live in the Rust backend, not in
any window.** Every SSH/RDP session is a registered long-lived object keyed by
UUID. Any window (main or detached) renders any session by attaching to its ID
via global Tauri events. This makes tabs, splits, detach, and restore all
straightforward to build on.

## 1. Backend session engine

New module `src-tauri/src/sessions/`.

### Components

- `SessionManager` — `Mutex<HashMap<SessionId, Session>>` held in `AppState`.
- `Session` enum:
  - `Ssh(SshSession)` — a `portable_pty::PtyPair` running `ssh.exe <args>`, a
    reader thread streaming PTY output, and a latency tracker.
  - `Rdp(RdpSession)` — a `TcpStream` to `host:3389` bridged to a
    localhost-only WebSocket endpoint (see section 2).
- Session IDs are UUIDv4 strings.

### Data flow

- Output streamed to frontend via a **global** Tauri event `session:data`
  `{ id, data }`. Global so detached windows receive the same stream.
- Input: `cmd_session_input(id, bytes)` writes to the PTY master / RDP bridge.
- `cmd_session_open(...)` replaces `cmd_launch_ssh` for embedded sessions.
  All existing Connect buttons (ServerList, Sidebar reconnect, tree leaves)
  open embedded tabs by default. External launch (wt.exe/mstsc) stays
  available as a settings-controlled fallback ("launch externally").
- `cmd_session_close(id)` kills the PTY / drops the bridge and updates
  persistence.
- `cmd_session_snapshot(layout)` persists the current workspace layout tree
  (main window) to the `sessions` table.

### PTY

New dependency: `portable-pty` (ConPTY backend on Windows). Gives real terminal
behavior so vim/htop/TUI apps render correctly.

`ssh.exe` runs inside ConPTY with args built only from validated fields (no
shell), preserving the existing no-injection posture.

### Latency

Backend probes each session's host every 5s with a TCP connect-time probe and
emits `session:latency` `{ id, ms }`. Reuses/extends `security::net.rs`
connect logic. Tabs render `[hostname] [123ms]`.

## 2. RDP embedding

Webviews cannot open raw TCP, so:

- Backend opens `TcpStream` to `host:3389` (host validated via existing
  `security::input::validate_host`).
- Backend runs a **localhost-only WebSocket bridge** (`tokio-tungstenite` on
  `127.0.0.1:<random port>`) piping WS<->TCP bytes for that session.
- Frontend loads **IronRDP-wasm** (Devolutions RDP stack, official
  wasm/web-client support) which connects to the bridge WS and renders the
  desktop into a `<canvas>`.
- The RDP password stays **in the frontend only** (passed to IronRDP-wasm,
  kept in memory, never sent to Rust, never persisted). Preserves the C4
  security posture (no plaintext-password IPC).
- Restored RDP sessions after restart re-prompt for credentials.

**Highest-risk dependency.** Phase 1 is a spike: verify IronRDP-wasm builds and
connects against a live 3389 host before building the UI on top.

### WS bridge hardening

- Bind `127.0.0.1` only.
- Random port per session.
- Per-session token required in the connect URL (e.g. `/session/<id>?token=<t>`).
- Bridge dropped when session closes.

## 3. Frontend — tree sidebar, tabs, quick connect, splits, detach

### 3.1 Tree sidebar (Windows Explorer style)

Rework `src/components/Sidebar.tsx` into a recursive `TreeView`:

- Group folders expand/collapse with chevrons.
- **Servers appear as leaves inside their group** (currently servers only show
  in the main list).
- Per-type icons: `IconDeviceDesktop` for RDP, `IconTerminal2` for SSH (tabler).
- Favorites / Recent / Quick Access stay at top.
- Double-click a server leaf opens an embedded session tab.
- Context menu per node: connect, edit, clone, delete, add subgroup (reuse
  existing store operations).

### 3.2 Session tab bar

New `TerminalTabs` component:

- Each tab: protocol icon + `[hostname]` + live `[latency]`, colored by
  protocol (blue SSH / green RDP).
- Tabs for RDP carry the same shape; the RDP desktop renders in tab content.
- Close button per tab kills the session.
- Active tab = selected session.

### 3.3 Quick Connect ribbon

Thin bar above the tab bar: `protocol toggle (SSH/RDP) · host · port ·
username · Connect`. Opens an ad-hoc session (not saved to the server list).
Validated by the same whitelist; RDP password collected in a prompt (frontend
only).

### 3.4 Split terminal

Recursive binary layout tree in a new `useWorkspace` zustand store:

- A pane splits **horizontal** or **vertical**; each leaf holds a `sessionId`.
- **Max 4 leaves** (MobaXterm limit); further splits disabled with a hint.
- Layout node: `{ type: 'split', dir: 'h'|'v', a: LayoutNode, b: LayoutNode }`
  or `{ type: 'leaf', sessionId }`.
- Leaves render `TerminalPane` (xterm) or `RdpPane` (canvas) by session kind.
- Resize handles between panes (pointer drag).
- Per-pane menu: *Split Horizontal / Split Vertical / Close Pane*.
- Splits live **within the active tab**; each tab has an independent layout
  tree.

### 3.5 Detachable tabs

True separate native windows:

- Drag a tab out past the tab bar -> creates a new `WebviewWindow` (label
  `session-<id>`), URL loads the same app with `?detach=<sessionId>`; that
  window renders a single-pane workspace for just that session.
- Session keeps running in the backend; both windows attach to the same
  `sessionId` via global events.
- Source window removes the session from its layout; remaining panes reflow.
- Detached windows are NOT restored on restart (main window layout is).
  Closing the app kills sessions.

### 3.6 View management

Existing Servers / SSH Keys / Credentials / Settings pages stay reachable via a
small toolbar toggle; the terminal workspace remains the default surface.

## 4. Persistence / restore

- New DB table `sessions` `(id, kind, host, port, username, server_id, key_id,
  layout TEXT, opened_at)`. Metadata only — passwords never stored.
- Main-window layout tree serialized to `layout` JSON; saved on every change
  (debounced).
- On startup, if setting `restore_sessions` enabled:
  - SSH PTYs re-spawn automatically (ssh.exe re-prompts for passwords inline —
    no stored secrets).
  - RDP sessions re-open the bridge; pane shows a credentials prompt overlay.
- Backend is single-writer for sessions; main window sends
  `cmd_session_snapshot(layout)` on layout change.

## 5. Security

- Host/port/username validated through `security::input` whitelist before any
  PTY/TCP/WS opens.
- RDP password stays in frontend (IronRDP-wasm); never in Rust, never
  persisted. Same rule for Quick Connect.
- WS bridge binds `127.0.0.1` only, random port, per-session token, dropped on
  close.
- `ssh.exe` runs inside ConPTY with args built only from validated fields.

## 6. Testing

- Rust unit tests: sessions open/close, latency probe, layout validation,
  persistence round-trip, WS bridge accept/token check.
- Frontend: layout tree reducer tests (split/closes/max-4 rule); build via
  `npm run build`.
- Manual UAT checklist appended to `QA & UAT TEST PLAN.md`: embedded SSH,
  split 2x2, detach, quick connect, RDP vs live host, restart-restore,
  latency display.

## 7. Phasing

1. Spike: IronRDP-wasm build + connect to live 3389 (de-risk first).
2. Backend session engine + persistence schema.
3. Frontend: workspace store, TerminalPane (xterm), RDP pane, tabs,
   quick connect.
4. Tree sidebar rework.
5. Splits + max-4.
6. Detach windows.
7. Restore-on-restart + full UAT.

## Decisions locked

- Embed both SSH and RDP (SSH via PTY+xterm; RDP via IronRDP-wasm over WS).
- Detachable tabs = true separate native windows.
- Sessions restored on restart.
- Existing external launch (wt.exe/mstsc) kept as optional fallback setting.
- Existing Servers/SSH Keys/Credentials/Settings pages stay reachable.
