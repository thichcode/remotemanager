import { useRef, useCallback } from 'react';
import { Text, Group, ActionIcon, Stack, Badge, Tooltip } from '@mantine/core';
import { IconTerminal, IconDeviceDesktop, IconPlayerPlay, IconPencil, IconTrash, IconStar, IconStarFilled } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';
import { notifications } from '@mantine/notifications';
import type { Server } from '../types';
import { ServerForm } from './ServerForm';

interface GroupServerTreeProps {
  servers: Server[];
}

export function GroupServerTree({ servers }: GroupServerTreeProps) {
  const { openSession, openRdpTab, toggleFavorite, deleteServer, sessionTabs } = useStore();
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

  const handleEdit = useCallback((server: Server) => {
    modals.open({
      title: `Edit "${server.name}"`,
      children: <ServerForm server={server} />,
      size: 'lg',
    });
  }, []);

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
            <Tooltip label="Toggle favorite">
              <ActionIcon size="sm" variant="subtle" aria-label="Toggle favorite" onClick={(e) => { e.stopPropagation(); toggleFavorite(server.id); }}>
                {server.favorite ? <IconStarFilled size={12} color="yellow" /> : <IconStar size={12} />}
              </ActionIcon>
            </Tooltip>
            <Tooltip label="Connect">
              <ActionIcon size="sm" variant="subtle" aria-label="Connect server" onClick={(e) => { e.stopPropagation(); handleConnect(server); }}>
                <IconPlayerPlay size={12} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label="Edit">
              <ActionIcon size="sm" variant="subtle" aria-label="Edit server" onClick={(e) => { e.stopPropagation(); handleEdit(server); }}>
                <IconPencil size={12} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label="Delete">
              <ActionIcon size="sm" variant="subtle" aria-label="Delete server" color="red" onClick={(e) => { e.stopPropagation(); handleDelete(server); }}>
                <IconTrash size={12} />
              </ActionIcon>
            </Tooltip>
          </Group>
        );
      })}
    </Stack>
  );
}
