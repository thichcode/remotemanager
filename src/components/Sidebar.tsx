import { Box, Text, Group, ActionIcon, Stack, Divider } from '@mantine/core';
import { IconPlus, IconServer, IconStar } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { useState } from 'react';

export function Sidebar() {
  const { groups, servers, selectedGroupId, setSelectedGroup, createGroup } = useStore();
  const [newGroupName, setNewGroupName] = useState('');

  const favorites = servers.filter(s => s.favorite);
  const rootGroups = groups.filter(g => !g.parent_id);

  const handleAddGroup = async () => {
    if (newGroupName.trim()) {
      await createGroup(newGroupName.trim());
      setNewGroupName('');
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
