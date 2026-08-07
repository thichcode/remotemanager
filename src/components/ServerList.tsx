import { Group, Text, Stack, ActionIcon, Paper, Badge, Tooltip } from '@mantine/core';
import { IconStar, IconStarFilled, IconPlus, IconPlayerPlay, IconActivity, IconPencil, IconKey } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { ServerForm } from './ServerForm';
import { modals } from '@mantine/modals';
import { launchSsh, launchRdp, pingHost } from '../services/tauri';
import { notifications } from '@mantine/notifications';

export function ServerList() {
  const { servers, credentials, sshKeys, toggleFavorite, selectedGroupId } = useStore();

  const filteredServers = selectedGroupId
    ? servers.filter(s => s.group_id === selectedGroupId)
    : servers;

  const handleConnect = async (server: typeof servers[0]) => {
    try {
      if (server.protocol === 'ssh') {
        await launchSsh(server.host, server.port, server.username, server.id, server.name, server.ssh_key_id);
      } else {
        await launchRdp(server.host, server.username, false, false, server.id, server.name);
      }
    } catch (e: any) {
      notifications.show({ title: 'Error', message: e.toString(), color: 'red' });
    }
  };

  const handlePing = async (host: string) => {
    try {
      const result = await pingHost(host);
      notifications.show({
        title: 'Ping Result',
        message: result,
        color: result.startsWith('Reachable') ? 'green' : 'red',
      });
    } catch (e: any) {
      notifications.show({ title: 'Ping Error', message: e.toString(), color: 'red' });
    }
  };

  const openCreateModal = () => {
    modals.open({
      title: 'Add Server',
      children: <ServerForm />,
      size: 'md',
    });
  };

  const openEditModal = (server: typeof servers[0]) => {
    modals.open({
      title: 'Edit Server',
      children: <ServerForm server={server} />,
      size: 'md',
    });
  };

  const authBadge = (server: typeof servers[0]) => {
    if (server.protocol !== 'ssh') return null;
    if (server.ssh_key_id) {
      const key = sshKeys.find(k => k.id === server.ssh_key_id);
      return <Badge size="sm" variant="light" color="violet" leftSection={<IconKey size={10} />}>Key{key ? `: ${key.name}` : ''}</Badge>;
    }
    if (server.credential_id) {
      const cred = credentials.find(c => c.id === server.credential_id);
      return <Badge size="sm" variant="light" color="grape">Password{cred ? `: ${cred.name}` : ''}</Badge>;
    }
    if (server.username) return <Badge size="sm" variant="light" color="gray">Password</Badge>;
    return null;
  };

  return (
    <Stack gap="md">
      <Group justify="space-between">
        <Text fw={600} size="lg">Servers ({filteredServers.length})</Text>
        <ActionIcon variant="filled" onClick={openCreateModal}>
          <IconPlus size={16} />
        </ActionIcon>
      </Group>

      {filteredServers.length === 0 ? (
        <Paper p="xl" ta="center" withBorder>
          <Text c="dimmed">No servers yet. Click + to add one.</Text>
        </Paper>
      ) : (
        filteredServers.map(server => (
          <Paper key={server.id} p="md" withBorder>
            <Group justify="space-between">
              <Group>
                <ActionIcon
                  size="sm"
                  variant="subtle"
                  onClick={() => toggleFavorite(server.id)}
                >
                  {server.favorite
                    ? <IconStarFilled size={16} style={{ color: '#FFD43B' }} />
                    : <IconStar size={16} />}
                </ActionIcon>
                <div>
                  <Text fw={500}>{server.name}</Text>
                  <Text size="xs" c="dimmed">{server.host}:{server.port}</Text>
                </div>
                <Badge size="sm" variant="light" color={server.protocol === 'ssh' ? 'blue' : 'green'}>
                  {server.protocol.toUpperCase()}
                </Badge>
                {authBadge(server)}
              </Group>
              <Group gap="xs">
                <Tooltip label="Edit">
                  <ActionIcon size="sm" variant="light" onClick={() => openEditModal(server)}>
                    <IconPencil size={14} />
                  </ActionIcon>
                </Tooltip>
                <Tooltip label="Connect">
                  <ActionIcon size="sm" variant="light" onClick={() => handleConnect(server)}>
                    <IconPlayerPlay size={14} />
                  </ActionIcon>
                </Tooltip>
                <Tooltip label="Ping">
                  <ActionIcon size="sm" variant="light" onClick={() => handlePing(server.host)}>
                    <IconActivity size={14} />
                  </ActionIcon>
                </Tooltip>
              </Group>
            </Group>
          </Paper>
        ))
      )}
    </Stack>
  );
}
