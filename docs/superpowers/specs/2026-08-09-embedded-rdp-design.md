# Embedded RDP via WebSocket Relay

## Goal

Replace the external `mstsc.exe` RDP sessions with an embedded HTML5 RDP client rendered inside the app's tab system, matching the SSH terminal experience.

## Scope

Basic RDP: screen display, keyboard input, mouse input. No clipboard, drive redirect, audio, or printer redirection.

## Architecture

```
Frontend (React)                    Rust Backend
┌──────────────────┐               ┌──────────────────────────┐
│  RdpCanvas.tsx   │               │  rdp.rs                  │
│  - WebSocket      │◄──ws://────►│  - WebSocket server       │
│  - <canvas>       │              │  - rdp-rs client          │
│  - Input capture  │              │  - Framebuffer bridge     │
└──────────────────┘               └──────────┬───────────────┘
                                              │ TCP
                                     ┌────────┴────────┐
                                     │  RDP Server      │
                                     │  (Windows Server)│
                                     └─────────────────┘
```

## Data Flow

1. User clicks Connect on an RDP server → `openRdpTab(server)`
2. Frontend calls `api.openRdpSession({host, username, ...})`
3. Rust spawns tokio task:
   a. Resolves password from credential vault (DPAPI decrypt)
   b. Establishes TCP connection to RDP server via `rdp-rs`
   c. Performs RDP handshake (negotiation, licensing, security)
   d. Starts WebSocket server on `127.0.0.1:0` (OS-assigned port)
   e. Returns `{ sessionId, wsPort }` to frontend
4. Frontend receives response, stores `wsPort` in tab state
5. `RdpCanvas` component connects to `ws://127.0.0.1:{wsPort}`
6. Bidirectional bridge:
   - Server → Client: RDP bitmap updates encoded as RGBA frames
   - Client → Server: Mouse/keyboard events encoded as binary messages

## Wire Protocol

All messages are binary, little-endian.

### Server → Client

**Frame update (type 0x01)**:
```
[0x01] [width: u16] [height: u16] [x: u16] [y: u16] [data_len: u32] [rgba: bytes]
```

**Session closed (type 0x02)**:
```
[0x02] [reason_code: u16] [reason_msg: utf8]
```

### Client → Server

**Mouse event (type 0x10)**:
```
[0x10] [x: u16] [y: u16] [buttons: u8] [flags: u8]
```
- `buttons`: bitmask (bit 0=left, bit 1=right, bit 2=middle)
- `flags`: 0=move, 1=click down, 2=click up, 3=wheel

**Keyboard event (type 0x11)**:
```
[0x11] [scan_code: u16] [flags: u8]
```
- `flags`: 0=key down, 1=key up, 2=key release (extended)

**Resize (type 0x12)**:
```
[0x12] [width: u16] [height: u16]
```

## Rust Implementation

### New Dependencies

```toml
# src-tauri/Cargo.toml
rdp-rs = "0.1"
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
```

### New File: `src-tauri/src/rdp.rs`

```rust
pub struct RdpSession {
    pub id: String,
    pub ws_port: u16,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

pub struct RdpSessionManager {
    sessions: Mutex<HashMap<String, RdpSession>>,
}

impl RdpSessionManager {
    pub fn start_session(
        &self,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        width: u16,
        height: u16,
    ) -> Result<(String, u16), String>;

    pub fn stop_session(&self, session_id: &str) -> Result<(), String>;

    pub fn stop_all(&self);
}
```

### Modified: `src-tauri/src/commands/ssh.rs`

Replace:
```rust
#[tauri::command(rename_all = "snake_case")]
pub fn cmd_launch_rdp_session(...) -> Result<i32, String> {
    // old: spawns mstsc.exe, returns PID
}
```

With:
```rust
#[tauri::command(rename_all = "snake_case")]
pub fn cmd_open_rdp_session(
    state: State<AppState>,
    host: String,
    username: String,
    server_id: Option<String>,
    server_name: Option<String>,
    credential_id: Option<String>,
    fullscreen: bool,
    admin_mode: bool,
) -> Result<RdpSessionInfo, String> {
    // 1. Validate host/username
    // 2. Resolve password from credential vault
    // 3. Call rdp_manager.start_session(...)
    // 4. Record history
    // 5. Return { session_id, ws_port }
}
```

### Modified: `src-tauri/src/lib.rs`

