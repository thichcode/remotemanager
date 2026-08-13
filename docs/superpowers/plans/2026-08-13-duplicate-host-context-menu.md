# Duplicate Host via Right-Click Context Menu — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Duplicate" action and a cursor-following right-click context menu for server rows in the sidebar.

**Architecture:** A new `ServerContextMenu` component, owned by `Sidebar`, renders at the cursor via Portal and owns all row-action logic (Connect/Duplicate/Favorite/Edit/Delete). `GroupServerTree` rows slim down to name/badges only, keep click=connect + keyboard nav, and call `onOpenMenu(server, x, y)` on right-click. Store `cloneServer` gets a Favorites-view guard. All backend clone plumbing already exists and is unchanged.

**Tech Stack:** React 18 + TypeScript + Mantine v7 (Portal, Paper, UnstyledButton) + Zustand v4 + Playwright e2e with a mocked Tauri backend (`e2e/tauri-mock.ts` already implements `cmd_clone_server`).

**Verification commands:**
- Tests: `npx playwright test` (starts dev server on :1420 automatically, headless)
- Typecheck/build: `npm run build` (tsc + vite build)

---

## File Structure

- **Create** `src/components/ServerContextMenu.tsx` — cursor-positioned context menu; owns Connect/Duplicate/Favorite/Edit/Delete logic.
- **Modify** `src/components/GroupServerTree.tsx` — remove the 4 row `ActionIcon`s, add `onContextMenu` + `onOpenMenu` prop.
- **Modify** `src/components/Sidebar.tsx:33` — own menu state, pass `onOpenMenu` to all `GroupServerTree` usages, render `<ServerContextMenu>`.
- **Modify** `src/store/useStore.ts:141-150` — make `cloneServer` skip appending in `__favorites__` view.
- **Modify** `e2e/app.spec.ts` — add duplicate + context-menu tests; update 4 existing tests that use removed icon buttons.

---

### Task 1: Write/update e2e tests (red phase)

**Files:**
- Modify: `e2e/app.spec.ts`

- [ ] **Step 1: Replace the 4 tests that use the soon-to-be-removed row icon buttons**

Replace the connect/favorite/edit button interactions with right-click context-menu interactions.

Test `favoriting increments sidebar count` (currently line 74-79) — replace:

```ts
test('favoriting increments sidebar count', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 's1', name: 'web1', favorite: false })] });
  await expect(page.getByText('web1', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Toggle favorite' }).click();
  await expect(page.getByText('Favorites (1)', { exact: true })).toBeVisible();
});
```

with:

```ts
test('favoriting via context menu increments sidebar count', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 's1', name: 'web1', favorite: false })] });
  await expect(page.getByText('web1', { exact: true })).toBeVisible();
  await page.getByText('web1', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Toggle favorite' }).click();
  await expect(page.getByText('Favorites (1)', { exact: true })).toBeVisible();
});
```

Test `SSH key selection persists across edit and reload` (line 63) — replace:

```ts
  await page.getByRole('button', { name: 'Edit server' }).click();
```

with:

```ts
  await page.getByText('gitlab', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Edit' }).click();
```

Tests `ssh connect opens embedded terminal tab and streams output` (line 134), `ssh terminal sends keystrokes and closes session` (line 146), and `ssh terminal session survives view switches` (line 182) — replace ALL occurrences of:

```ts
  await page.getByRole('button', { name: 'Connect server' }).click();
```

with:

```ts
  await page.getByText('web-node', { exact: true }).click();
```

