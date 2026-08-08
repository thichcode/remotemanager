# Embedded SSH Terminal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Embed SSH sessions inside the Remote Manager app as terminal tabs, keeping the existing "open in external terminal" (wt.exe) flow as a secondary option.

**Architecture:** A Rust `SessionManager` owns one spawned `ssh.exe` process per session, streaming stdout/stderr to the frontend via Tauri events (`ssh://output`, `ssh://exit`) and accepting input/resize/close through commands. The frontend renders each session with xterm.js inside a Mantine tab bar in the Servers view (split layout: server list left, terminal section right). Sessions stay alive while switching views (components stay mounted, hidden by CSS) and are killed on tab close or app exit.

**Tech Stack:** Rust (tauri v2, std::process, uuid), TypeScript/React, `@xterm/xterm` + `@xterm/addon-fit` (already in package.json), Mantine `Tabs`, Playwright e2e with existing Tauri mock.

---

### Task 1: Rust — `SessionManager` module with unit tests

**Files:**
- Create: `src-tauri/src/sessions.rs`
- Test: inline `#[cfg(test)]` module in `src-tauri/src/sessions.rs`

- [ ] **Step 1: Write the failing unit tests for the session map**

```rust
// src-tauri/src/sessions.rs
use std::collections::HashMap;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Mutex;
use uuid::Uuid;

pub struct SessionManager {
    pub sessions: Mutex<HashMap<String, Child>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, id: String, child: Child) {
        self.sessions.lock().unwrap().insert(id, child);
    }

    pub fn remove(&self, id: &str) -> Option<Child> {
        self.sessions.lock().unwrap().remove(id)
    }

    pub fn get_mut(&self, id: &str) -> Option<std::sync::MutexGuard<'_, HashMap<String, Child>>> {
        let g = self.sessions.lock().unwrap();
        g.get_mut(id)?;
        Some(g)
    }

    pub fn len(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_child() -> Child {
        Command::new("ping")
            .arg("127.0.0.1")
            .arg("-n")
            .arg("10")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn ping")
    }

    #[test]
    fn insert_and_len() {
        let m = SessionManager::new();
        assert_eq!(m.len(), 0);
        m.insert("a".into(), dummy_child());
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn remove_returns_child() {
        let m = SessionManager::new();
        m.insert("a".into(), dummy_child());
        let child = m.remove("a");
        assert!(child.is_some());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn remove_missing_returns_none() {
        let m = SessionManager::new();
        assert!(m.remove("nope").is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (module not registered yet)**

Run: `cargo test` in `src-tauri`
Expected: compile error `unresolved module sessions` (module not declared in lib.rs yet — add `mod sessions;` to `src-tauri/src/lib.rs` first, then re-run; the tests will compile but the empty impl will fail because the methods don't exist yet — actually write the full impl above and just verify they pass).

Note: implement the full `SessionManager` above (it already contains the passing implementation) and verify:

Run: `cargo test sessions::tests -- --nocapture` in `src-tauri`
Expected: `test result: ok. 3 passed; 0 failed`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/sessions.rs src-tauri/src/lib.rs
git commit -m "feat: add SessionManager for embedded ssh sessions"
```

---

### Task 2: Rust — SSH session commands

**Files:**
- Modify: `src-tauri/src/commands/sessions.rs` (new file)
- Modify: `src-tauri/src/commands/mod.rs` (add `pub mod sessions;`)
- Modify: `src-tauri/src/db/mod.rs` (AppState holds `sessions: Arc<SessionManager>`)
- Modify: `src-tauri/src/lib.rs` (add `mod sessions;` — already added in Task 1; manage state; register commands; cleanup on exit)

**Design note:** The spawned `Child` stays in the `SessionManager` map so `cmd_ssh_write` can reach `child.stdin`. stdout/stderr handles are extracted at spawn and moved into reader threads. A monitor thread polls `try_wait()` and emits `ssh://exit` when the process exits, then removes the session. `AppState.sessions` is an `Arc<SessionManager>` so the monitor thread can hold a clone.

