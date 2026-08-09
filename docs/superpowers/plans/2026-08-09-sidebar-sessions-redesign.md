# Sidebar + Sessions Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure sidebar to nest servers inside groups, and unify SSH/RDP under a shared session/tab system.

**Architecture:** Two phases: (1) Extend data model for RDP tabs, (2) Rebuild sidebar with servers nested in groups. RDP tabs show status cards while launching mstsc.exe externally.

**Tech Stack:** TypeScript, React, Zustand, Mantine UI, Rust/Tauri, Playwright e2e

---

## File Map

### Create
- `src/components/GroupServerTree.tsx` — servers nested under groups in sidebar
- `src/components/RdpSession.tsx` — RDP status tab content component
- `src/components/SessionPanel.tsx` — unified tab content switcher (SSH xterm vs RDP status)

### Modify
- `src/types/index.ts` — rename TerminalTab → SessionTab, add protocol/processId
- `src/store/useStore.ts` — rename fields, add openRdpTab, expandedGroups state
- `src/components/Sidebar.tsx` — integrate GroupServerTree, remove old group rendering
- `src/components/Layout.tsx` — remove ServerList panel, single main area for tabs
- `src/services/tauri.ts` — add openRdpSession, rdpProcessAlive commands
- `src-tauri/src/commands/ssh.rs` — add cmd_launch_rdp_session (returns PID), cmd_rdp_process_alive
- `src-tauri/src/lib.rs` — register new commands
- `e2e/app.spec.ts` — update tests for new sidebar structure

### Delete
- `src/components/ServerList.tsx` — replaced by GroupServerTree in sidebar

---

## Task 1: Data Model — SessionTab

**Files:**
- Modify: `src/types/index.ts:76-82`

- [ ] **Step 1: Update SessionTab type**

In `src/types/index.ts`, replace `TerminalTab` with:

```typescript
export interface SessionTab {
  id: string;
  title: string;
  protocol: 'ssh' | 'rdp';
  serverId: string | null;
  sessionId: string | null;     // SSH: ConPTY session id
  processId: number | null;     // RDP: mstsc.exe PID
  status: 'connecting' | 'connected' | 'closed';
}
```

- [ ] **Step 2: Update all imports of TerminalTab**

Run: `rg "TerminalTab" src/ --files-with-matches`
Replace `TerminalTab` → `SessionTab` in all files.

Key files: `src/store/useStore.ts`, `src/components/Layout.tsx`, `src/components/Terminal.tsx`

- [ ] **Step 3: Verify tsc passes**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/types/index.ts src/store/useStore.ts src/components/Layout.tsx src/components/Terminal.tsx
git commit -m "refactor: rename TerminalTab to SessionTab with protocol field"
```

---

## Task 2: Store — Rename Fields + Add RDP Actions

**Files:**
- Modify: `src/store/useStore.ts`

- [ ] **Step 1: Rename store fields**

In `src/store/useStore.ts`, rename throughout:
- `terminalTabs` → `sessionTabs`
- `activeTerminalTabId` → `activeSessionTabId`
- `openTerminalTab` → `openSession` (keep as `openSession` for clarity)

Update the AppState interface and implementation.

- [ ] **Step 2: Add expandedGroups state**

Add to AppState interface and initial state:

```typescript
expandedGroups: Record<string, boolean>;  // group_id → expanded

// In create():
expandedGroups: {},
```

Add action:

```typescript
toggleGroupExpanded: (groupId: string) => void;

