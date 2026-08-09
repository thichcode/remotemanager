# Embedded RDP via WebSocket Relay — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace external `mstsc.exe` RDP sessions with an embedded HTML5 RDP client rendered inside the app's tab system.

**Architecture:** Rust backend uses `rdp-rs` crate to connect to RDP servers via TCP, bridges framebuffer data to a local WebSocket server. Frontend connects via WebSocket, renders frames to `<canvas>`, and sends mouse/keyboard events back.

**Tech Stack:** rdp-rs, tokio, tokio-tungstenite (Rust); React, canvas API (TypeScript)

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/Cargo.toml` | Modify | Add rdp-rs, tokio, tokio-tungstenite deps |
| `src-tauri/src/rdp.rs` | **Create** | RDP session manager, WebSocket relay, frame encoding |
| `src-tauri/src/rdp/frame.rs` | **Create** | Frame parsing/encoding for wire protocol |
| `src-tauri/src/rdp/input.rs` | **Create** | Mouse/keyboard event encoding |
| `src-tauri/src/commands/ssh.rs` | Modify | Replace cmd_launch_rdp_session → cmd_open_rdp_session |
| `src-tauri/src/commands/mod.rs` | Modify | Add `pub mod rdp` |
| `src-tauri/src/lib.rs` | Modify | Register RdpSessionManager in AppState |
| `src/components/RdpCanvas.tsx` | **Create** | Canvas-based RDP renderer + WebSocket client |
| `src/components/RdpSession.tsx` | **Delete** | Replaced by RdpCanvas |
| `src/components/Layout.tsx` | Modify | Import RdpCanvas instead of RdpSession |
| `src/store/useStore.ts` | Modify | Add wsPort to tab, update openRdpTab |
| `src/services/tauri.ts` | Modify | Update openRdpSession return type |
| `src/types/index.ts` | Modify | Add wsPort to SessionTab |

---

### Task 1: Add Rust Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add new dependencies to Cargo.toml**

```toml
[dependencies]
# ... existing deps ...
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
```

Note: `rdp-rs` will be added in Task 3 after we verify the API. For now, add tokio and tungstenite.

- [ ] **Step 2: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: PASS (new deps compile)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add tokio and tokio-tungstenite dependencies"
```

---

### Task 2: Create Wire Protocol Types

**Files:**
- Create: `src-tauri/src/rdp/frame.rs`
- Create: `src-tauri/src/rdp/input.rs`
- Create: `src-tauri/src/rdp/mod.rs`

- [ ] **Step 1: Create `src-tauri/src/rdp/mod.rs`**

```rust
pub mod frame;
pub mod input;

use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::oneshot;

pub struct RdpSessionHandle {
    pub id: String,
    pub ws_port: u16,
    pub shutdown_tx: oneshot::Sender<()>,
}

pub struct RdpSessionManager {
    sessions: Mutex<HashMap<String, RdpSessionHandle>>,
}

impl RdpSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&self, id: String, handle: RdpSessionHandle) {
        self.sessions.lock().unwrap().insert(id, handle);
    }

    pub fn remove(&self, id: &str) -> Option<RdpSessionHandle> {
        self.sessions.lock().unwrap().remove(id)
    }

    pub fn stop_all(&self) {
        let sessions = self.sessions.lock().unwrap();
        for (_, handle) in sessions.iter() {
            let _ = handle.shutdown_tx.send(());
        }
    }
}
```

- [ ] **Step 2: Create `src-tauri/src/rdp/frame.rs`**

