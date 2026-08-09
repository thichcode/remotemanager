import { useState } from 'react';
import {
  Stack, Group, Text, Paper, Button, ActionIcon, Modal, TextInput,
  PasswordInput, Badge, Tooltip,
} from '@mantine/core';
import { IconTrash, IconPlus, IconPencil, IconActivity } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { testCredential } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import { modals } from '@mantine/modals';
import type { Credential } from '../types';

export function Credentials() {
  const { credentials, createCredential, updateCredential, deleteCredential } = useStore();
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState<Credential | null>(null);
  const [name, setName] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [testHostMap, setTestHostMap] = useState<Record<string, string>>({});
  const [testingId, setTestingId] = useState<string | null>(null);

  const openCreate = () => {
    setEditing(null);
    setName('');
    setUsername('');
    setPassword('');
    setTestHostMap({});
    setModalOpen(true);
  };

  const openEdit = (cred: Credential) => {
    setEditing(cred);
    setName(cred.name);
    setUsername(cred.username);
    setPassword('');
    setTestHostMap({});
    setModalOpen(true);
  };

  const handleSave = async () => {
    if (!name.trim() || !username.trim()) return;
    try {
      if (editing) {
        await updateCredential(editing.id, name.trim(), username.trim(), password || undefined);
      } else {
        if (!password) {
          notifications.show({ title: 'Error', message: 'Password is required', color: 'red' });
          return;
        }
        await createCredential(name.trim(), username.trim(), password);
      }
      setModalOpen(false);
      notifications.show({ title: 'Saved', message: 'Credential updated', color: 'green' });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      notifications.show({ title: 'Error', message: msg, color: 'red' });
    }
  };

  const handleDelete = async (cred: Credential) => {
    modals.openConfirmModal({
      title: `Delete "${cred.name}"`,
      children: <Text size="sm">This credential will be removed from all servers that use it. This cannot be undone.</Text>,
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await deleteCredential(cred.id);
          notifications.show({ title: 'Deleted', message: `Credential "${cred.name}" removed`, color: 'green' });
        } catch (e: unknown) {
          const msg = e instanceof Error ? e.message : String(e);
          notifications.show({ title: 'Error', message: msg, color: 'red' });
        }
      },
    });
  };

  const handleTest = async (cred: Credential) => {
    const hostVal = testHostMap[cred.id] ?? '';
    if (!hostVal.trim()) {
      notifications.show({ title: 'Test Credential', message: 'Enter a host/IP to test against', color: 'yellow' });
      return;
    }
    setTestingId(cred.id);
    try {
      const result = await testCredential(cred.id, hostVal.trim());
      notifications.show({ title: 'Credential Test', message: result, color: result.includes('Reachable') ? 'green' : 'yellow' });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      notifications.show({ title: 'Test Failed', message: msg, color: 'red' });
    } finally {
      setTestingId(null);
    }
  };

  return (
    <Stack gap="md">
      <Group justify="space-between">
        <Text fw={600} size="lg">Credentials Vault</Text>
        <Button leftSection={<IconPlus size={14} />} onClick={openCreate}>Add Credential</Button>
      </Group>

      <Text size="sm" c="dimmed">
        Passwords are encrypted with Windows DPAPI and stored locally.
      </Text>

      {credentials.length === 0 ? (
        <Paper p="xl" ta="center" withBorder>
          <Text c="dimmed">No credentials saved yet.</Text>
        </Paper>
      ) : (
        credentials.map(cred => (
          <Paper key={cred.id} p="md" withBorder>
            <Group justify="space-between">
              <div>
                <Group gap="sm">
                  <Text fw={500}>{cred.name}</Text>
                  <Badge size="sm" variant="light" color="grape">{cred.username}</Badge>
                </Group>
                <Text size="xs" c="dimmed">Created {cred.created_at}</Text>
              </div>
              <Group gap="xs">
                <Tooltip label="Edit">
                  <ActionIcon size="sm" variant="light" aria-label="Edit credential" onClick={() => openEdit(cred)}>
                    <IconPencil size={14} />
                  </ActionIcon>
                </Tooltip>
                <Tooltip label="Delete">
                  <ActionIcon size="sm" color="red" variant="light" aria-label="Delete credential" onClick={() => handleDelete(cred)}>
                    <IconTrash size={14} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            </Group>
            <Group mt="sm" align="center">
              <TextInput
                placeholder="Host to test against (e.g. 192.168.1.100)"
                value={testHostMap[cred.id] ?? ''}
                onChange={(e) => setTestHostMap(prev => ({ ...prev, [cred.id]: e.currentTarget.value }))}
                size="xs"
                style={{ flex: 1 }}
              />
              <Button size="xs" variant="light" leftSection={<IconActivity size={14} />} loading={testingId === cred.id} onClick={() => handleTest(cred)}>
                Test
              </Button>
            </Group>
          </Paper>
        ))
      )}

      <Modal opened={modalOpen} onClose={() => setModalOpen(false)} title={editing ? 'Edit Credential' : 'Add Credential'} centered>
        <Stack>
          <TextInput label="Name" placeholder="Production root" required value={name} onChange={(e) => setName(e.currentTarget.value)} />
          <TextInput label="Username" placeholder="root" required value={username} onChange={(e) => setUsername(e.currentTarget.value)} />
          <PasswordInput
            label={editing ? 'New Password (leave blank to keep current)' : 'Password'}
            required={!editing}
            value={password}
            onChange={(e) => setPassword(e.currentTarget.value)}
          />
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setModalOpen(false)}>Cancel</Button>
            <Button onClick={handleSave} disabled={!name.trim() || !username.trim() || (!editing && !password)}>
              {editing ? 'Save Changes' : 'Add Credential'}
            </Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
