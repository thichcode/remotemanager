# Sidebar + Sessions Redesign

## Summary

Restructure the UI to:
1. Nest servers inside groups in the sidebar (remove the separate ServerList panel)
2. Unify SSH and RDP under a shared session/tab system (RDP opens as a status tab + external mstsc.exe)

## Part 1: Servers Nested in Sidebar Groups

### Current State
- Sidebar: Quick Access + Groups tree
- Main area: ServerList panel (filtered by selected group) + terminal tabs
- ServerList is hidden when terminal is open (280px sidebar mode)

### Target State
- Sidebar: Quick Access + Groups tree with servers nested under each group
- Main area: Only session tabs (SSH + RDP). No more ServerList panel.
- When no tabs open: welcome screen "Select a server to connect"

### Sidebar Structure

```
QUICK ACCESS
  [icon] All Servers (N)
  [icon] Favorites (N)        [only if any favorites]

GROUPS
  > Group A
    [SSH] server-1  [Connect] [Edit] [Delete]
    [RDP] server-2  [Connect] [Edit] [Delete]
  > Group B
    [SSH] server-3  [Connect]
  [New group input]

UNGROUPED
  [SSH] server-4  [Connect]
```

### Group Node Component
- Click group name → expand/collapse (toggle `expandedGroups` state in sidebar)
- Servers listed under each group, filtered by protocol
- Each server row:
  - Protocol icon (terminal for SSH, monitor for RDP)
  - Server name (truncated)
  - Connect button (play icon) → opens session tab
  - Edit button (pencil) → opens ServerForm modal
  - Delete button (trash) → confirm modal
  - Favorite toggle (star)
- Ungrouped servers shown at bottom under "UNGROUPED" heading

### SearchBar
- Stays in header, filters across ALL servers (not just selected group)
- When search is active, sidebar groups collapse; search results show flat list

### Removed
- `ServerList.tsx` component → deleted (or gutted to minimal)
- Layout flex split (280px server list + terminal) → replaced by single main area

## Part 2: Session Tabs (SSH + RDP)

### Data Model

Rename `TerminalTab` to `SessionTab`:

```typescript
interface SessionTab {
  id: string;
  title: string;
  protocol: 'ssh' | 'rdp';
  serverId: string | null;
  // SSH-specific
  sessionId: string | null;     // ConPTY session ID
  // RDP-specific
  processId: number | null;      // mstsc.exe PID
  // Shared
  status: 'connecting' | 'connected' | 'closed';
}
```

### Store Changes

```typescript
// Rename fields
terminalTabs → sessionTabs
activeTerminalTabId → activeSessionTabId
openTerminalTab → openSession

// New actions
openRdpTab: (server: Server) => Promise<void>
```

### SSH Tab Behavior (unchanged)
- Opens xterm.js → ConPTY session
- Real-time terminal I/O
- Resize via fit()

### RDP Tab Behavior (new)
- Tab opens with status card (no xterm.js)
- Status states:
  - `connecting`: spinner + "Connecting to {server}..."
  - `connected`: green badge + "RDP session active" + "mstsc.exe running" + Reconnect button
  - `disconnected`: yellow badge + "mstsc.exe closed" + Reconnect button
  - `closed`: red badge + "Session closed"
- Connect flow:
  1. Tab opens → status: `connecting`
  2. Backend launches `mstsc.exe` → returns PID
  3. Tab status → `connected`
  4. Background thread polls process; when mstsc exits → status → `disconnected`
- Close tab → kill mstsc.exe if still running

### Backend Changes (RDP)

New command: `cmd_launch_rdp_session` (returns PID)
```rust
#[tauri::command]
pub fn cmd_launch_rdp_session(
    state: State<AppState>,
    host: String,
    username: String,
    fullscreen: bool,
    admin_mode: bool,
    server_id: Option<String>,
    server_name: Option<String>,
    credential_id: Option<String>,
) -> Result<i32, String>  // returns mstsc PID
```

Modified: `cmd_launch_rdp` stays for backward compat (history reconnect, sidebar quick connect)

New command: `cmd_rdp_process_alive`
```rust
#[tauri::command]
pub fn cmd_rdp_process_alive(pid: i32) -> Result<bool, String>
```

### Layout

```
+----------+------------------------------------------+
| Sidebar  |  [tab: ssh] [tab: rdp] [tab: ssh]       |
| (groups  |  +--------------------------------------+|
|  + srvs) |  | Tab content                          ||
|          |  | SSH → xterm.js (ConPTY)               ||
|          |  | RDP → status card + controls           ||
|          |  +--------------------------------------+|
+----------+------------------------------------------+
```

When no tabs: centered welcome message with icon.

## Files to Modify

### Delete
- `src/components/ServerList.tsx` (or replace with thin wrapper)

### Create
- `src/components/GroupServerTree.tsx` — sidebar server tree
- `src/components/RdpSession.tsx` — RDP status tab component

### Modify
- `src/types/index.ts` — rename TerminalTab → SessionTab, add protocol/processId
- `src/store/useStore.ts` — rename fields, add openRdpTab, expandedGroups
- `src/components/Sidebar.tsx` — integrate GroupServerTree
- `src/components/Layout.tsx` — remove ServerList, single main area
- `src/components/Terminal.tsx` — rename props if needed
- `src/services/tauri.ts` — add openRdpSession, rdpProcessAlive
- `src-tauri/src/commands/ssh.rs` — add cmd_launch_rdp_session, cmd_rdp_process_alive
- `src-tauri/src/lib.rs` — register new commands
- `e2e/app.spec.ts` — update tests for new structure

## Testing
- Unit: store actions (openRdpTab, closeSessionTab)
- E2E: all existing tests updated for new sidebar
- Manual: RDP tab lifecycle (connect, detect mstsc close, reconnect)
