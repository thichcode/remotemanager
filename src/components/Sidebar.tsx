import { Box, Text, Group, ActionIcon, Stack, Divider } from '@mantine/core';
import { IconPlus, IconServer, IconStar, IconClock, IconTrash } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { useState } from 'react';
import { launchSsh, launchRdp } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import type { HistoryEntry } from '../types';

export function Sidebar() {
  const { groups, servers, selectedGroupId, setSelectedGroup, createGroup, history, clearHistory } = useStore();
  const [newGroupName, setNewGroupName] = useState('');

  const favorites = servers.filter(s => s.favorite);
  const rootGroups = groups.filter(g => !g.parent_id);

  const handleAddGroup = async () => {
    if (newGroupName.trim()) {
      await createGroup(newGroupName.trim());
      setNewGroupName('');
    }
  };

  const handleReconnect = async (entry: HistoryEntry) => {
    try {
      if (entry.protocol === 'ssh') {
        await launchSsh(entry.host, entry.port ?? 22, entry.username, entry.server_id ?? undefined, entry.server_name, entry.ssh_key_id ?? undefined);
      } else {
        await launchRdp(entry.host, entry.username, false, false, entry.server_id ?? undefined, entry.server_name);
      }
    } catch (e: any) {
      notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
    }
  };

  return (
    <Box>
      <Stack gap={4}>
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">Quick Access</Text>
        <Group
          gap={8}
          p="xs"
          style={{ cursor: 'pointer', borderRadius: 4 }}
          bg={selectedGroupId === null ? 'var(--mantine-color-dark-5)' : undefined}
          onClick={() => setSelectedGroup(null)}
        >
          <IconServer size={16} />
          <Text size="sm">All Servers ({servers.length})</Text>
        </Group>
        {favorites.length > 0 && (
          <Group
            gap={8}
            p="xs"
            style={{ cursor: 'pointer', borderRadius: 4 }}
          >
            <IconStar size={16} />
            <Text size="sm">Favorites ({favorites.length})</Text>
          </Group>
        )}
      </Stack>

      <Divider my="md" />

      {history.length > 0 && (
        <Stack gap={4} mb="md">
          <Group justify="space-between" align="center">
            <Text size="xs" fw={600} c="dimmed" tt="uppercase">Recent</Text>
            <ActionIcon size="sm" variant="subtle" onClick={clearHistory}>
              <IconTrash size={14} />
            </ActionIcon>
          </Group>
          {history.slice(0, 5).map(entry => (
            <Group
              key={entry.id}
              gap={8}
              p="xs"
              style={{ cursor: 'pointer', borderRadius: 4 }}
              onClick={() => handleReconnect(entry)}
            >
              <IconClock size={14} />
              <Box style={{ flex: 1 }}>
                <Text size="sm" truncate>{entry.server_name}</Text>
                <Text size="xs" c="dimmed">{entry.host}:{entry.port ?? (entry.protocol === 'rdp' ? 3389 : 22)}</Text>
              </Box>
            </Group>
          ))}
        </Stack>
      )}

      <Divider my="md" />

      <Stack gap={4}>
        <Group justify="space-between" align="center">
          <Text size="xs" fw={600} c="dimmed" tt="uppercase">Groups</Text>
          <ActionIcon size="sm" variant="subtle" onClick={handleAddGroup}>
            <IconPlus size={14} />
          </ActionIcon>
        </Group>

        {rootGroups.map(group => (
          <Group
            key={group.id}
            gap={8}
            p="xs"
            style={{ cursor: 'pointer', borderRadius: 4 }}
            bg={selectedGroupId === group.id ? 'var(--mantine-color-dark-5)' : undefined}
            onClick={() => setSelectedGroup(group.id)}
          >
            <Text size="sm">{group.name}</Text>
          </Group>
        ))}
      </Stack>
    </Box>
  );
}