// Implementation:
toggleGroupExpanded: (groupId) => set((s) => ({
  expandedGroups: { ...s.expandedGroups, [groupId]: !s.expandedGroups[groupId] },
})),
```

- [ ] **Step 3: Add openRdpTab action**

Add to AppState interface:

```typescript
openRdpTab: (server: Server) => Promise<void>;
```

Implementation:

```typescript
openRdpTab: async (server) => {
  const tabId = crypto.randomUUID();
  set({
    sessionTabs: [
      ...get().sessionTabs,
      { id: tabId, title: server.name, protocol: 'rdp', serverId: server.id, sessionId: null, processId: null, status: 'connecting' },
    ],
    activeSessionTabId: tabId,
  });
  try {
    const pid = await api.openRdpSession({
      host: server.host,
      username: server.username,
      fullscreen: get().settings?.rdp_fullscreen ?? false,
      adminMode: get().settings?.rdp_admin_mode ?? false,
      serverId: server.id,
      serverName: server.name,
      credentialId: server.credential_id,
    });
    if (!get().sessionTabs.some(t => t.id === tabId)) {
      // Tab closed while connecting
      try { await api.rdpKillProcess(pid); } catch {}
      return;
    }
    set({
      sessionTabs: get().sessionTabs.map(t =>
        t.id === tabId ? { ...t, processId: pid, status: 'connected' } : t
      ),
    });
    // Start background poll for process exit
    startRdpPoll(get, tabId, pid);
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

Add helper function (outside the store, exported):

```typescript
function startRdpPoll(get: () => AppState, tabId: string, pid: number) {
  const poll = async () => {
    const tab = get().sessionTabs.find(t => t.id === tabId);
    if (!tab || tab.status === 'closed') return; // tab gone or already closed
    try {
      const alive = await api.rdpProcessAlive(pid);
      if (!alive) {
        set({
          sessionTabs: get().sessionTabs.map(t =>
            t.id === tabId ? { ...t, status: 'closed' } : t
          ),
        });
        return;
      }
    } catch {}
    setTimeout(poll, 2000);
  };
  setTimeout(poll, 2000);
}
```

Wait — `startRdpPoll` needs access to `set` and `get`. Define it inside the store creator or pass set. Better: define as a method.

Actually, let me restructure. Add a `_startRdpPoll` internal method:

```typescript
_startRdpPoll: (tabId: string, pid: number) => {
  const poll = async () => {
    const { sessionTabs } = get();
    const tab = sessionTabs.find(t => t.id === tabId);
    if (!tab || tab.status === 'closed') return;
    try {
      const alive = await api.rdpProcessAlive(pid);
      if (!alive) {
        set({
          sessionTabs: get().sessionTabs.map(t =>
            t.id === tabId ? { ...t, status: 'closed' } : t
          ),
        });
        return;
      }
    } catch {}
    setTimeout(poll, 2000);
  };
  setTimeout(poll, 2000);
},
```

- [ ] **Step 4: Update closeTerminalTab → closeSessionTab**

Rename to `closeSessionTab`. Add RDP process kill:

```typescript
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
  if (tab?.protocol === 'rdp' && tab.processId) {
    try { await api.rdpKillProcess(tab.processId); } catch {}
  }
},
```

- [ ] **Step 5: Update focusTerminalTab → focusSessionTab**

```typescript
focusSessionTab: (id) => set({ activeSessionTabId: id }),
```

- [ ] **Step 6: Verify tsc passes**

Run: `npx tsc --noEmit`
Expected: May fail (backend commands don't exist yet). Fix by stubbing `api.openRdpSession` and `api.rdpProcessAlive` in tauri.ts (return dummy values).

- [ ] **Step 7: Commit**

```bash
git add src/store/useStore.ts src/services/tauri.ts
git commit -m "refactor: rename terminal store fields, add RDP tab lifecycle"
```

---

## Task 3: Backend — RDP Session Commands

**Files:**
- Modify: `src-tauri/src/commands/ssh.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: (no new files, extend existing ssh.rs)

- [ ] **Step 1: Add cmd_launch_rdp_session**

In `src-tauri/src/commands/ssh.rs`, add:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn cmd_launch_rdp_session(
    state: tauri::State<crate::db::AppState>,
    host: String,
    username: String,
    fullscreen: bool,
    admin_mode: bool,
    server_id: Option<String>,
    server_name: Option<String>,
    credential_id: Option<String>,
) -> Result<i32, String> {
    // Reuse existing RDP file creation logic from cmd_launch_rdp
    validate_host(&host)?;
    let username = resolve_username(&state, username, credential_id.as_deref())?;
    validate_username(&username)?;

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let name = server_name.unwrap_or_else(|| host.clone());
        let _ = crate::history::record(&conn, server_id.as_deref(), &name, &host, Some(3389), "rdp", &username, None);
        if let Some(sid) = server_id.as_deref() {
            let _ = crate::db::operations::touch_last_connected(&conn, sid);
        }
    }

    let mut rdp_content = format!(
        "full address:s:{}\r\nusername:s:{}\r\nscreen mode id:i:{}\r\n",
        host, username, if fullscreen { 2 } else { 1 }
    );
    if admin_mode {
        rdp_content.push_str("administrative session:i:1\r\n");
    }
    if let Some(encrypted_pw) = resolve_credential_password(&state, credential_id.as_deref())? {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(encrypted_pw.as_bytes());
        rdp_content.push_str(&format!("password 51:b:{}\r\n", encoded));
    }

    let safe_host: String = host
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let temp_path = std::env::temp_dir().join(format!("rm_{}.rdp", safe_host));
    std::fs::write(&temp_path, &rdp_content)
        .map_err(|e| format!("Failed to create RDP file: {}", e))?;

    #[cfg(windows)]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let mut c = Command::new("mstsc.exe");
        c.arg(temp_path.to_str().unwrap());
        c.creation_flags(CREATE_NEW_PROCESS_GROUP);
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("mstsc.exe");
        c.arg(temp_path.to_str().unwrap());
        c
    };

    let child = cmd.spawn().map_err(|e| format!("Failed to launch RDP: {}", e))?;
    let pid = child.id() as i32;

    // Cleanup temp file after delay
    let cleanup_path = temp_path;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let _ = std::fs::remove_file(&cleanup_path);
    });

    Ok(pid)
}
```

- [ ] **Step 2: Add cmd_rdp_process_alive**

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn cmd_rdp_process_alive(pid: i32) -> Result<bool, String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/FO", "CSV", "/NH"])
            .output()
            .map_err(|e| e.to_string())?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains(&pid.to_string()))
    }
    #[cfg(not(windows))]
    {
        Ok(false) // Non-Windows: mstsc not available
    }
}
```

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the invoke handler:

```rust
commands::sessions::cmd_launch_rdp_session,
commands::sessions::cmd_rdp_process_alive,
```

Wait — these are in `commands::ssh.rs`. Move them to `commands::sessions.rs` since they're session-related. Actually, keep in ssh.rs for now since cmd_launch_rdp is already there. Add to lib.rs:

```rust
commands::ssh::cmd_launch_rdp_session,
commands::ssh::cmd_rdp_process_alive,
```

- [ ] **Step 4: cargo check**

Run: `cargo check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/ssh.rs src-tauri/src/lib.rs
git commit -m "feat: add RDP session commands (launch with PID, process alive check)"
```

---

## Task 4: Frontend API — RDP Session Commands

**Files:**
- Modify: `src/services/tauri.ts`

- [ ] **Step 1: Add API functions**

In `src/services/tauri.ts`, add:

```typescript
export const openRdpSession = (args: {
  host: string;
  username: string;
  fullscreen: boolean;
  adminMode: boolean;
  serverId?: string | null;
  serverName?: string | null;
  credentialId?: string | null;
}): Promise<number> =>
  invoke('cmd_launch_rdp_session', {
    host: args.host,
    username: args.username,
    fullscreen: args.fullscreen,
    admin_mode: args.adminMode,
    server_id: args.serverId ?? null,
    server_name: args.serverName ?? null,
    credential_id: args.credentialId ?? null,
  });

