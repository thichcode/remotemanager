import { useState } from 'react';
import { Stack, Group, Text, Paper, Button, ActionIcon, Modal, TextInput, PasswordInput } from '@mantine/core';
import { IconTrash, IconUpload } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { open } from '@tauri-apps/plugin-dialog';
import { notifications } from '@mantine/notifications';

export function SshKeys() {
  const { sshKeys, deleteSshKey, importSshKey } = useStore();
  const [importOpen, setImportOpen] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [passphrase, setPassphrase] = useState('');

  const handleImport = async () => {
    try {
      const selected = await open({ multiple: false, filters: [{ name: 'SSH Keys', extensions: ['key', 'pem', 'pub'] }] });
      if (selected) {
        setSelectedPath(selected);
        setImportOpen(true);
        setName('');
        setPassphrase('');
      }
    } catch (e: any) {
      notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
    }
  };

  const handleConfirmImport = async () => {
    if (!selectedPath) return;
    try {
      await importSshKey(selectedPath, name.trim(), passphrase || undefined);
      setImportOpen(false);
      setSelectedPath(null);
      notifications.show({ title: 'Imported', message: `Key "${name}" imported`, color: 'green' });
    } catch (e: any) {
      notifications.show({ title: 'Import Failed', message: e.toString(), color: 'red' });
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deleteSshKey(id);
    } catch (e: any) {
      notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
    }
  };

  return (
    <Stack gap="md">
      <Group justify="space-between">
        <Text fw={600} size="lg">SSH Keys</Text>
        <Button leftSection={<IconUpload size={14} />} onClick={handleImport}>Import Key</Button>
      </Group>
      {sshKeys.length === 0 ? (
        <Paper p="xl" ta="center" withBorder><Text c="dimmed">No SSH keys imported.</Text></Paper>
      ) : (
        sshKeys.map(k => (
          <Paper key={k.id} p="md" withBorder>
            <Group justify="space-between">
              <div>
                <Text fw={500}>{k.name}</Text>
                <Text size="xs" c="dimmed">Added {k.created_at}</Text>
              </div>
              <ActionIcon color="red" variant="subtle" onClick={() => handleDelete(k.id)}>
                <IconTrash size={14} />
              </ActionIcon>
            </Group>
          </Paper>
        ))
      )}

      <Modal opened={importOpen} onClose={() => setImportOpen(false)} title="Import SSH Key" centered>
        <Stack>
          <TextInput label="Key Name" placeholder="My production key" required value={name} onChange={(e) => setName(e.currentTarget.value)} />
          <PasswordInput label="Passphrase (optional)" value={passphrase} onChange={(e) => setPassphrase(e.currentTarget.value)} />
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setImportOpen(false)}>Cancel</Button>
            <Button onClick={handleConfirmImport} disabled={!name.trim()}>Import</Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