```rust
/// Wire protocol: Server → Client frame update
///
/// [0x01] [width: u16 LE] [height: u16 LE] [x: u16 LE] [y: u16 LE] [data_len: u32 LE] [rgba: bytes]
pub const MSG_FRAME: u8 = 0x01;
pub const MSG_SESSION_CLOSED: u8 = 0x02;

pub struct FrameUpdate {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

impl FrameUpdate {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(11 + self.rgba.len());
        buf.push(MSG_FRAME);
        buf.extend_from_slice(&self.width.to_le_bytes());
        buf.extend_from_slice(&self.height.to_le_bytes());
        buf.extend_from_slice(&self.x.to_le_bytes());
        buf.extend_from_slice(&self.y.to_le_bytes());
        buf.extend_from_slice(&(self.rgba.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.rgba);
        buf
    }
}

pub fn encode_session_closed(reason: &str) -> Vec<u8> {
    let reason_bytes = reason.as_bytes();
    let mut buf = Vec::with_capacity(3 + reason_bytes.len());
    buf.push(MSG_SESSION_CLOSED);
    buf.extend_from_slice(&(reason_bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(reason_bytes);
    buf
}
```

- [ ] **Step 3: Create `src-tauri/src/rdp/input.rs`**

```rust
/// Wire protocol: Client → Server input events
///
/// Mouse:  [0x10] [x: u16 LE] [y: u16 LE] [buttons: u8] [flags: u8]
/// Keyboard: [0x11] [scan_code: u16 LE] [flags: u8]
/// Resize: [0x12] [width: u16 LE] [height: u16 LE]
pub const MSG_MOUSE: u8 = 0x10;
pub const MSG_KEYBOARD: u8 = 0x11;
pub const MSG_RESIZE: u8 = 0x12;

pub enum ClientMessage {
    Mouse {
        x: u16,
        y: u16,
        buttons: u8,
        flags: u8,
    },
    Keyboard {
        scan_code: u16,
        flags: u8,
    },
    Resize {
        width: u16,
        height: u16,
    },
}

impl ClientMessage {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }
        match data[0] {
            MSG_MOUSE if data.len() >= 6 => Some(ClientMessage::Mouse {
                x: u16::from_le_bytes([data[1], data[2]]),
                y: u16::from_le_bytes([data[3], data[4]]),
                buttons: data[5],
                flags: if data.len() > 6 { data[6] } else { 0 },
            }),
            MSG_KEYBOARD if data.len() >= 4 => Some(ClientMessage::Keyboard {
                scan_code: u16::from_le_bytes([data[1], data[2]]),
                flags: data[3],
            }),
            MSG_RESIZE if data.len() >= 5 => Some(ClientMessage::Resize {
                width: u16::from_le_bytes([data[1], data[2]]),
                height: u16::from_le_bytes([data[3], data[4]]),
            }),
            _ => None,
        }
    }
}
```

- [ ] **Step 4: Register module in lib.rs**

Add to `src-tauri/src/lib.rs`:
```rust
mod rdp;
```

- [ ] **Step 5: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/rdp/
git commit -m "feat: add RDP wire protocol types and session manager"
```

---

### Task 3: Implement RDP Session with rdp-rs

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/rdp/mod.rs`

- [ ] **Step 1: Add rdp-rs dependency**

```toml
[dependencies]
# ... existing ...
rdp-rs = { version = "0.1", features = [] }
```

- [ ] **Step 2: Implement RDP connection + WS relay in `rdp/mod.rs`**

Replace the `RdpSessionManager` with full implementation. This is the core module — it:
1. Connects to RDP server via `rdp-rs`
2. Starts a WebSocket server on random port
3. Bridges RDP framebuffer → WebSocket
4. Bridges WebSocket input → RDP

Key function signature:

```rust
pub async fn start_session(
    manager: std::sync::Arc<RdpSessionManager>,
    host: String,
    port: u16,
    username: String,
    password: String,
    width: u16,
    height: u16,
) -> Result<(String, u16), String>
```

This function:
1. Generates session ID (UUID)
2. Creates `oneshot` channel for shutdown
3. Spawns a tokio task that:
   a. Connects via `rdp_rs::client::connect(...)` 
   b. Starts `tokio_tungstenite::WebSocketListener` on `127.0.0.1:0`
   c. Gets the assigned port
   d. Loops: accept WS connections, bridge to RDP
4. Registers session in manager
5. Returns `(session_id, ws_port)`

Note: The actual `rdp-rs` API needs to be verified. If `rdp-rs` doesn't work well, fall back to a simpler approach: run `wfreerdp.exe` (FreeRDP Windows build) as a subprocess and bridge its framebuffer output via pipe.