- Add `RdpSessionManager` to `AppState`
- Register `rdp_manager` as Tauri managed state
- On `ExitRequested`, call `rdp_manager.stop_all()`

## Frontend Implementation

### New File: `src/components/RdpCanvas.tsx`

```tsx
interface RdpCanvasProps {
  tab: SessionTab;
  active: boolean;
}

export function RdpCanvas({ tab, active }: RdpCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Connect WebSocket on mount
  useEffect(() => {
    if (!tab.wsPort) return;
    const ws = new WebSocket(`ws://127.0.0.1:${tab.wsPort}`);
    ws.binaryType = 'arraybuffer';

    ws.onmessage = (e) => {
      const frame = parseFrame(e.data);
      renderFrame(canvasRef.current, frame);
    };

    ws.onclose = () => { /* update tab status */ };
    wsRef.current = ws;

    return () => ws.close();
  }, [tab.wsPort]);

  // Capture mouse events
  const handleMouseMove = (e) => { /* send to WS */ };
  const handleMouseDown = (e) => { /* send to WS */ };
  const handleMouseUp = (e) => { /* send to WS */ };
  const handleWheel = (e) => { /* send to WS */ };

  // Capture keyboard events
  const handleKeyDown = (e) => { /* send to WS, preventDefault */ };
  const handleKeyUp = (e) => { /* send to WS */ };

  return (
    <canvas
      ref={canvasRef}
      tabIndex={0}
      onMouseMove={handleMouseMove}
      onMouseDown={handleMouseDown}
      onMouseUp={handleMouseUp}
      onWheel={handleWheel}
      onKeyDown={handleKeyDown}
      onKeyUp={handleKeyUp}
      style={{ width: '100%', height: '100%', cursor: 'default', background: '#000' }}
    />
  );
}
```

### Modified: `src/store/useStore.ts`

```ts
// Add wsPort to SessionTab
interface SessionTab {
  // ...existing
  wsPort?: number;
}

// Modify openRdpTab
openRdpTab: async (server) => {
  // ...create tab...
  const result = await api.openRdpSession({ ... });
  // result = { sessionId: string, wsPort: number }
  set({
    sessionTabs: get().sessionTabs.map(t =>
      t.id === tabId ? { ...t, sessionId: result.sessionId, wsPort: result.wsPort, status: 'connected' } : t
    ),
  });
}
```

### Modified: `src/components/Layout.tsx`

Replace:
```tsx
{tab.protocol === 'ssh' ? (
  <Terminal tab={tab} active={tab.id === activeSessionTabId} />
) : (
  <RdpSession tab={tab} />
)}
```

With:
```tsx
{tab.protocol === 'ssh' ? (
  <Terminal tab={tab} active={tab.id === activeSessionTabId} />
) : (
  <RdpCanvas tab={tab} active={tab.id === activeSessionTabId} />
)}
```

### Deleted: `src/components/RdpSession.tsx`

No longer needed — replaced by `RdpCanvas.tsx`.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| RDP connection refused | Tab status → 'closed', notification "Connection refused" |
| Invalid credentials | Tab status → 'closed', notification "Authentication failed" |
| WebSocket disconnect | Auto-reconnect 3× with 1s delay, then close tab |
| RDP protocol error | Tab status → 'closed', notification with error details |
| Canvas resize | Send resize event to RDP server, server sends new framebuffer |

## Files Changed

| File | Action | Description |
|------|--------|-------------|
| `src-tauri/Cargo.toml` | Modify | Add rdp-rs, tokio, tokio-tungstenite |
| `src-tauri/src/rdp.rs` | **New** | RDP session manager + WebSocket relay |
| `src-tauri/src/lib.rs` | Modify | Register RdpSessionManager, add cleanup |
| `src-tauri/src/commands/ssh.rs` | Modify | Replace cmd_launch_rdp_session with cmd_open_rdp_session |
| `src/components/RdpCanvas.tsx` | **New** | Canvas-based RDP renderer |
| `src/components/RdpSession.tsx` | **Delete** | Replaced by RdpCanvas |
| `src/components/Layout.tsx` | Modify | Use RdpCanvas instead of RdpSession |
| `src/store/useStore.ts` | Modify | Add wsPort to tab, update openRdpTab |
| `src/services/tauri.ts` | Modify | Update openRdpSession return type |
| `src/types/index.ts` | Modify | Add wsPort to SessionTab |