- [ ] **Step 1: Make `resolve_username` reusable**

In `src-tauri/src/commands/ssh.rs`, change the private function to `pub(crate)`:

```rust
pub(crate) fn resolve_username(
    state: &tauri::State<crate::db::AppState>,
    username: String,
    credential_id: Option<&str>,
) -> Result<String, String> {
```

- [ ] **Step 2: Add `sessions: Arc<SessionManager>` to AppState**

Modify `src-tauri/src/db/mod.rs`:

```rust
pub struct AppState {
    pub db: Mutex<Connection>,
    pub sessions: Arc<crate::sessions::SessionManager>,
}
```

Update imports at top of `src-tauri/src/db/mod.rs`:

```rust
use std::sync::{Arc, Mutex};
```

- [ ] **Step 3: Write the session commands**

```rust
// src-tauri/src/commands/sessions.rs
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{Emitter, State};
use uuid::Uuid;
use crate::db::AppState;

fn spawn_reader_thread(mut reader: impl Read + Send + 'static, app: tauri::AppHandle, sid: String) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    let _ = app.emit("ssh://output", serde_json::json!({ "sessionId": sid, "data": data }));
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_exit_monitor(app: tauri::AppHandle, sid: String, sessions: std::sync::Arc<crate::sessions::SessionManager>) {
    std::thread::spawn(move || {
        loop {
            let status = {
                let mut guard = sessions.sessions.lock().unwrap();
                let Some(child) = guard.get_mut(&sid) else { break };
                child.try_wait().ok().flatten()
            };
            match status {
                Some(status) => {
                    let code = status.code().unwrap_or(-1);
                    let _ = app.emit("ssh://exit", serde_json::json!({ "sessionId": sid, "code": code }));
                    if let Some(mut child) = sessions.remove(&sid) {
                        let _ = child.wait();
                    }
                    break;
                }
                None => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    });
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_open_ssh_session(
    app: tauri::AppHandle,
    state: State<AppState>,
    host: String,
    port: i32,
    username: String,
    server_id: Option<String>,
    server_name: Option<String>,
    ssh_key_id: Option<String>,
    credential_id: Option<String>,
) -> Result<String, String> {
    crate::security::input::validate_host(&host)?;
    if port < 1 || port > 65535 {
        return Err("Port must be between 1 and 65535".to_string());
    }

    let username = crate::commands::ssh::resolve_username(&state, username, credential_id.as_deref())?;
    crate::security::input::validate_username(&username)?;

    let mut extra_args: Vec<String> = Vec::new();
    if let Some(kid) = ssh_key_id.as_deref() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        if let Some(key_path) = crate::sshkeys::get_private_key_path(&conn, kid)? {
            crate::sshkeys::ensure_key_permissions(&key_path);
            extra_args.push("-i".to_string());
            extra_args.push(key_path);
        }
    }

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let name = server_name.unwrap_or_else(|| host.clone());
        let _ = crate::history::record(&conn, server_id.as_deref(), &name, &host, Some(port), "ssh", &username, ssh_key_id.as_deref());
        if let Some(sid) = server_id.as_deref() {
            let _ = crate::db::operations::touch_last_connected(&conn, sid);
        }
    }

    let mut cmd = Command::new("ssh");
    cmd.args(&extra_args);
    cmd.arg("-o").arg("IdentitiesOnly=yes");
    cmd.args(["-p", &port.to_string()]);
    cmd.arg("-t");
    cmd.arg(format!("{}@{}", username, host));
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to launch SSH: {}", e))?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let session_id = Uuid::new_v4().to_string();

    state.sessions.insert(session_id.clone(), child);
    spawn_reader_thread(stdout, app.clone(), session_id.clone());
    spawn_reader_thread(stderr, app.clone(), session_id.clone());
    spawn_exit_monitor(app, session_id.clone(), state.sessions.clone());

    Ok(session_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_write(state: State<AppState>, session_id: String, data: Vec<u8>) -> Result<(), String> {
    use std::io::Write;
    let mut guard = state.sessions.sessions.lock().map_err(|e| e.to_string())?;
    let child = guard.get_mut(&session_id).ok_or("Session not found")?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(&data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_resize(state: State<AppState>, session_id: String, cols: i32, rows: i32) -> Result<(), String> {
    // Best-effort: no real PTY on Windows spawn; size is kept consistent by
    // xterm fit() on the frontend. Reserved for future CONPTY integration.
    let _ = (state, session_id, cols, rows);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_close(state: State<AppState>, session_id: String) -> Result<(), String> {
    if let Some(mut child) = state.sessions.remove(&session_id) {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn cmd_ssh_close_all(state: State<AppState>) -> Result<(), String> {
    let ids: Vec<String> = {
        let guard = state.sessions.sessions.lock().map_err(|e| e.to_string())?;
        guard.keys().cloned().collect()
    };
    for id in ids {
        if let Some(mut child) = state.sessions.remove(&id) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Register module and commands, manage state, cleanup on exit**

Modify `src-tauri/src/commands/mod.rs`:

```rust
pub mod sessions;
```

Modify `src-tauri/src/lib.rs` — AppState construction now needs the sessions manager:

```rust
let state = AppState {
    db: std::sync::Mutex::new(conn),
    sessions: std::sync::Arc::new(crate::sessions::SessionManager::new()),
};
```

Register the new commands in the `invoke_handler`:

```rust
commands::sessions::cmd_open_ssh_session,
commands::sessions::cmd_ssh_write,
commands::sessions::cmd_ssh_resize,
commands::sessions::cmd_ssh_close,
commands::sessions::cmd_ssh_close_all,
```

Cleanup on exit — replace `.run(...)` with `.build(...)` + `.run(...)` callback:

```rust
let app = tauri::Builder::default()
    .manage(state)
    // ... plugins and invoke_handler unchanged ...
    .build(tauri::generate_context!())
    .expect("error while building tauri application");