- [ ] **Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/rdp/mod.rs
git commit -m "feat: implement RDP session with WebSocket relay"
```

---

### Task 4: Update Tauri Commands

**Files:**
- Modify: `src-tauri/src/commands/ssh.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `cmd_open_rdp_session` to `commands/ssh.rs`**

Add after the existing SSH session commands:

```rust
#[derive(serde::Serialize, Clone)]
pub struct RdpSessionInfo {
    pub session_id: String,
    pub ws_port: u16,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_open_rdp_session(
    state: tauri::State<'_, crate::db::AppState>,
    host: String,
    username: String,
    server_id: Option<String>,
    server_name: Option<String>,
    credential_id: Option<String>,
    fullscreen: bool,
    _admin_mode: bool,
) -> Result<RdpSessionInfo, String> {
    use crate::security::input::{validate_host, validate_username};

    validate_host(&host)?;
    let username = resolve_username(&state, username, credential_id.as_deref())?;
    validate_username(&username)?;

    // Resolve password from credential vault
    let password = if let Some(cid) = credential_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let encrypted = crate::db::operations::get_credential_password(&conn, cid)
            .map_err(|e| e.to_string())?
            .ok_or("Credential not found")?;
        crate::security::decrypt(&encrypted).map_err(|e| e.to_string())?
    } else {
        String::new()
    };

    // Record history
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let name = server_name.unwrap_or_else(|| host.clone());
        let _ = crate::history::record(&conn, server_id.as_deref(), &name, &host, Some(3389), "rdp", &username, None);
    }

    // Start RDP session
    let (session_id, ws_port) = crate::rdp::start_session(
        state.rdp_manager.clone(),
        host,
        3389,
        username,
        password,
        1024,
        768,
    ).await?;

    Ok(RdpSessionInfo { session_id, ws_port })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cmd_close_rdp_session(
    state: tauri::State<'_, crate::db::AppState>,
    session_id: String,
) -> Result<(), String> {
    crate::rdp::stop_session(&state.rdp_manager, &session_id)
}
```

- [ ] **Step 2: Update `AppState` in `lib.rs`**

```rust
pub struct AppState {
    pub db: std::sync::Mutex<Connection>,
    pub sessions: std::sync::Arc<crate::sessions::SessionManager>,
    pub rdp_manager: std::sync::Arc<crate::rdp::RdpSessionManager>,
}
```

Initialize in `run()`:
```rust
let rdp_manager = std::sync::Arc::new(crate::rdp::RdpSessionManager::new());
let state = AppState {
    db: std::sync::Mutex::new(conn),
    sessions: std::sync::Arc::new(crate::sessions::SessionManager::new()),
    rdp_manager,
};
```

Register commands:
```rust
commands::ssh::cmd_open_rdp_session,
commands::ssh::cmd_close_rdp_session,
```

Add cleanup:
```rust
app.run(|app_handle, event| {
    if let tauri::RunEvent::ExitRequested { .. } = event {
        if let Some(state) = app_handle.try_state::<AppState>() {
            let _ = crate::commands::sessions::cmd_ssh_close_all(state.clone());
            state.rdp_manager.stop_all();
        }
    }
});
```

- [ ] **Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/ssh.rs src-tauri/src/lib.rs
git commit -m "feat: add cmd_open_rdp_session and cmd_close_rdp_session"
```

---

### Task 5: Update TypeScript Types and API

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/services/tauri.ts`

- [ ] **Step 1: Add wsPort to SessionTab**

In `src/types/index.ts`, update `SessionTab`:
```ts
export interface SessionTab {
  id: string;
  title: string;
  protocol: 'ssh' | 'rdp';
  serverId: string | null;
  sessionId: string | null;
  processId: number | null;
  wsPort?: number;
  status: 'connecting' | 'connected' | 'closed';
}
```

- [ ] **Step 2: Update API functions**