export const rdpProcessAlive = (pid: number): Promise<boolean> =>
  invoke('cmd_rdp_process_alive', { pid });

export const rdpKillProcess = (pid: number): Promise<void> =>
  invoke('cmd_rdp_kill_process', { pid });
```

- [ ] **Step 2: Add cmd_rdp_kill_process to backend**

In `src-tauri/src/commands/ssh.rs`:

```rust
#[tauri::command(rename_all = "snake_case")]
pub fn cmd_rdp_kill_process(pid: i32) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::process::Command;
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
```

Register in lib.rs.

- [ ] **Step 3: cargo check + tsc**

Run: `cargo check && npx tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/services/tauri.ts src-tauri/src/commands/ssh.rs src-tauri/src/lib.rs
git commit -m "feat: add RDP session API (launch, alive check, kill)"
```

---

## Task 5: RDP Session Component

**Files:**
- Create: `src/components/RdpSession.tsx`

- [ ] **Step 1: Create RdpSession component**

```tsx
import { Paper, Text, Badge, Button, Stack, Loader, Group } from '@mantine/core';
import { IconMonitor, IconPlayerPlay, IconRefresh } from '@tabler/icons-react';
import type { SessionTab } from '../types';

interface RdpSessionProps {
  tab: SessionTab;
  onReconnect: () => void;
}