app.run(|app_handle, event| {
    if let tauri::RunEvent::ExitRequested { .. } = event {
        if let Some(state) = app_handle.try_state::<AppState>() {
            let _ = crate::commands::sessions::cmd_ssh_close_all(state);
        }
    }
});
```

- [ ] **Step 5: Add a unit test for write/close behavior**

In `src-tauri/src/sessions.rs` tests, add:

```rust
#[test]
fn write_and_close_lifecycle() {
    let m = SessionManager::new();
    m.insert("a".into(), dummy_child());
    {
        use std::io::Write;
        let mut guard = m.sessions.lock().unwrap();
        if let Some(c) = guard.get_mut("a") {
            if let Some(stdin) = c.stdin.as_mut() {
                let _ = stdin.write_all(b"hello\n");
            }
        }
    }
    assert_eq!(m.len(), 1);
    let mut child = m.remove("a").unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(m.len(), 0);
}
```

- [ ] **Step 6: Compile and run all Rust tests**

Run: `cargo test` in `src-tauri`
Expected: compile success; all tests pass (existing + new session tests)

- [ ] **Step 7: Commit**

```bash
git add src-tauri
git commit -m "feat: add ssh session commands (open/write/resize/close) with events"
```

---

### Task 3: Frontend — service wrappers and store state

**Files:**
- Modify: `src/services/tauri.ts`
- Modify: `src/store/useStore.ts`
- Modify: `src/types/index.ts`

- [ ] **Step 1: Add service wrappers**

In `src/services/tauri.ts`, add:

```ts
// SSH Sessions
export const openSshSession = (args: {
  host: string;
  port: number;
  username: string;
  serverId?: string | null;
  serverName?: string | null;
  sshKeyId?: string | null;
  credentialId?: string | null;
}): Promise<string> =>
  invoke('cmd_open_ssh_session', {
    host: args.host,
    port: args.port,
    username: args.username,
    server_id: args.serverId ?? null,
    server_name: args.serverName ?? null,
    ssh_key_id: args.sshKeyId ?? null,
    credential_id: args.credentialId ?? null,
  });

