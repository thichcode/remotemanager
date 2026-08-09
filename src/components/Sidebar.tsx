import { Box, Text, Group, ActionIcon, Stack, Divider, TextInput, Button } from '@mantine/core';
import { IconPlus, IconServer, IconStar, IconClock, IconTrash, IconPencil, IconFolder, IconChevronRight } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { useState } from 'react';
import { launchSsh, launchRdp } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import { modals } from '@mantine/modals';
import type { HistoryEntry } from '../types';

function GroupNode({ group, depth }: { group: { id: string; name: string }, depth: number }) {
  const { selectedGroupId, setSelectedGroup, deleteGroup, updateGroup } = useStore();
  const [addingChild, setAddingChild] = useState(false);
  const [childName, setChildName] = useState('');

  const handleRename = () => {
    modals.open({
      title: `Rename "${group.name}"`,
      children: <RenameGroupForm currentName={group.name} onSave={(name) => updateGroup(group.id, name)} />,
      size: 'sm',
    });
  };

  const handleDelete = () => {
    modals.openConfirmModal({
      title: 'Delete Group',
      children: <Text size="sm">Delete "{group.name}"? Servers in it will become ungrouped.</Text>,
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: () => deleteGroup(group.id),
    });
  };

  const handleAddChild = async () => {
    if (childName.trim()) {
      await createChild(group.id, childName.trim());
      setChildName('');
      setAddingChild(false);
    }
  };

  const { createGroup } = useStore();
  const createChild = async (parentId: string, name: string) => {
    await createGroup(name, parentId);
  };

  return (
    <Box>
      <Group
        gap={6}
        p="xs"
        pl={depth * 12 + 8}
        style={{ cursor: 'pointer', borderRadius: 4 }}
        bg={selectedGroupId === group.id ? 'var(--mantine-color-dark-5)' : undefined}
      >
        <IconChevronRight size={12} />
        <IconFolder size={14} />
        <Text size="sm" style={{ flex: 1 }} onClick={() => setSelectedGroup(group.id)}>{group.name}</Text>
        <ActionIcon size="sm" variant="subtle" onClick={() => setAddingChild(v => !v)}>
          <IconPlus size={12} />
        </ActionIcon>
        <ActionIcon size="sm" variant="subtle" onClick={handleRename}>
          <IconPencil size={12} />
        </ActionIcon>
        <ActionIcon size="sm" variant="subtle" color="red" onClick={handleDelete}>
          <IconTrash size={12} />
        </ActionIcon>
      </Group>
      {addingChild && (
        <Group gap={6} pl={depth * 12 + 16} pb="xs">
          <TextInput
            size="xs"
            placeholder="Sub-group name"
            value={childName}
            onChange={(e) => setChildName(e.currentTarget.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') handleAddChild(); }}
            style={{ flex: 1 }}
          />
        </Group>
      )}
    </Box>
  );
}

function RenameGroupForm({ currentName, onSave }: { currentName: string; onSave: (name: string) => void }) {
  const [name, setName] = useState(currentName);
  return (
    <Stack>
      <TextInput label="Group Name" value={name} onChange={(e) => setName(e.currentTarget.value)} />
      <Group justify="flex-end">
        <Button variant="subtle" onClick={() => modals.closeAll()}>Cancel</Button>
        <Button onClick={() => { onSave(name.trim()); modals.closeAll(); }} disabled={!name.trim()}>Save</Button>
      </Group>
    </Stack>
  );
}

export function Sidebar() {
  const { groups, servers, settings, selectedGroupId, setSelectedGroup, createGroup, history, clearHistory } = useStore();
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
        await launchRdp(entry.host, entry.username, settings?.rdp_fullscreen ?? false, settings?.rdp_admin_mode ?? false, entry.server_id ?? undefined, entry.server_name);
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
            bg={selectedGroupId === '__favorites__' ? 'var(--mantine-color-dark-5)' : undefined}
            onClick={() => setSelectedGroup('__favorites__')}
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
          <ActionIcon size="sm" variant="subtle" aria-label="Add group" onClick={handleAddGroup}>
            <IconPlus size={14} />
          </ActionIcon>
        </Group>

        {rootGroups.length === 0 ? (
          <Text size="xs" c="dimmed" p="xs">No groups yet. Create one to organize servers.</Text>
        ) : (
          rootGroups.map(group => (
            <GroupNode key={group.id} group={group} depth={0} />
          ))
        )}
        <TextInput
          size="xs"
          placeholder="New group name + Enter"
          value={newGroupName}
          onChange={(e) => setNewGroupName(e.currentTarget.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') handleAddGroup(); }}
          mt="xs"
        />
      </Stack>
    </Box>
  );
}
