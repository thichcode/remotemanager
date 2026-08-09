import { Text, Group, ActionIcon, Stack } from '@mantine/core';
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

  const handleEdit = (server: Server) => {
    modals.open({
      title: `Edit "${server.name}"`,
      children: <ServerForm server={server} />,
      size: 'lg',
    });
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
          p="xs"
          pl={24}
          style={{ cursor: 'default', borderRadius: 4 }}
        >
          {server.protocol === 'ssh' ? (
            <IconTerminal size={14} style={{ opacity: 0.6 }} />
          ) : (
            <IconDeviceDesktop size={14} style={{ opacity: 0.6 }} />
          )}
          <Text size="sm" style={{ flex: 1 }} truncate>{server.name}</Text>
          <ActionIcon size="sm" variant="subtle" aria-label="Toggle favorite" onClick={() => toggleFavorite(server.id)}>
            {server.favorite ? <IconStarFilled size={12} color="yellow" /> : <IconStar size={12} />}
          </ActionIcon>
          <ActionIcon size="sm" variant="subtle" aria-label="Connect server" onClick={() => handleConnect(server)}>
            <IconPlayerPlay size={12} />
          </ActionIcon>
          <ActionIcon size="sm" variant="subtle" aria-label="Edit server" onClick={() => handleEdit(server)}>
            <IconPencil size={12} />
          </ActionIcon>
          <ActionIcon size="sm" variant="subtle" aria-label="Delete server" color="red" onClick={() => handleDelete(server)}>
            <IconTrash size={12} />
          </ActionIcon>
        </Group>
      ))}
    </Stack>
  );
}
