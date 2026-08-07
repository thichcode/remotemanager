import { useEffect, useState } from 'react';
import { Group, Text, Stack, ActionIcon, Paper, Badge, Tooltip, Button, Menu, Modal, SegmentedControl } from '@mantine/core';
import {
  IconStar, IconStarFilled, IconPlus, IconPlayerPlay, IconActivity, IconPencil,
  IconKey, IconCopy, IconTrash, IconUpload, IconDownload, IconDots,
} from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { ServerForm } from './ServerForm';
import { modals } from '@mantine/modals';
import { launchSsh, launchRdp, pingHost, importCsv, importJson, exportCsv, exportJson, cloneServer } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import { open, save } from '@tauri-apps/plugin-dialog';
import type { Protocol } from '../types';

export function ServerList() {
  const { servers, credentials, sshKeys, toggleFavorite, selectedGroupId, deleteServer, loadServers } = useStore();
  const [protocolFilter, setProtocolFilter] = useState<Protocol | 'all'>('all');
  const [exportModalOpen, setExportModalOpen] = useState(false);
  const [exportFormat, setExportFormat] = useState<'csv' | 'json'>('csv');
  const [visibleCount, setVisibleCount] = useState(100);
  const PAGE_SIZE = 100;

  useEffect(() => {
    const handler = (e: Event) => {
      setProtocolFilter((e as CustomEvent).detail as Protocol | 'all');
    };
    window.addEventListener('rm:filter-protocol', handler);
    return () => window.removeEventListener('rm:filter-protocol', handler);
  }, []);

  useEffect(() => {
    setVisibleCount(PAGE_SIZE);
  }, [protocolFilter, selectedGroupId, servers.length]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n') {
        e.preventDefault();
        openCreateModal();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [servers, credentials, sshKeys]);

  const filteredServers = servers.filter(s => {
    if (selectedGroupId && selectedGroupId !== '__favorites__' && s.group_id !== selectedGroupId) return false;
    if (protocolFilter !== 'all' && s.protocol !== protocolFilter) return false;
    return true;
  });

  const visibleServers = filteredServers.slice(0, visibleCount);
  const hasMore = filteredServers.length > visibleCount;

  const handleConnect = async (server: typeof servers[0]) => {
    try {
      if (server.protocol === 'ssh') {
        await launchSsh(server.host, server.port, server.username, server.id, server.name, server.ssh_key_id, server.credential_id);
      } else {
        await launchRdp(server.host, server.username, false, false, server.id, server.name, server.credential_id);
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

  const handleClone = async (server: typeof servers[0]) => {
    try {
      await cloneServer(server.id);
      await loadServers();
      notifications.show({ title: 'Cloned', message: `"${server.name}" duplicated`, color: 'green' });
    } catch (e: any) {
      notifications.show({ title: 'Clone Failed', message: e.toString(), color: 'red' });
    }
  };

  const handleDelete = (server: typeof servers[0]) => {
    modals.openConfirmModal({
      title: 'Delete Server',
      children: <Text size="sm">Delete "{server.name}"? This cannot be undone.</Text>,
      labels: { confirm: 'Delete', cancel: 'Cancel' },
      confirmProps: { color: 'red' },
      onConfirm: async () => {
        try {
          await deleteServer(server.id);
          notifications.show({ title: 'Deleted', message: `"${server.name}" removed`, color: 'green' });
        } catch (e: any) {
          notifications.show({ title: 'Delete Failed', message: e.toString(), color: 'red' });
        }
      },
    });
  };

  const handleImport = async () => {
    try {
      const path = await open({
        multiple: false,
        filters: [{ name: 'Import Files', extensions: ['csv', 'json'] }],
      });
      if (path) {
        const isJson = path.toLowerCase().endsWith('.json');
        const result = isJson ? await importJson(path) : await importCsv(path);
        await loadServers();
        notifications.show({
          title: 'Import Complete',
          message: `${result.imported} server(s) imported${result.errors.length ? `, ${result.errors.length} error(s)` : ''}`,
          color: result.errors.length ? 'yellow' : 'green',
        });
      }
    } catch (e: any) {
      notifications.show({ title: 'Import Failed', message: e.toString(), color: 'red' });
    }
  };

  const handleExport = async () => {
    try {
      const path = await save({
        defaultPath: `remote-managers.${exportFormat}`,
        filters: [{ name: 'Export Files', extensions: [exportFormat] }],
      });
      if (path) {
        if (exportFormat === 'csv') await exportCsv(path);
        else await exportJson(path);
        setExportModalOpen(false);
        notifications.show({ title: 'Exported', message: 'Data exported successfully', color: 'green' });
      }
    } catch (e: any) {
      notifications.show({ title: 'Export Failed', message: e.toString(), color: 'red' });
    }
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

  const tagBadges = (server: typeof servers[0]) => {
    if (!server.tags) return null;
    return server.tags.split(',').map(t => t.trim()).filter(Boolean).map(tag => (
      <Badge key={tag} size="xs" variant="dot" color="cyan">{tag}</Badge>
    ));
  };

  return (
    <Stack gap="md">
      <Group justify="space-between">
        <Text fw={600} size="lg">Servers ({filteredServers.length})</Text>
        <Group gap="xs">
          <Tooltip label="Import CSV/JSON">
            <Button size="xs" variant="light" leftSection={<IconUpload size={14} />} onClick={handleImport}>Import</Button>
          </Tooltip>
          <Tooltip label="Export CSV/JSON">
            <Button size="xs" variant="light" leftSection={<IconDownload size={14} />} onClick={() => setExportModalOpen(true)}>Export</Button>
          </Tooltip>
          <ActionIcon variant="filled" onClick={openCreateModal}>
            <IconPlus size={16} />
          </ActionIcon>
        </Group>
      </Group>

      {filteredServers.length === 0 ? (
        <Paper p="xl" ta="center" withBorder>
          <Text c="dimmed">No servers match. Click + to add one.</Text>
        </Paper>
      ) : (
        visibleServers.map(server => (
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
                  <Group gap="xs">
                    <Text fw={500}>{server.name}</Text>
                    {tagBadges(server)}
                  </Group>
                  <Text size="xs" c="dimmed">{server.host}:{server.port}</Text>
                  {server.description && (
                    <Text size="xs" c="dimmed" lineClamp={1}>{server.description}</Text>
                  )}
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
                <Menu position="bottom-end">
                  <Menu.Target>
                    <ActionIcon size="sm" variant="light">
                      <IconDots size={14} />
                    </ActionIcon>
                  </Menu.Target>
                  <Menu.Dropdown>
                    <Menu.Item leftSection={<IconCopy size={14} />} onClick={() => handleClone(server)}>Clone</Menu.Item>
                    <Menu.Item leftSection={<IconTrash size={14} />} color="red" onClick={() => handleDelete(server)}>Delete</Menu.Item>
                  </Menu.Dropdown>
                </Menu>
              </Group>
            </Group>
          </Paper>
        ))
      )}

      {hasMore && (
        <Group justify="center">
          <Button
            variant="light"
            size="xs"
            onClick={() => setVisibleCount(v => v + PAGE_SIZE)}
          >
            Load more ({filteredServers.length - visibleCount} remaining)
          </Button>
        </Group>
      )}

      <Modal opened={exportModalOpen} onClose={() => setExportModalOpen(false)} title="Export Servers" centered>
        <Stack>
          <Text size="sm" c="dimmed">Choose an export format. All servers will be written to the selected file.</Text>
          <SegmentedControl
            value={exportFormat}
            onChange={(v) => setExportFormat(v as 'csv' | 'json')}
            data={[
              { label: 'CSV', value: 'csv' },
              { label: 'JSON', value: 'json' },
            ]}
          />
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setExportModalOpen(false)}>Cancel</Button>
            <Button onClick={handleExport}>Choose File & Export</Button>
          </Group>
        </Stack>
      </Modal>
    </Stack>
  );
}
