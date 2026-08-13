# Design: Duplicate Host via Right-Click Context Menu

Date: 2026-08-13
Status: Approved
Applies to: Remote Manager (Tauri 2 + React 18 + Mantine v7 + Zustand)

## Summary

Add a "Duplicate" capability and a reusable right-click context menu for server
rows in the sidebar tree. The backend plumbing already exists and is fully
functional:

- `cmd_clone_server` command (`src-tauri/src/commands/servers.rs`)
- `operations::clone_server` SQL (`src-tauri/src/db/operations.rs`) — clones
  name/host/port/protocol/username/group/tags/notes/description and normalizes
  credential/ssh-key references; new row gets a fresh UUID and is named
  `{name} (copy)`.
- `cloneServer(id)` service wrapper (`src/services/tauri.ts`)
- `cloneServer(id)` store action (`src/store/useStore.ts`) — appends the new
  copy to the visible list.

The only missing piece is the UI trigger. This design adds it as a cursor-following
context menu (Approach A), which also absorbs the row's existing icon actions to
declutter each server row.

## Requirements

1. Right-clicking a server row opens a context menu positioned at the cursor,
   clamped to the viewport.
2. The menu offers: Connect, Duplicate, Toggle favorite / Unfavorite, Edit, Delete.
3. Duplicate creates `{name} (copy)` instantly (no prompt), same group/tags/auth,
   and appends it to the visible list.
4. Row icon buttons (Connect/Favorite/Edit/Delete) are removed; the row displays
   only the protocol icon, name/badges, and host:port. Single/double-click still connects.
5. Keyboard affordances are preserved: Enter = Connect, Delete = Delete confirm,
   ArrowUp/ArrowDown = row focus movement.
6. Menu closes on outside click, Escape, or after an action executes.
7. Duplicate in Favorites view must not leak the non-favorite copy into the list.

## Architecture

### New component: `src/components/ServerContextMenu.tsx`

- Manages local state `{ x, y, server } | null`.
- Renders through Mantine `<Portal>` a `Paper`/`Stack` of `UnstyledButton` action
  rows (icon + label), styled to match the dark sidebar.
- Positioned `fixed` at `(x, y)`, clamped to viewport using a ref to measure the
  menu's own size with a small margin.
- Owns action logic: imports `useStore`, `modals`, `notifications`.
- Action handlers:
  - **Connect** — `openSession` for SSH, `openRdpTab` for RDP (existing helpers).
  - **Duplicate** — `cloneServer(server.id)`; on success shows
    `Duplicated "{name}"` notification (green).
  - **Toggle favorite** — `toggleFavorite(server.id)`; label switches between
    "Toggle favorite" / "Unfavorite" based on `server.favorite`.
  - **Edit** — opens the existing `ServerForm` modal pre-filled.
  - **Delete** — opens the existing confirm modal (`modals.openConfirmModal`),
    preserving the 5-second undo notification flow.
- Close triggers: `mousedown` outside the menu, `Escape` key, or after any action.
- Exposes `open(x, y, server)` and `close()` to the parent via a forwarded ref or
  a controlled `open` object prop agreed at implementation time.

### `src/components/GroupServerTree.tsx` changes

- Remove the four `ActionIcon` buttons; slim the row to
  protocol icon + name/badges (status/connected) + host:port.
- Add `onContextMenu` to the row element: `e.preventDefault()` then open the menu
  at `e.clientX/e.clientY` with the row's server.
- Keep row `onClick`/`onDoubleClick` = Connect and the keyboard navigation.

### `src/store/useStore.ts` changes

- Make `cloneServer` group-aware: when `selectedGroupId === FAVORITES_ID`, skip
  appending the non-favorite copy and reload servers for the current view instead;
  otherwise append to the visible list as today.

## Data Flow

```
Row onContextMenu
  -> ServerContextMenu.open(x, y, server)
  -> user clicks "Duplicate"
  -> store.cloneServer(server.id)
  -> api.cloneServer(id) -> cmd_clone_server -> operations::clone_server
  -> new {name} (copy) row appended (or list reloaded in Favorites view)
  -> green notification "Duplicated {name}"
```

## Error Handling

- Any action failure surfaces through the existing `notifications.show` red toast.
- Menu positions/sizing guarded so it never renders off-screen.
- No new backend surface: creates only UI code plus one small store tweak.

## Testing

- `npm run build` (tsc + vite) passes.
- Manual: right-click row -> menu appears at cursor; each action works; menu closes
  on outside click/Escape; duplicate appears in same group; duplicate from
  Favorites view does not show in the Favorites list; row keyboard nav intact.

## Out of Scope

- No backend changes (clone logic already correct).
- No rename flow after duplicating (auto-name `(copy)` only).
- No keyboard shortcut (e.g. Shift+F10) to open the menu — deferred unless requested.