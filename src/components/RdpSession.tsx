import { Paper, Text, Badge, Button, Stack, Loader, Group } from '@mantine/core';
import { IconDeviceDesktop, IconRefresh } from '@tabler/icons-react';
import type { SessionTab } from '../types';
import { useStore } from '../store/useStore';

interface RdpSessionProps {
  tab: SessionTab;
}

export function RdpSession({ tab }: RdpSessionProps) {
  const { openRdpTab, closeSessionTab } = useStore();

  const statusColor = {
    connecting: 'blue',
    connected: 'green',
    closed: 'red',
  }[tab.status] as string;

  const statusLabel = {
    connecting: 'Connecting...',
    connected: 'Connected',
    closed: 'Disconnected',
  }[tab.status];

  const handleReconnect = async () => {
    // Find the server from store to reconnect
    const { servers } = useStore.getState();
    const server = servers.find(s => s.id === tab.serverId);
    if (server) {
      // Close old tab, open new one
      await closeSessionTab(tab.id);
      await openRdpTab(server);
    }
  };

  return (
    <Paper p="xl" h="100%" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      <Stack align="center" gap="md">
        <IconDeviceDesktop size={48} style={{ opacity: 0.3 }} />
        <Text size="lg" fw={600}>{tab.title}</Text>
        <Group gap="sm">
          <Badge color={statusColor} size="lg">{statusLabel}</Badge>
          {tab.status === 'connecting' && <Loader size="sm" />}
        </Group>
        {tab.status === 'connected' && (
          <Text size="sm" c="dimmed">mstsc.exe is running. Close this tab to disconnect.</Text>
        )}
        {tab.status === 'closed' && (
          <Button leftSection={<IconRefresh size={14} />} onClick={handleReconnect} variant="light">
            Reconnect
          </Button>
        )}
      </Stack>
    </Paper>
  );
}