export const sshWrite = (sessionId: string, data: number[]): Promise<void> =>
  invoke('cmd_ssh_write', { session_id: sessionId, data });

export const sshResize = (sessionId: string, cols: number, rows: number): Promise<void> =>
  invoke('cmd_ssh_resize', { session_id: sessionId, cols, rows });

export const sshClose = (sessionId: string): Promise<void> =>
  invoke('cmd_ssh_close', { session_id: sessionId });

export const sshCloseAll = (): Promise<void> =>
  invoke('cmd_ssh_close_all');
```

- [ ] **Step 2: Add TerminalTab type**

In `src/types/index.ts`:

```ts
export interface TerminalTab {
  id: string;
  title: string;
  serverId: string | null;
  sessionId: string | null;
  status: 'connecting' | 'connected' | 'closed';
}
```

- [ ] **Step 3: Add store state and actions**

In `src/store/useStore.ts`:

Add to the `AppState` interface:

```ts
  terminalTabs: TerminalTab[];
  activeTerminalTabId: string | null;
  openTerminalTab: (server: Server) => Promise<void>;
  closeTerminalTab: (id: string) => Promise<void>;
  focusTerminalTab: (id: string) => void;
```

Add to initial state:

```ts
  terminalTabs: [],
  activeTerminalTabId: null,
```

Add the actions:

```ts
  openTerminalTab: async (server) => {
    const tabId = crypto.randomUUID();
    set({
      terminalTabs: [
        ...get().terminalTabs,
        { id: tabId, title: `${server.username || 'user'}@${server.host}`, serverId: server.id, sessionId: null, status: 'connecting' },
      ],
      activeTerminalTabId: tabId,
    });
    try {
      const sessionId = await api.openSshSession({
        host: server.host,
        port: server.port,
        username: server.username,
        serverId: server.id,
        serverName: server.name,
        sshKeyId: server.ssh_key_id,
        credentialId: server.credential_id,
      });
      set({
        terminalTabs: get().terminalTabs.map(t =>
          t.id === tabId ? { ...t, sessionId, status: 'connected' } : t
        ),
      });
    } catch (e) {
      set({
        terminalTabs: get().terminalTabs.map(t =>
          t.id === tabId ? { ...t, status: 'closed' } : t
        ),
      });
      throw e;
    }
  },

  closeTerminalTab: async (id) => {
    const tab = get().terminalTabs.find(t => t.id === id);
    const remaining = get().terminalTabs.filter(t => t.id !== id);
    set({
      terminalTabs: remaining,
      activeTerminalTabId:
        get().activeTerminalTabId === id
          ? remaining.length > 0 ? remaining[0].id : null
          : get().activeTerminalTabId,
    });
    if (tab?.sessionId) {
      try { await api.sshClose(tab.sessionId); } catch { /* already gone */ }
    }
  },

  focusTerminalTab: (id) => set({ activeTerminalTabId: id }),
```

Add `TerminalTab` to the import from `../types` in useStore.ts.

- [ ] **Step 4: Typecheck**

Run: `npx tsc --noEmit`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src/services/tauri.ts src/store/useStore.ts src/types/index.ts
git commit -m "feat: add terminal tab state and ssh session service wrappers"
```

---

### Task 4: Frontend — `Terminal` component (xterm.js)

**Files:**
- Create: `src/components/Terminal.tsx`
- Modify: `src/main.tsx` (import xterm CSS)

- [ ] **Step 1: Import xterm CSS in main.tsx**

In `src/main.tsx`, add at top:

