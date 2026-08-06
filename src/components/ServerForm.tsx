import { useState, useEffect } from 'react';
import { TextInput, NumberInput, Select, Textarea, Button, Stack, Group } from '@mantine/core';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';

export function ServerForm() {
  const { createServer, groups, credentials, loadCredentials } = useStore();
  const [name, setName] = useState('');
  const [host, setHost] = useState('');
  const [port, setPort] = useState(22);
  const [protocol, setProtocol] = useState<string | null>('ssh');
  const [username, setUsername] = useState('');
  const [groupId, setGroupId] = useState<string | null>(null);
  const [tags, setTags] = useState('');
  const [notes, setNotes] = useState('');
  const [credentialId, setCredentialId] = useState<string | null>(null);

  useEffect(() => {
    loadCredentials();
  }, []);

  const handleSubmit = async () => {
    if (!name.trim() || !host.trim() || !protocol) return;

    await createServer({
      name: name.trim(),
      host: host.trim(),
      port,
      protocol: protocol as 'ssh' | 'rdp',
      username: username.trim(),
      group_id: groupId,
      tags: tags.trim(),
      notes: notes.trim(),
      favorite: false,
      credential_id: credentialId,
    });

    modals.closeAll();
  };

  return (
    <Stack>
      <TextInput
        label="Name"
        placeholder="My Server"
        value={name}
        onChange={(e) => setName(e.currentTarget.value)}
        required
      />
      <Group grow>
        <TextInput
          label="Host / IP"
          placeholder="192.168.1.100"
          value={host}
          onChange={(e) => setHost(e.currentTarget.value)}
          required
        />
        <NumberInput
          label="Port"
          value={port}
          onChange={(v) => setPort(Number(v))}
          min={1}
          max={65535}
        />
      </Group>
      <Group grow>
        <Select
          label="Protocol"
          data={[{ value: 'ssh', label: 'SSH' }, { value: 'rdp', label: 'RDP' }]}
          value={protocol}
          onChange={setProtocol}
          required
        />
        <TextInput
          label="Username"
          placeholder="root"
          value={username}
          onChange={(e) => setUsername(e.currentTarget.value)}
        />
      </Group>
      <Select
        label="Group"
        data={groups.map(g => ({ value: g.id, label: g.name }))}
        value={groupId}
        onChange={setGroupId}
        clearable
        searchable
      />
      <Select
        label="Credential Profile"
        data={credentials.map(c => ({ value: c.id, label: c.name }))}
        value={credentialId}
        onChange={setCredentialId}
        clearable
        searchable
      />
      <TextInput
        label="Tags"
        placeholder="k8s, production"
        value={tags}
        onChange={(e) => setTags(e.currentTarget.value)}
      />
      <Textarea
        label="Notes"
        placeholder="Optional notes..."
        value={notes}
        onChange={(e) => setNotes(e.currentTarget.value)}
        autosize
        minRows={2}
      />
      <Group justify="flex-end">
        <Button variant="subtle" onClick={() => modals.closeAll()}>Cancel</Button>
        <Button onClick={handleSubmit} disabled={!name.trim() || !host.trim() || !protocol}>
          Save Server
        </Button>
      </Group>
    </Stack>
  );
}