In `src/services/tauri.ts`, replace:
```ts
// Old
export const openRdpSession = (args: { ... }): Promise<number> =>
  invoke('cmd_launch_rdp_session', { ... });

export const rdpProcessAlive = (pid: number): Promise<boolean> =>
  invoke('cmd_rdp_process_alive', { pid });

export const rdpKillProcess = (pid: number): Promise<void> =>
  invoke('cmd_rdp_kill_process', { pid });
```

With:
```ts
export const openRdpSession = (args: {
  host: string;
  username: string;
  fullscreen: boolean;
  adminMode: boolean;
  serverId?: string | null;
  serverName?: string | null;
  credentialId?: string | null;
}): Promise<{ session_id: string; ws_port: number }> =>
  invoke('cmd_open_rdp_session', {
    host: args.host,
    username: args.username,
    fullscreen: args.fullscreen,
    admin_mode: args.adminMode,
    server_id: args.serverId ?? null,
    server_name: args.serverName ?? null,
    credential_id: args.credentialId ?? null,
  });

export const closeRdpSession = (sessionId: string): Promise<void> =>
  invoke('cmd_close_rdp_session', { session_id: sessionId });
```

Remove `rdpProcessAlive` and `rdpKillProcess` (no longer needed).

- [ ] **Step 3: Verify typecheck**

