import { useState, useEffect, useRef } from 'react';
import { TextInput, NumberInput, Select, Textarea, Button, Stack, Group, Text } from '@mantine/core';
import { useStore } from '../store/useStore';
import { modals } from '@mantine/modals';
import type { Server } from '../types';

export function ServerForm({ server }: { server?: Server }) {
  const { createServer, updateServer, groups, credentials, sshKeys, loadCredentials, loadSshKeys } = useStore();
  const [name, setName] = useState(server?.name ?? '');
  const [host, setHost] = useState(server?.host ?? '');
  const [port, setPort] = useState(server?.port ?? 22);
  const [protocol, setProtocol] = useState<string | null>(server?.protocol ?? 'ssh');
  const [username, setUsername] = useState(server?.username ?? '');
  const [groupId, setGroupId] = useState<string | null>(server?.group_id ?? null);
  const [tags, setTags] = useState(server?.tags ?? '');
  const [notes, setNotes] = useState(server?.notes ?? '');
  const [description, setDescription] = useState(server?.description ?? '');
  const [credentialId, setCredentialId] = useState<string | null>(server?.credential_id ?? null);
  const [sshKeyId, setSshKeyId] = useState<string | null>(server?.ssh_key_id ?? null);
  const initialValuesRef = useRef({
    name: server?.name ?? '',
    host: server?.host ?? '',
    port: server?.port ?? 22,
    protocol: server?.protocol ?? 'ssh',
    username: server?.username ?? '',
    group_id: server?.group_id ?? null,
    tags: server?.tags ?? '',
    notes: server?.notes ?? '',
    description: server?.description ?? '',
    credential_id: server?.credential_id ?? null,
    ssh_key_id: server?.ssh_key_id ?? null,
  });

  const isDirty = name !== initialValuesRef.current.name ||
    host !== initialValuesRef.current.host ||
    port !== initialValuesRef.current.port ||
    protocol !== initialValuesRef.current.protocol ||
    username !== initialValuesRef.current.username ||
    groupId !== initialValuesRef.current.group_id ||
    tags !== initialValuesRef.current.tags ||
    notes !== initialValuesRef.current.notes ||
    description !== initialValuesRef.current.description ||
    credentialId !== initialValuesRef.current.credential_id ||
    sshKeyId !== initialValuesRef.current.ssh_key_id;

  useEffect(() => {
    loadCredentials();
    if (useStore.getState().sshKeys.length === 0) loadSshKeys();
  }, []);

  const handleCancel = () => {
    if (isDirty) {
      modals.openConfirmModal({
        title: 'Discard changes?',
        children: <Text size="sm">You have unsaved changes.</Text>,
        labels: { confirm: 'Discard', cancel: 'Keep Editing' },
        confirmProps: { color: 'red' },
        onConfirm: () => modals.closeAll(),
      });
    } else {
      modals.closeAll();
    }
  };

  const handleSubmit = async () => {
    if (!name.trim() || !host.trim() || !protocol) return;

    const payload = {
      name: name.trim(),
      host: host.trim(),
      port,
      protocol: protocol as 'ssh' | 'rdp',
      username: username.trim(),
      group_id: groupId,
      tags: tags.trim(),
      notes: notes.trim(),
      description: description.trim(),
      favorite: server?.favorite ?? false,
      credential_id: credentialId,
      ssh_key_id: sshKeyId,
      last_connected_at: null,
    };

    if (server) {
      await updateServer(server.id, payload);
    } else {
      await createServer(payload);
    }

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
        label="Credential Profile (password)"
        description={sshKeyId ? 'Clear the SSH Key to use a saved password.' : 'Saved password used when no SSH Key is set.'}
        data={credentials.map(c => ({ value: c.id, label: c.name }))}
        value={credentialId}
        onChange={(v) => {
          setCredentialId(v);
          if (v) setSshKeyId(null);
        }}
        clearable
        searchable
      />
      {protocol === 'ssh' && (
        <Select
          label="SSH Key"
          description={credentialId ? 'Selecting a key switches this server to key auth and clears the saved password.' : 'Uses the private key instead of a password.'}
          placeholder="None"
          data={sshKeys.map(k => ({ value: k.id, label: k.name }))}
          value={sshKeyId}
          onChange={(v) => {
            setSshKeyId(v);
            if (v) setCredentialId(null);
          }}
          clearable
          searchable
        />
      )}
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
      <TextInput
        label="Description"
        placeholder="Short description shown in the list"
        value={description}
        onChange={(e) => setDescription(e.currentTarget.value)}
      />
      <Group justify="flex-end">
        <Button variant="subtle" onClick={handleCancel}>Cancel</Button>
        <Button onClick={handleSubmit} disabled={!name.trim() || !host.trim() || !protocol}>
          {server ? 'Save Changes' : 'Save Server'}
        </Button>
      </Group>
    </Stack>
  );
}