```ts
import '@xterm/xterm/css/xterm.css';
```

- [ ] **Step 2: Write the Terminal component**

```tsx
import { useEffect, useRef } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { listen } from '@tauri-apps/api/event';
import { sshWrite, sshResize, sshClose } from '../services/tauri';
import { Button, Group, Text, Stack } from '@mantine/core';
import type { TerminalTab } from '../types';

interface Props {
  tab: TerminalTab;
  active: boolean;
}

export function Terminal({ tab, active }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<string | null>(tab.sessionId);
  const tabRef = useRef<TerminalTab>(tab);
  tabRef.current = tab;

  useEffect(() => {
    sessionRef.current = tab.sessionId;
  }, [tab.sessionId]);

  // init xterm once
  useEffect(() => {
    if (termRef.current || !containerRef.current) return;
    const term = new XTerm({
      convertEol: true,
      fontFamily: 'Consolas, monospace',
      fontSize: 13,
      cursorBlink: true,
      theme: { background: '#0d1117', foreground: '#e6edf3' },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;

    term.onData((data) => {
      const sid = sessionRef.current;
      if (!sid) return;
      const bytes = Array.from(new TextEncoder().encode(data));
      sshWrite(sid, bytes).catch(() => {});
    });

    const unlistenOutput = listen<{ sessionId: string; data: number[] }>('ssh://output', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      term.write(new Uint8Array(event.payload.data));
    });
    const unlistenExit = listen<{ sessionId: string; code: number }>('ssh://exit', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      term.write(`\r\n\x1b[31mConnection closed (code ${event.payload.code})\x1b[0m\r\n`);
    });
    let cancelled = false;
    Promise.all([unlistenOutput, unlistenExit]).then(([a, b]) => {
      if (cancelled) { a(); b(); return; }
    });
    return () => {
      cancelled = true;
      const sid = sessionRef.current;
      if (sid) sshClose(sid).catch(() => {});
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, []);

  // fit + resize when becoming active
  useEffect(() => {
    if (active && fitRef.current && termRef.current) {
      const t = termRef.current;
      fitRef.current.fit();
      const sid = sessionRef.current;
      if (sid) sshResize(sid, t.cols, t.rows).catch(() => {});
    }
  }, [active]);

  // fit on window resize
  useEffect(() => {
    const onResize = () => {
      if (active && fitRef.current && termRef.current) {
        fitRef.current.fit();
      }
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [active]);

  if (tab.status === 'closed' && !tab.sessionId) {
    return (
      <Stack align="center" justify="center" h="100%">
        <Text c="dimmed">Session failed to start.</Text>
      </Stack>
    );
  }

  return (
    <div
      ref={containerRef}
      style={{ height: '100%', padding: 8, background: '#0d1117' }}
    />
  );
}
```

- [ ] **Step 3: Typecheck**

Run: `npx tsc --noEmit`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/components/Terminal.tsx src/main.tsx
git commit -m "feat: add xterm.js Terminal component for ssh sessions"
```

---

### Task 5: Frontend — Layout with tab bar (split view)

**Files:**
- Modify: `src/components/Layout.tsx`

- [ ] **Step 1: Rewrite Layout to add the terminal section**

```tsx
import { AppShell, Group, Text, SegmentedControl, Tabs, ActionIcon } from '@mantine/core';
import { IconX } from '@tabler/icons-react';
import { Sidebar } from './Sidebar';
import { ServerList } from './ServerList';
import { SshKeys } from './SshKeys';
import { Settings } from './Settings';
import { Credentials } from './Credentials';
import { Terminal } from './Terminal';
import { useStore } from '../store/useStore';
import { useState } from 'react';

export type View = 'servers' | 'keys' | 'credentials' | 'settings';

export function Layout() {
  const [view, setView] = useState<View>('servers');
  const { terminalTabs, activeTerminalTabId, focusTerminalTab, closeTerminalTab } = useStore();
  const showTerminal = view === 'servers' && terminalTabs.length > 0;

  return (
    <AppShell
      header={{ height: 50 }}
      navbar={{ width: 250, breakpoint: 'sm' }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between">
          <Group gap="lg">
            <Text fw={700} size="lg">Remote Manager</Text>
            <SegmentedControl
              size="xs"
              value={view}
              onChange={(v) => setView(v as View)}
              data={[
                { label: 'Servers', value: 'servers' },
                { label: 'SSH Keys', value: 'keys' },
                { label: 'Credentials', value: 'credentials' },
                { label: 'Settings', value: 'settings' },
              ]}
            />
          </Group>
          {view === 'servers' && <SearchBar />}
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <Sidebar />
      </AppShell.Navbar>

      <AppShell.Main>
        {/* Servers view: kept mounted always so terminal sessions survive view switches */}
        <div style={{ display: view === 'servers' ? 'flex' : 'none', gap: 16, height: '100%', minHeight: 0 }}>
          <div style={{ flex: showTerminal ? '0 0 40%' : '1 1 auto', minWidth: 0, overflowY: 'auto' }}>
            <ServerList />
          </div>
          {showTerminal && (
            <div style={{ flex: '1 1 auto', minWidth: 0, display: 'flex', flexDirection: 'column' }}>
              <Tabs value={activeTerminalTabId ?? undefined} onChange={(v) => v && focusTerminalTab(v)} variant="outline">
                <Tabs.List>
                  {terminalTabs.map((tab) => (
                    <Tabs.Tab
                      key={tab.id}
                      value={tab.id}
                      rightSection={
                        <ActionIcon
                          size="xs"
                          variant="subtle"
                          aria-label={`Close terminal ${tab.title}`}
                          onClick={(e) => { e.stopPropagation(); closeTerminalTab(tab.id); }}
                        >
                          <IconX size={12} />
                        </ActionIcon>
                      }
                    >
                      <Text size="xs" w={120} truncate>{tab.title}</Text>
                    </Tabs.Tab>
                  ))}
                </Tabs.List>
              </Tabs>
              <div style={{ flex: 1, minHeight: 0 }}>
                {terminalTabs.map((tab) => (
                  <div key={tab.id} style={{ display: tab.id === activeTerminalTabId ? 'block' : 'none', height: '100%' }}>
                    <Terminal tab={tab} active={tab.id === activeTerminalTabId} />
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {view === 'keys' && <SshKeys />}
        {view === 'credentials' && <Credentials />}
        {view === 'settings' && <Settings />}
      </AppShell.Main>
    </AppShell>
  );
}
```

Note: `SearchBar` is used in the header — keep the import (it was in the original Layout). Ensure `SearchBar` is imported at the top.

- [ ] **Step 2: Typecheck**

Run: `npx tsc --noEmit`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/components/Layout.tsx
git commit -m "feat: add terminal tab bar and split servers/terminal layout"
```

---

### Task 6: Frontend — Connect button opens embedded terminal

**Files:**
- Modify: `src/components/ServerList.tsx`

- [ ] **Step 1: Route SSH connect to embedded terminal**

- [ ] **Step 2: Modify imports**

Remove `launchSsh` from the direct-connect path but keep it for the external menu. Update `useStore` destructuring to include `openTerminalTab`:

```ts
const { servers, credentials, sshKeys, toggleFavorite, selectedGroupId, deleteServer, loadServers, openTerminalTab } = useStore();
```

- [ ] **Step 3: Modify `handleConnect`**

```ts
const handleConnect = async (server: typeof servers[0]) => {
  try {
    if (server.protocol === 'ssh') {
      await openTerminalTab(server);
    } else {
      await launchRdp(server.host, server.username, false, false, server.id, server.name, server.credential_id);
    }
  } catch (e: any) {
    notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
  }
};

const handleConnectExternal = async (server: typeof servers[0]) => {
  try {
    await launchSsh(server.host, server.port, server.username, server.id, server.name, server.ssh_key_id, server.credential_id);
  } catch (e: any) {
    notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
  }
};
```

- [ ] **Step 4: Add menu item for external terminal**

In the actions `<Menu>` dropdown, add before "Clone":

```tsx
<Menu.Item leftSection={<IconExternalLink size={14} />} onClick={() => handleConnectExternal(server)}>
  Open in external terminal
</Menu.Item>
```

Add `IconExternalLink` to the tabler icons import.

- [ ] **Step 5: Typecheck and run e2e (mock still needs updating — see Task 7, so expect SSH test to fail until then)**

Run: `npx tsc --noEmit`
Expected: no errors

- [ ] **Step 6: Commit**

```bash
git add src/components/ServerList.tsx
git commit -m "feat: route SSH connect to embedded terminal; keep external option"
```

---

### Task 7: E2E — mock support + terminal tests

**Files:**
- Modify: `e2e/tauri-mock.ts`
- Modify: `e2e/app.spec.ts`

- [ ] **Step 1: Add ssh session handlers to the mock**

The Tauri event API `listen()` calls `invoke('plugin:event|listen', { event, target, handler: transformCallback(handler) })`. The mock must intercept that and store the callback so test code can emit events.

In `e2e/tauri-mock.ts`, inside the mock body (before `const handler = {`), add an `emit` helper and listener registry:

```ts
  const listeners = {};
  const emit = (event, payload) => {
    (listeners[event] || []).forEach((cb) => cb({ event, id: 0, payload }));
  };
  window.__rm_emit = emit;
  window.__rm_listeners = listeners;
```

In the `handler`, add:

```ts
cmd_open_ssh_session: (a) => {
  const sid = 'sess-' + uid();
  db.sessions = db.sessions || {};
  db.sessions[sid] = { server_id: a.server_id, writes: [] };
  setTimeout(() => {
    emit('ssh://output', { sessionId: sid, data: Array.from(new TextEncoder().encode('mock ssh session ready\r\n')) });
  }, 50);
  return sid;
},
cmd_ssh_write: (a) => {
  const s = db.sessions && db.sessions[a.session_id];
  if (s) {
    s.writes.push(a.data);
    emit('ssh://output', { sessionId: a.session_id, data: a.data });
  }
  return null;
},
cmd_ssh_resize: () => null,
cmd_ssh_close: (a) => { if (db.sessions) delete db.sessions[a.session_id]; return null; },
cmd_ssh_close_all: () => { db.sessions = {}; return null; },
```

In the mock's `invoke` function, intercept the event plugin BEFORE the generic `plugin:` branch:

```ts
    invoke: (cmd, args) => {
      if (cmd === 'plugin:event|listen') {
        const { event, handler } = args || {};
        (listeners[event] = listeners[event] || []).push((e) => runCallback(handler, e));
        return Promise.resolve(Date.now());
      }
      if (cmd === 'plugin:event|unlisten') return Promise.resolve(null);
      if (cmd.startsWith('plugin:')) {
        if (cmd.includes('dialog')) return Promise.resolve('/mock/path');
        return Promise.resolve(null);
      }
      if (typeof handler[cmd] !== 'function') return Promise.reject('unknown command: ' + cmd);
      try {
        const result = handler[cmd](args || {});
        save();
        return Promise.resolve(result);
      } catch (e) {
        return Promise.reject((e && e.message) ? e.message : String(e));
      }
    },
```

Note: `runCallback` is defined later in the mock body (after `handler`). Because `invoke` is a function closure evaluated at call time (not definition time), referencing `runCallback` inside it is fine regardless of ordering.

- [ ] **Step 2: Add e2e tests**

In `e2e/app.spec.ts`, add:

```ts
test('ssh connect opens embedded terminal tab and streams output', async ({ page }) => {
  await boot(page, {
    servers: [makeServer({ id: 'srv-ssh', name: 'web-node', host: '10.0.0.66', username: 'ubuntu', ssh_key_id: 'key-001' })],
    sshKeys: [{ id: 'key-001', name: 'crewkey', public_key: 'ssh-ed25519 AAAA', created_at: new Date().toISOString() }],
  });

  await expect(page.getByText('web-node', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Connect server' }).click();

  await expect(page.getByText('ubuntu@10.0.0.66', { exact: true })).toBeVisible();
  await expect(page.locator('.xterm')).toContainText('mock ssh session ready');
});

test('ssh terminal sends keystrokes and closes session', async ({ page }) => {
  await boot(page, {
    servers: [makeServer({ id: 'srv-ssh', name: 'web-node', host: '10.0.0.66', username: 'ubuntu', ssh_key_id: 'key-001' })],
    sshKeys: [{ id: 'key-001', name: 'crewkey', public_key: 'ssh-ed25519 AAAA', created_at: new Date().toISOString() }],
  });

  await page.getByRole('button', { name: 'Connect server' }).click();
  await expect(page.locator('.xterm')).toBeVisible();

  await page.locator('.xterm').click();
  await page.keyboard.type('ls');
  await page.waitForTimeout(300);

  const sessions = await page.evaluate(() => window.__rm_listeners && window.__rm_get ? window.__rm_get() : null);
  // Read back the mock's sessions store to assert writes were recorded
  const sessionWrites = await page.evaluate(() => {
    const db = localStorage.getItem('rm_mock_db_v1');
    const parsed = db ? JSON.parse(db) : null;
    return parsed && parsed.sessions ? Object.values(parsed.sessions) : [];
  });
  const hasLs = sessionWrites.some((s: any) => {
    const bytes = (s.writes || []).flat();
    return String.fromCharCode(...bytes).includes('ls');
  });
  expect(hasLs).toBe(true);

  await page.getByRole('button', { name: /Close terminal/ }).click();
  await page.waitForTimeout(300);
  const sessionsAfterClose = await page.evaluate(() => {
    const db = localStorage.getItem('rm_mock_db_v1');
    const parsed = db ? JSON.parse(db) : null;
    return parsed && parsed.sessions ? Object.keys(parsed.sessions) : [];
  });
  expect(sessionsAfterClose.length).toBe(0);
});
```

- [ ] **Step 3: Run e2e**

Run: `npx playwright test` (from repo root; playwright config starts `npm run dev`)
Expected: all existing 9 tests pass + new tests pass

- [ ] **Step 4: Commit**

```bash
git add e2e/tauri-mock.ts e2e/app.spec.ts
git commit -m "test: e2e coverage for embedded ssh terminal tabs"
```

---

### Task 8: Manual verification checklist

- [ ] **Step 1: Run the app in dev**

Run: `npm run tauri:dev`
Expected: app boots, servers view renders.

- [ ] **Step 2: Verify embedded terminal**

1. Create/edit an SSH server with an SSH key attached.
2. Click Connect → a tab `user@host` opens, terminal shows remote shell prompt.
3. Type commands (`ls`, `pwd`, `whoami`) — output streams into the terminal.
4. Open a second server → second tab appears; switching tabs keeps both sessions alive.
5. Resize the window → xterm re-fits.
6. Close a tab (×) → session is killed; the other tab remains.
7. Type `exit` in a session → "Connection closed (code 0)" shown.
8. Switch to Settings view then back to Servers → terminal tabs and sessions still alive.
9. Quit the app → ssh processes are cleaned up (verify no stray ssh.exe in Task Manager).

- [ ] **Step 3: Verify external terminal still works**

From the server actions menu (⋯) choose "Open in external terminal" → wt.exe opens the ssh session.

- [ ] **Step 4: Commit any manual-test fixes**

```bash
git add -A
git commit -m "fix: adjust terminal behavior from manual testing"
```