Run: `npx tsc --noEmit`
Expected: PASS (will show errors until store is updated — that's fine, next task fixes it)

- [ ] **Step 4: Commit**

```bash
git add src/types/index.ts src/services/tauri.ts
git commit -m "feat: update TypeScript types and API for embedded RDP"
```

---

### Task 6: Update Store (openRdpTab)

**Files:**
- Modify: `src/store/useStore.ts`

- [ ] **Step 1: Update openRdpTab to use new API**

Replace the `openRdpTab` action:

```ts
openRdpTab: async (server) => {
  const tabId = crypto.randomUUID();
  set({
    sessionTabs: [
      ...get().sessionTabs,
      { id: tabId, title: server.name, protocol: 'rdp' as const, serverId: server.id, sessionId: null, processId: null, wsPort: undefined, status: 'connecting' as const },
    ],
    activeSessionTabId: tabId,
  });
  try {
    const result = await api.openRdpSession({
      host: server.host,
      username: server.username,
      fullscreen: get().settings?.rdp_fullscreen ?? false,
      adminMode: get().settings?.rdp_admin_mode ?? false,
      serverId: server.id,
      serverName: server.name,
      credentialId: server.credential_id,
    });
    if (!get().sessionTabs.some(t => t.id === tabId)) {
      try { await api.closeRdpSession(result.session_id); } catch {}
      return;
    }
    set({
      sessionTabs: get().sessionTabs.map(t =>
        t.id === tabId ? { ...t, sessionId: result.session_id, wsPort: result.ws_port, status: 'connected' } : t
      ),
    });
  } catch (e) {
    set({
      sessionTabs: get().sessionTabs.map(t =>
        t.id === tabId ? { ...t, status: 'closed' } : t
      ),
    });
    throw e;
  }
},
```

- [ ] **Step 2: Update closeSessionTab for RDP**

Replace the RDP cleanup in `closeSessionTab`:

```ts
closeSessionTab: async (id) => {
  const tab = get().sessionTabs.find(t => t.id === id);
  const remaining = get().sessionTabs.filter(t => t.id !== id);
  set({
    sessionTabs: remaining,
    activeSessionTabId:
      get().activeSessionTabId === id
        ? remaining.length > 0 ? remaining[0].id : null
        : get().activeSessionTabId,
  });
  if (tab?.protocol === 'ssh' && tab.sessionId) {
    try { await api.sshClose(tab.sessionId); } catch {}
  }
  if (tab?.protocol === 'rdp' && tab.sessionId) {
    try { await api.closeRdpSession(tab.sessionId); } catch {}
  }
},
```

- [ ] **Step 3: Remove _startRdpPoll**

Delete the `_startRdpPoll` action entirely (RDP is now WebSocket-based, no polling needed).

- [ ] **Step 4: Verify typecheck**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/store/useStore.ts
git commit -m "feat: update store for embedded RDP sessions"
```

---

### Task 7: Create RdpCanvas Component

**Files:**
- Create: `src/components/RdpCanvas.tsx`

- [ ] **Step 1: Create `src/components/RdpCanvas.tsx`**

```tsx
import { useEffect, useRef, useCallback } from 'react';
import { Stack, Text } from '@mantine/core';
import { useStore } from '../store/useStore';
import type { SessionTab } from '../types';

const MSG_FRAME = 0x01;
const MSG_SESSION_CLOSED = 0x02;
const MSG_MOUSE = 0x10;
const MSG_KEYBOARD = 0x11;
const MSG_RESIZE = 0x12;

interface RdpCanvasProps {
  tab: SessionTab;
  active: boolean;
}

function parseFrame(buffer: ArrayBuffer): { x: number; y: number; width: number; height: number; rgba: Uint8Array } | null {
  const view = new DataView(buffer);
  if (view.byteLength < 11 || view.getUint8(0) !== MSG_FRAME) return null;
  const width = view.getUint16(1, true);
  const height = view.getUint16(3, true);
  const x = view.getUint16(5, true);
  const y = view.getUint16(7, true);
  const dataLen = view.getUint32(9, true);
  const rgba = new Uint8Array(buffer, 11, dataLen);
  return { x, y, width, height, rgba };
}

export function RdpCanvas({ tab, active }: RdpCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const closeSessionTab = useStore((s) => s.closeSessionTab);

  useEffect(() => {
    if (!tab.wsPort || tab.status !== 'connected') return;

    const ws = new WebSocket(`ws://127.0.0.1:${tab.wsPort}`);
    ws.binaryType = 'arraybuffer';

    ws.onmessage = (e) => {
      const data = e.data as ArrayBuffer;
      const view = new DataView(data);
      if (view.byteLength < 1) return;

      const msgType = view.getUint8(0);

      if (msgType === MSG_FRAME) {
        const frame = parseFrame(data);
        if (!frame) return;
        const canvas = canvasRef.current;
        if (!canvas) return;

        if (canvas.width !== frame.width || canvas.height !== frame.height) {
          canvas.width = frame.width;
          canvas.height = frame.height;
        }

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        const imageData = new ImageData(
          new Uint8ClampedArray(frame.rgba.buffer, frame.rgba.byteOffset, frame.rgba.byteLength),
          frame.width,
          frame.height,
        );
        ctx.putImageData(imageData, frame.x, frame.y);
      } else if (msgType === MSG_SESSION_CLOSED) {
        const reasonLen = view.getUint16(1, true);
        const reason = new TextDecoder().decode(new Uint8Array(data, 3, reasonLen));
        console.error('RDP session closed:', reason);
        closeSessionTab(tab.id);
      }
    };

    ws.onerror = (e) => {
      console.error('RDP WebSocket error:', e);
    };

    ws.onclose = () => {
      if (tab.status === 'connected') {
        closeSessionTab(tab.id);
      }
    };

    wsRef.current = ws;

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [tab.wsPort, tab.status, tab.id, closeSessionTab]);

  // Capture mouse events
  const sendMouse = useCallback((x: number, y: number, buttons: number, flags: number) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const buf = new ArrayBuffer(7);
    const view = new DataView(buf);
    view.setUint8(0, MSG_MOUSE);
    view.setUint16(1, x, true);
    view.setUint16(3, y, true);
    view.setUint8(5, buttons);
    view.setUint8(6, flags);
    ws.send(buf);
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = Math.floor((e.clientX - rect.left) * scaleX);
    const y = Math.floor((e.clientY - rect.top) * scaleY);
    sendMouse(x, y, 0, 0);
  }, [sendMouse]);

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = Math.floor((e.clientX - rect.left) * scaleX);
    const y = Math.floor((e.clientY - rect.top) * scaleY);
    const buttons = e.button === 0 ? 1 : e.button === 1 ? 4 : e.button === 2 ? 2 : 0;
    sendMouse(x, y, buttons, 1);
  }, [sendMouse]);

  const handleMouseUp = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = Math.floor((e.clientX - rect.left) * scaleX);
    const y = Math.floor((e.clientY - rect.top) * scaleY);
    const buttons = e.button === 0 ? 1 : e.button === 1 ? 4 : e.button === 2 ? 2 : 0;
    sendMouse(x, y, buttons, 2);
  }, [sendMouse]);

  const handleWheel = useCallback((e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    const x = Math.floor((e.clientX - rect.left) * scaleX);
    const y = Math.floor((e.clientY - rect.top) * scaleY);
    const buttons = e.deltaY < 0 ? 1 : 2;
    sendMouse(x, y, buttons, 3);
  }, [sendMouse]);

  // Capture keyboard events
  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const buf = new ArrayBuffer(4);
    const view = new DataView(buf);
    view.setUint8(0, MSG_KEYBOARD);
    view.setUint16(1, e.keyCode, true);
    view.setUint8(3, 0); // key down
    ws.send(buf);
  }, []);

  const handleKeyUp = useCallback((e: React.KeyboardEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const buf = new ArrayBuffer(4);
    const view = new DataView(buf);
    view.setUint8(0, MSG_KEYBOARD);
    view.setUint16(1, e.keyCode, true);
    view.setUint8(3, 1); // key up
    ws.send(buf);
  }, []);

  if (tab.status === 'closed') {
    return (
      <Stack align="center" justify="center" h="100%">
        <Text c="dimmed">RDP session closed.</Text>
      </Stack>
    );
  }

  return (
    <div ref={containerRef} style={{ height: '100%', width: '100%', overflow: 'hidden', background: '#000' }}>
      <canvas
        ref={canvasRef}
        tabIndex={0}
        onMouseMove={handleMouseMove}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onWheel={handleWheel}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        onContextMenu={(e) => e.preventDefault()}
        style={{
          width: '100%',
          height: '100%',
          cursor: 'default',
          outline: 'none',
        }}
      />
    </div>
  );
}
```

- [ ] **Step 2: Verify typecheck**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/components/RdpCanvas.tsx
git commit -m "feat: add RdpCanvas embedded RDP renderer component"
```