(Clicking the row text triggers the row's single-click connect.)

- [ ] **Step 2: Add two new failing tests for Duplicate**

Append before the final closing of the file (after `ssh terminal session survives view switches`):

```ts
test('duplicate creates a named copy in the same group', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 'srv-d', name: 'db', host: '10.0.0.9' })] });
  await expect(page.getByText('db', { exact: true })).toBeVisible();

  await page.getByText('db', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Duplicate' }).click();

  await expect(page.getByText('db (copy)', { exact: true })).toBeVisible();
});

test('duplicate from favorites view does not leak into favorites list', async ({ page }) => {
  await boot(page, { servers: [makeServer({ id: 'srv-f', name: 'db', host: '10.0.0.9', favorite: true })] });
  await expect(page.getByText('Favorites (1)', { exact: true })).toBeVisible();

  await page.getByText('Favorites (1)', { exact: true }).click();
  await expect(page.getByText('db', { exact: true })).toBeVisible();

  await page.getByText('db', { exact: true }).click({ button: 'right' });
  await page.getByRole('button', { name: 'Duplicate' }).click();

  // The copy is not a favorite, so it must NOT appear in the favorites list.
  await expect(page.getByText('db (copy)', { exact: true })).not.toBeVisible();
});
```

- [ ] **Step 3: Run the suite to confirm the red phase**

Run: `npx playwright test`
Expected: the `duplicate` tests FAIL (no context menu); the 4 updated tests FAIL (removed buttons / no menu). Failed tests reference a missing "Duplicate" / "Toggle favorite" / "Edit" button or timeout waiting for it.

- [ ] **Step 4: Commit**

```bash
git add e2e/app.spec.ts
git commit -m "test: cover duplicate host via right-click context menu"
```

---

### Task 2: Create `ServerContextMenu` component

**Files:**
- Create: `src/components/ServerContextMenu.tsx`

- [ ] **Step 1: Write the component**

```tsx
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Portal, Paper, Stack, UnstyledButton, Group, Text, Divider } from '@mantine/core';
import {
  IconPlayerPlay, IconCopy, IconStar, IconStarFilled, IconPencil, IconTrash,
} from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import { ServerForm } from './ServerForm';
import type { Server } from '../types';

export interface ContextMenuState {
  x: number;
  y: number;
  server: Server;
}

interface ServerContextMenuProps {
  state: ContextMenuState | null;
  onClose: () => void;
}

export function ServerContextMenu({ state, onClose }: ServerContextMenuProps) {
  const { openSession, openRdpTab, toggleFavorite, deleteServer, cloneServer } = useStore();
  const menuRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: 0, top: 0 });

  useEffect(() => {
    if (!state) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    const onMouseDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('mousedown', onMouseDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('mousedown', onMouseDown);
    };
  }, [state, onClose]);

  useLayoutEffect(() => {
    if (!state || !menuRef.current) return;
    const rect = menuRef.current.getBoundingClientRect();
    const margin = 8;
    const left = Math.min(state.x, window.innerWidth - rect.width - margin);
    const top = Math.min(state.y, window.innerHeight - rect.height - margin);
    setPos({ left: Math.max(margin, left), top: Math.max(margin, top) });
  }, [state]);

  if (!state) return null;
  const { server } = state;

  const run = async (fn: () => Promise<void>) => {
    try {
      await fn();
    } catch (e: unknown) {
      notifications.show({
        title: 'Error',
        message: e instanceof Error ? e.message : String(e),
        color: 'red',
      });
    }
    onClose();
  };

  const handleConnect = () => run(async () => {
    if (server.protocol === 'ssh') {
      await openSession(server);
    } else {
      await openRdpTab(server);
    }
  });

  const handleDuplicate = () => run(async () => {
    await cloneServer(server.id);
    notifications.show({ title: 'Duplicated', message: `"${server.name}" copied`, color: 'green' });
  });

  const handleToggleFavorite = () => run(async () => {
    await toggleFavorite(server.id);
  });

  const handleEdit = () => {
    modals.open({ title: `Edit "${server.name}"`, children: <ServerForm server={server} />, size: 'lg' });
    onClose();
  };

  const handleDelete = () => {
    modals.openConfirmModal({
      title: `Delete "${server.name}"`,
      children: <Text size="sm">This cannot be undone.</Text>,
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: () => deleteServer(server.id),
    });
    onClose();
  };

  return (
    <Portal>
      <Paper
        ref={menuRef}
        shadow="md"
        withBorder
        p="xs"
        style={{ position: 'fixed', left: pos.left, top: pos.top, zIndex: 300, minWidth: 180 }}
      >
        <Stack gap={2}>
          <UnstyledButton onClick={handleConnect} p="xs" style={{ borderRadius: 4 }}>
            <Group gap={8}>
              <IconPlayerPlay size={14} />
              <Text size="sm">Connect</Text>
            </Group>
          </UnstyledButton>
          <UnstyledButton onClick={handleDuplicate} p="xs" style={{ borderRadius: 4 }}>
            <Group gap={8}>
              <IconCopy size={14} />
              <Text size="sm">Duplicate</Text>
            </Group>
          </UnstyledButton>
          <Divider my={2} />
          <UnstyledButton onClick={handleToggleFavorite} p="xs" style={{ borderRadius: 4 }}>
            <Group gap={8}>
              {server.favorite
                ? <IconStarFilled size={14} color="yellow" />
                : <IconStar size={14} />}
              <Text size="sm">{server.favorite ? 'Unfavorite' : 'Toggle favorite'}</Text>
            </Group>
          </UnstyledButton>
          <UnstyledButton onClick={handleEdit} p="xs" style={{ borderRadius: 4 }}>
            <Group gap={8}>
              <IconPencil size={14} />
              <Text size="sm">Edit</Text>
            </Group>
          </UnstyledButton>
          <Divider my={2} />
          <UnstyledButton onClick={handleDelete} p="xs" style={{ borderRadius: 4 }} c="red">
            <Group gap={8}>
              <IconTrash size={14} />
              <Text size="sm" c="red">Delete</Text>
            </Group>
          </UnstyledButton>
        </Stack>
      </Paper>
    </Portal>
  );
}
```

- [ ] **Step 2: Typecheck**

Run: `npm run build`
Expected: PASS (tsc compiles; no import errors).

- [ ] **Step 3: Commit**

```bash
git add src/components/ServerContextMenu.tsx
git commit -m "feat: add cursor-positioned server context menu component"
```

---

### Task 3: Wire context menu into Sidebar and GroupServerTree

**Files:**
- Modify: `src/components/GroupServerTree.tsx`
- Modify: `src/components/Sidebar.tsx`

- [ ] **Step 1: Rewrite `GroupServerTree.tsx`**

Replace the entire file with:

```tsx
import { useRef, useCallback } from 'react';
import { Text, Group, Stack, Badge, Tooltip } from '@mantine/core';
import { IconTerminal, IconDeviceDesktop } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import type { Server } from '../types';

interface GroupServerTreeProps {
  servers: Server[];
  onOpenMenu: (server: Server, x: number, y: number) => void;
}

export function GroupServerTree({ servers, onOpenMenu }: GroupServerTreeProps) {
  const { openSession, openRdpTab, deleteServer, sessionTabs } = useStore();
  const listRef = useRef<HTMLDivElement>(null);
  const activeSessionServerIds = new Set(sessionTabs.filter(t => t.status === 'connected' || t.status === 'connecting').map(t => t.serverId).filter(Boolean));

  const handleConnect = useCallback(async (server: Server) => {
    try {
      if (server.protocol === 'ssh') {
        await openSession(server);
      } else {
        await openRdpTab(server);
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      notifications.show({ title: 'Error', message: msg, color: 'red' });
    }
  }, [openSession, openRdpTab]);

  const handleDelete = useCallback((server: Server) => {
    modals.openConfirmModal({
      title: `Delete "${server.name}"`,
      children: <Text size="sm">This cannot be undone.</Text>,
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: () => deleteServer(server.id),
    });
  }, [deleteServer]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent, server: Server) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleConnect(server);
    } else if (e.key === 'Delete') {
      e.preventDefault();
      handleDelete(server);
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const dir = e.key === 'ArrowDown' ? 1 : -1;
      const items = listRef.current?.querySelectorAll<HTMLElement>('[data-server-row]');
      if (!items) return;
      const idx = Array.from(items).indexOf(e.currentTarget as HTMLElement);
      const next = items[idx + dir];
      if (next) next.focus();
    }
  }, [handleConnect, handleDelete]);

  if (servers.length === 0) return null;

  return (
    <Stack gap={2} ref={listRef}>
      {servers.map((server) => {
        const isConnected = activeSessionServerIds.has(server.id);
        return (
          <Group
            key={server.id}
            gap={6}
            p="xs"
            pl={24}
            wrap="nowrap"
            style={{ cursor: 'pointer', borderRadius: 4 }}
            data-server-row
            tabIndex={0}
            role="listitem"
            aria-label={`${server.name} (${server.protocol.toUpperCase()}) ${server.host}:${server.port}`}
            onClick={() => handleConnect(server)}
            onDoubleClick={() => handleConnect(server)}
            onContextMenu={(e) => { e.preventDefault(); onOpenMenu(server, e.clientX, e.clientY); }}
            onKeyDown={(e) => handleKeyDown(e, server)}
          >
            <Tooltip label={server.protocol.toUpperCase()}>
              {server.protocol === 'ssh' ? (
                <IconTerminal size={14} color={isConnected ? 'var(--mantine-color-green-5)' : undefined} style={{ opacity: isConnected ? 1 : 0.5 }} />
              ) : (
                <IconDeviceDesktop size={14} color={isConnected ? 'var(--mantine-color-green-5)' : undefined} style={{ opacity: isConnected ? 1 : 0.5 }} />
              )}
            </Tooltip>
            <Stack gap={0} style={{ flex: 1, minWidth: 0 }}>
              <Group gap={6} wrap="nowrap">
                <Text size="sm" truncate>{server.name}</Text>
                <Badge size="xs" variant="light" color={server.protocol === 'ssh' ? 'blue' : 'grape'}>{server.protocol.toUpperCase()}</Badge>
                {isConnected && <Badge size="xs" variant="dot" color="green">Connected</Badge>}
              </Group>
              <Text size="xs" c="dimmed" truncate>{server.host}:{server.port}</Text>
            </Stack>
          </Group>
        );
      })}
    </Stack>
  );
}
```

- [ ] **Step 2: Modify `Sidebar.tsx`**

Add to the imports (after line 9, keeping alphabetical grouping):

```tsx
import { ServerContextMenu } from './ServerContextMenu';
import type { ContextMenuState } from './ServerContextMenu';
import type { Server } from '../types';
```

(Keep the existing `import type { Server } from '../types';` line — used by the new callback. If duplicate import error arises, remove the older `Server` import instead.)

Add state near the other `useState` declarations (after line 35):

```tsx
const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
const openContextMenu = useCallback((server: Server, x: number, y: number) => {
  setContextMenu({ x, y, server });
}, []);
```

Update imports from React at line 4 to include `useCallback`:

```tsx
import { useState, useEffect, useCallback } from 'react';
```

Pass `onOpenMenu` to all three `GroupServerTree` usages:

- Line 189: `<GroupServerTree servers={groupSrvs} />` → `<GroupServerTree servers={groupSrvs} onOpenMenu={openContextMenu} />`
- Line 198: `<GroupServerTree servers={ungroupedServers} />` → `<GroupServerTree servers={ungroupedServers} onOpenMenu={openContextMenu} />`
- Line 205: `<GroupServerTree servers={recentServers} />` → `<GroupServerTree servers={recentServers} onOpenMenu={openContextMenu} />`

Render the menu once, just inside the root `<Box>` (after the opening tag on line 67):

```tsx
      <ServerContextMenu state={contextMenu} onClose={() => setContextMenu(null)} />
```

- [ ] **Step 3: Typecheck**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 4: Run e2e (context-menu tests should pass now)**

Run: `npx playwright test`
Expected: the two duplicate tests and the 4 updated tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/GroupServerTree.tsx src/components/Sidebar.tsx
git commit -m "feat: wire right-click server context menu into sidebar rows"
```

---

### Task 4: Fix `cloneServer` Favorites-view leak

**Files:**
- Modify: `src/store/useStore.ts:141-150`

- [ ] **Step 1: Make `cloneServer` group-aware**

Replace lines 141-150:

```ts
  cloneServer: async (id) => {
    const newId = await api.cloneServer(id);
    const src = get().servers.find(s => s.id === id);
    if (src) {
      const cloned = { ...src, id: newId, name: `${src.name} (copy)`, favorite: false, last_connected_at: null };
      set({ servers: [...get().servers, cloned] });
    } else {
      await get().loadServers();
    }
  },
```

with:

```ts
  cloneServer: async (id) => {
    const newId = await api.cloneServer(id);
    const src = get().servers.find(s => s.id === id);
    if (src && get().selectedGroupId !== FAVORITES_ID) {
      const cloned = { ...src, id: newId, name: `${src.name} (copy)`, favorite: false, last_connected_at: null };
      set({ servers: [...get().servers, cloned] });
    } else {
      await get().loadServers();
    }
  },
```

`FAVORITES_ID` is already defined at the top of the file (line 5).

- [ ] **Step 2: Verify with the favorites e2e test**

Run: `npx playwright test`
Expected: `duplicate from favorites view does not leak into favorites list` PASSES.

- [ ] **Step 3: Typecheck + commit**

Run: `npm run build`
Expected: PASS.

```bash
git add src/store/useStore.ts
git commit -m "fix: keep non-favorite clones out of favorites view"
```

---

### Task 5: Full verification

- [ ] **Step 1: Run full e2e suite**

Run: `npx playwright test`
Expected: ALL tests pass (existing + new).

- [ ] **Step 2: Run typecheck/build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 3: Confirm git status clean**

Run: `git status --short`
Expected: clean working tree (all committed).

---

## Self-Review

**Spec coverage:**
- Right-click menu at cursor, clamped — Task 2 (Portal + `useLayoutEffect` clamp) ✓
- Menu items Connect/Duplicate/Toggle favorite/Edit/Delete — Task 2 ✓
- Duplicate → `{name} (copy)`, appends to visible list — Task 2 (`cloneServer`) ✓
- Row icon buttons removed; click/double-click still connects — Task 3 (GroupServerTree rewrite) ✓
- Keyboard nav preserved (Enter/Delete/arrows) — Task 3 (unchanged `handleKeyDown`) ✓
- Close on outside click / Escape / after action — Task 2 (`useEffect` listeners, `run()` + `onClose()`) ✓
- Favorites-view leak guard — Task 4 ✓
- Error handling via red toast — Task 2 (`run()` catch) ✓
- No backend changes — confirmed (mock already has `cmd_clone_server`) ✓

**Placeholders:** none — all steps contain full code.

**Type consistency:** `ContextMenuState` exported from Task 2 and consumed in Task 3 Sidebar; `onOpenMenu(server, x, y)` signature identical in GroupServerTree prop and Sidebar callback; `cloneServer(id: string)` matches existing store signature.