export function RdpSession({ tab, onReconnect }: RdpSessionProps) {
  const statusColor = {
    connecting: 'blue',
    connected: 'green',
    closed: 'red',
  }[tab.status] as string;

  const statusLabel = {
    connecting: 'Connecting...',
    connected: 'Connected',
    closed: 'Disconnected',
  }[tab.status];

  return (
    <Paper p="xl" h="100%" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      <Stack align="center" gap="md">
        <IconMonitor size={48} style={{ opacity: 0.3 }} />
        <Text size="lg" fw={600}>{tab.title}</Text>
        <Group gap="sm">
          <Badge color={statusColor} size="lg">{statusLabel}</Badge>
          {tab.status === 'connecting' && <Loader size="sm" />}
        </Group>
        {tab.status === 'connected' && (
          <Text size="sm" c="dimmed">mstsc.exe is running. Close this tab to disconnect.</Text>
        )}
        {tab.status === 'closed' && (
          <Button leftSection={<IconRefresh size={14} />} onClick={onReconnect} variant="light">
            Reconnect
          </Button>
        )}
      </Stack>
    </Paper>
  );
}
```

- [ ] **Step 2: tsc check**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/components/RdpSession.tsx
git commit -m "feat: add RDP session status component"
```

---

## Task 6: Layout — Remove ServerList, Single Main Area

**Files:**
- Modify: `src/components/Layout.tsx`

- [ ] **Step 1: Simplify Layout**

Replace the servers view section in `src/components/Layout.tsx`:

```tsx
<AppShell.Main>
  {/* Kept mounted always so sessions survive view switches */}
  <div
    style={{
      display: view === 'servers' ? 'block' : 'none',
      height: 'calc(100dvh - var(--app-shell-header-offset, 0px) - var(--app-shell-footer-offset, 0px) - calc(var(--app-shell-padding, 0px) * 2))',
    }}
  >
    {sessionTabs.length > 0 ? (
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
        <Tabs value={activeSessionTabId ?? undefined} onChange={(v) => v && focusSessionTab(v)} variant="outline">
          <Tabs.List>
            {sessionTabs.map((tab) => (
              <Tabs.Tab
                key={tab.id}
                value={tab.id}
                rightSection={
                  <ActionIcon
                    size="xs"
                    variant="subtle"
                    aria-label={`Close ${tab.protocol} session ${tab.title}`}
                    onClick={(e) => { e.stopPropagation(); closeSessionTab(tab.id); }}
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
          {sessionTabs.map((tab) => (
            <div key={tab.id} style={{ display: tab.id === activeSessionTabId ? 'block' : 'none', height: '100%' }}>
              {tab.protocol === 'ssh' ? (
                <Terminal tab={tab} active={tab.id === activeSessionTabId} />
              ) : (
                <RdpSession tab={tab} onReconnect={() => {/* handled by store */}} />
              )}
            </div>
          ))}
        </div>
      </div>
    ) : (
      <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Stack align="center" gap="md" c="dimmed">
          <IconServer size={48} style={{ opacity: 0.3 }} />
          <Text>Select a server from the sidebar to connect</Text>
        </Stack>
      </div>
    )}
  </div>
  {/* ... other views ... */}
</AppShell.Main>
```

Remove the ServerList import and all its wrapper divs.

- [ ] **Step 2: tsc check**

Run: `npx tsc --noEmit`
Expected: PASS (or fix import issues)

- [ ] **Step 3: Commit**

```bash
git add src/components/Layout.tsx
git commit -m "refactor: remove ServerList panel, single main area for session tabs"
```

---

## Task 7: Sidebar — Servers Nested in Groups

**Files:**
- Create: `src/components/GroupServerTree.tsx`
- Modify: `src/components/Sidebar.tsx`

