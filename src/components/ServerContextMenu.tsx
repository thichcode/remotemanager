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