---

### Task 8: Wire Up Layout and Store

**Files:**
- Modify: `src/components/Layout.tsx`
- Modify: `src/store/useStore.ts`

- [ ] **Step 1: Update Layout.tsx**

Replace import:
```tsx
// Old
import { RdpSession } from './RdpSession';

// New
import { RdpCanvas } from './RdpCanvas';
```

Replace usage:
```tsx
// Old
<RdpSession tab={tab} />

// New
<RdpCanvas tab={tab} active={tab.id === activeSessionTabId} />
```

- [ ] **Step 2: Remove dead code from store**

Remove from `useStore.ts`:
- `_startRdpPoll` action
- Any references to `rdpProcessAlive` or `rdpKillProcess`

- [ ] **Step 3: Verify typecheck**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/components/Layout.tsx src/store/useStore.ts
git commit -m "feat: wire RdpCanvas into Layout, clean up store"
```

---

### Task 9: Delete Old RDP Component

**Files:**
- Delete: `src/components/RdpSession.tsx`

- [ ] **Step 1: Delete the file**

```bash
rm src/components/RdpSession.tsx
```

- [ ] **Step 2: Verify no remaining imports**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add -A src/components/RdpSession.tsx
git commit -m "chore: remove old RdpSession component"
```

---

### Task 10: Full Build Verification

**Files:** None (verification only)

- [ ] **Step 1: TypeScript check**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 2: Rust check**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Build frontend**

Run: `npm run build`
Expected: PASS

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: embedded RDP via WebSocket relay — complete"
```

---

### Task 11: Version Bump and Push

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Bump version to 0.4.0** (new feature = minor bump)

Update all three files from `0.3.15` → `0.4.0`.

- [ ] **Step 2: Commit, tag, and push**

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.4.0"
git push
```

- [ ] **Step 3: Verify release workflow triggers**

Check GitHub Actions — the "Auto Release" workflow should trigger and build v0.4.0.