- [ ] **Step 1: Create GroupServerTree component**

```tsx
import { Box, Text, Group, ActionIcon, Stack } from '@mantine/core';
import { IconTerminal, IconMonitor, IconPlayerPlay, IconPencil, IconTrash, IconStar, IconStarFilled } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import type { Server } from '../types';

interface GroupServerTreeProps {
  servers: Server[];
  groupId: string | null;  // null = ungrouped
}

export function GroupServerTree({ servers, groupId }: GroupServerTreeProps) {
  const { openSession, openRdpTab, toggleFavorite, deleteServer } = useStore();

  const handleConnect = async (server: Server) => {
    try {
      if (server.protocol === 'ssh') {
        await openSession(server);
      } else {
        await openRdpTab(server);
      }
    } catch (e: any) {
      notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
    }
  };

  const handleDelete = (server: Server) => {
    modals.openConfirmModal({
      title: `Delete "${server.name}"`,
      children: <Text size="sm">This cannot be undone.</Text>,
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: () => deleteServer(server.id),
    });
  };

  if (servers.length === 0) return null;

  return (
    <Stack gap={2}>
      {servers.map((server) => (
        <Group
          key={server.id}
          gap={4}
          p={4}
          pl={24}
          style={{ cursor: 'default', borderRadius: 4 }}
        >
          {server.protocol === 'ssh' ? (
            <IconTerminal size={14} style={{ opacity: 0.6 }} />
          ) : (
            <IconMonitor size={14} style={{ opacity: 0.6 }} />
          )}
          <Text size="sm" style={{ flex: 1 }} truncate>{server.name}</Text>
          <ActionIcon size="sm" variant="subtle" onClick={() => toggleFavorite(server.id)}>
            {server.favorite ? <IconStarFilled size={12} color="yellow" /> : <IconStar size={12} />}
          </ActionIcon>
          <ActionIcon size="sm" variant="subtle" onClick={() => handleConnect(server)}>
            <IconPlayerPlay size={12} />
          </ActionIcon>
          <ActionIcon size="sm" variant="subtle" onClick={() => {/* open edit modal */}}>
            <IconPencil size={12} />
          </ActionIcon>
          <ActionIcon size="sm" variant="subtle" color="red" onClick={() => handleDelete(server)}>
            <IconTrash size={12} />
          </ActionIcon>
        </Group>
      ))}
    </Stack>
  );
}
```

- [ ] **Step 2: Rewrite Sidebar to use GroupServerTree**

Replace the Groups section in `src/components/Sidebar.tsx`. Remove the old `GroupNode` component. New structure:

```tsx
export function Sidebar() {
  const {
    groups, servers, settings, selectedGroupId, setSelectedGroup,
    createGroup, history, clearHistory, expandedGroups, toggleGroupExpanded,
  } = useStore();
  const [newGroupName, setNewGroupName] = useState('');

  const favorites = servers.filter(s => s.favorite);
  const rootGroups = groups.filter(g => !g.parent_id);

  // Group servers by group_id
  const groupedServers = new Map<string | null, Server[]>();
  for (const s of servers) {
    const gid = s.group_id ?? '__ungrouped__';
    if (!groupedServers.has(gid)) groupedServers.set(gid, []);
    groupedServers.get(gid)!.push(s);
  }

  // ... handleAddGroup, handleReconnect stay the same ...

  return (
    <Box>
      <Stack gap={4}>
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">Quick Access</Text>
        <Group gap={8} p="xs" style={{ cursor: 'pointer', borderRadius: 4 }}
          bg={selectedGroupId === null ? 'var(--mantine-color-dark-5)' : undefined}
          onClick={() => setSelectedGroup(null)}>
          <IconServer size={16} />
          <Text size="sm">All Servers ({servers.length})</Text>
        </Group>
        {favorites.length > 0 && (
          <Group gap={8} p="xs" style={{ cursor: 'pointer', borderRadius: 4 }}
            bg={selectedGroupId === '__favorites__' ? 'var(--mantine-color-dark-5)' : undefined}
            onClick={() => setSelectedGroup('__favorites__')}>
            <IconStar size={16} />
            <Text size="sm">Favorites ({favorites.length})</Text>
          </Group>
        )}
      </Stack>

      <Divider my="md" />

      {/* History - keep as is */}

      <Divider my="md" />

      <Stack gap={4}>
        <Group justify="space-between" align="center">
          <Text size="xs" fw={600} c="dimmed" tt="uppercase">Groups</Text>
          <ActionIcon size="sm" variant="subtle" onClick={handleAddGroup}>
            <IconPlus size={14} />
          </ActionIcon>
        </Group>

        {rootGroups.map(group => (
          <Box key={group.id}>
            <Group gap={6} p="xs" style={{ cursor: 'pointer', borderRadius: 4 }}
              bg={selectedGroupId === group.id ? 'var(--mantine-color-dark-5)' : undefined}
              onClick={() => { setSelectedGroup(group.id); toggleGroupExpanded(group.id); }}>
              <IconChevronRight size={12} style={{ transform: expandedGroups[group.id] ? 'rotate(90deg)' : 'none', transition: 'transform 0.15s' }} />
              <IconFolder size={14} />
              <Text size="sm" style={{ flex: 1 }}>{group.name}</Text>
            </Group>
            {expandedGroups[group.id] && (
              <GroupServerTree servers={groupedServers.get(group.id) ?? []} groupId={group.id} />
            )}
          </Box>
        ))}

        {/* Ungrouped servers */}
        {(groupedServers.get('__ungrouped__') ?? []).length > 0 && (
          <Box>
            <Text size="xs" c="dimmed" p="xs">Ungrouped</Text>
            <GroupServerTree servers={groupedServers.get('__ungrouped__')!} groupId={null} />
          </Box>
        )}

        <TextInput size="xs" placeholder="New group name + Enter"
          value={newGroupName} onChange={(e) => setNewGroupName(e.currentTarget.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleAddGroup(); }} mt="xs" />
      </Stack>
    </Box>
  );
}
```

- [ ] **Step 3: tsc check**

Run: `npx tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/components/GroupServerTree.tsx src/components/Sidebar.tsx
git commit -m "feat: nest servers inside groups in sidebar"
```

---

## Task 8: Update Remaining Components

**Files:**
- Modify: `src/components/SearchBar.tsx` — update to open sessions
- Modify: `e2e/app.spec.ts` — update selectors and assertions

- [ ] **Step 1: Update SearchBar to use openSession/openRdpTab**

The SearchBar dispatches `rm:filter-protocol` events. It may also need to open sessions. Check if it calls `launchRdp` or `openTerminalTab`. If so, update to use new store actions.

- [ ] **Step 2: Update e2e tests**

Key changes in `e2e/app.spec.ts`:
- Server list is now in sidebar, not a separate panel
- Tab assertions change from terminalTabs → sessionTabs
- RDP connect opens a status tab, not mstsc directly

Update test selectors:
- `page.getByRole('button', { name: 'Connect server' })` → may need to target sidebar server connect buttons
- `.xterm` assertions stay for SSH
- Add RDP tab assertions if applicable

- [ ] **Step 3: Full verification**

Run: `npx tsc --noEmit && cargo check && npx playwright test`
Expected: ALL PASS

- [ ] **Step 4: Commit**

```bash
git add src/components/SearchBar.tsx e2e/app.spec.ts
git commit -m "test: update e2e tests for sidebar and session tab redesign"
```

---

## Task 9: Cleanup + Final Verification

- [ ] **Step 1: Delete ServerList.tsx**

Run: `rm src/components/ServerList.tsx`
Remove all imports of ServerList from other files.

- [ ] **Step 2: Remove old launchRdp if unused**

Check if `cmd_launch_rdp` (non-session version) is still needed. If only `cmd_launch_rdp_session` is used, remove the old one and update frontend.

- [ ] **Step 3: Full test suite**

Run: `npx tsc --noEmit && cargo check && cargo test && npx playwright test`
Expected: ALL PASS

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "refactor: complete sidebar + session tab redesign"
```

---

## Execution Order

Tasks 1-2 (data model + store) → Task 3-4 (backend + API) → Task 5 (RDP component) → Task 6 (layout) → Task 7 (sidebar) → Task 8 (cleanup + e2e) → Task 9 (final)

Each task is independently testable and committable.
