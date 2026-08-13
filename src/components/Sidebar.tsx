import { Box, Text, Group, ActionIcon, Stack, Divider, TextInput, Button, Modal, SegmentedControl, Notification, Loader } from '@mantine/core';
import { IconPlus, IconServer, IconStar, IconFolder, IconChevronRight, IconDownload, IconClock, IconRotate } from '@tabler/icons-react';
import { useStore } from '../store/useStore';
import { useState, useEffect, useCallback } from 'react';
import { exportCsv, exportJson } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import { modals } from '@mantine/modals';
import { save } from '@tauri-apps/plugin-dialog';
import { GroupServerTree } from './GroupServerTree';
import { ServerContextMenu } from './ServerContextMenu';
import type { ContextMenuState } from './ServerContextMenu';
import { ServerForm } from './ServerForm';
import { SftpBrowser } from './SftpBrowser';
import type { Server } from '../types';

const FAVORITES_ID = '__favorites__';
const UNGROUPED_ID = '__ungrouped__';

export function Sidebar() {
  const groups = useStore((s) => s.groups);
  const servers = useStore((s) => s.servers);
  const isLoading = useStore((s) => s.isLoading);
  const selectedGroupId = useStore((s) => s.selectedGroupId);
  const setSelectedGroup = useStore((s) => s.setSelectedGroup);
  const createGroup = useStore((s) => s.createGroup);
  const clearHistory = useStore((s) => s.clearHistory);
  const expandedGroups = useStore((s) => s.expandedGroups);
  const toggleGroupExpanded = useStore((s) => s.toggleGroupExpanded);
  const activeSessionTabId = useStore((s) => s.activeSessionTabId);
  const sessionTabs = useStore((s) => s.sessionTabs);
  const recentServers = useStore((s) => s.recentServers);
  const undoAction = useStore((s) => s.undoAction);
  const performUndo = useStore((s) => s.performUndo);
  const clearUndo = useStore((s) => s.clearUndo);
  const [newGroupName, setNewGroupName] = useState('');
  const [exportModalOpen, setExportModalOpen] = useState(false);
  const [exportFormat, setExportFormat] = useState<'csv' | 'json'>('csv');
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const openContextMenu = useCallback((server: Server, x: number, y: number) => {
    setContextMenu({ x, y, server });
  }, []);

  const activeTab = sessionTabs.find((t) => t.id === activeSessionTabId && t.protocol === 'ssh');
  const activeServer = activeTab?.serverId ? servers.find((s) => s.id === activeTab!.serverId) ?? null : null;

  useEffect(() => {
    if (undoAction) {
      const timeout = setTimeout(() => clearUndo(), 5000);
      return () => clearTimeout(timeout);
    }
  }, [undoAction, clearUndo]);

  const favorites = servers.filter(s => s.favorite);
  const rootGroups = groups.filter(g => !g.parent_id);

  // Group servers by group_id
  const groupedServers = new Map<string, Server[]>();
  for (const s of servers) {
    const gid = s.group_id ?? UNGROUPED_ID;
    if (!groupedServers.has(gid)) groupedServers.set(gid, []);
    groupedServers.get(gid)!.push(s);
  }
  const ungroupedServers = groupedServers.get(UNGROUPED_ID) ?? [];

  const handleAddGroup = async () => {
    if (newGroupName.trim()) {
      await createGroup(newGroupName.trim());
      setNewGroupName('');
    }
  };

  return (
    <Box>
      <ServerContextMenu state={contextMenu} onClose={() => setContextMenu(null)} />
      {isLoading && (
        <Group justify="center" py="xs"><Loader size="xs" /><Text size="xs" c="dimmed">Loading...</Text></Group>
      )}
      {undoAction && (
        <Notification
          title={`"${undoAction.data.name}" deleted`}
          color="blue"
          withCloseButton
          onClose={clearUndo}
          p="xs"
          mb="xs"
        >
          <Group gap="xs">
            <Text size="xs">Undo?</Text>
            <Button size="xs" variant="light" leftSection={<IconRotate size={12} />} onClick={performUndo}>Undo</Button>
          </Group>
        </Notification>
      )}
      <Stack gap={4}>
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">Quick Access</Text>
        <Group
          gap={8}
          p="xs"
          style={{ cursor: 'pointer', borderRadius: 4 }}
          bg={selectedGroupId === null ? 'var(--mantine-color-dark-5)' : undefined}
          onClick={() => setSelectedGroup(null)}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedGroup(null); } }}
          tabIndex={0}
          role="button"
          aria-label="Show all servers"
        >
          <IconServer size={16} />
          <Text size="sm">All Servers ({servers.length})</Text>
        </Group>
        {favorites.length > 0 && (
          <Group
            gap={8}
            p="xs"
            style={{ cursor: 'pointer', borderRadius: 4 }}
            bg={selectedGroupId === FAVORITES_ID ? 'var(--mantine-color-dark-5)' : undefined}
            onClick={() => setSelectedGroup(FAVORITES_ID)}
            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedGroup(FAVORITES_ID); } }}
            tabIndex={0}
            role="button"
            aria-label="Show favorite servers"
          >
            <IconStar size={16} />
            <Text size="sm">Favorites ({favorites.length})</Text>
          </Group>
        )}
        {recentServers.length > 0 && (
          <Group
            gap={8}
            p="xs"
            style={{ cursor: 'pointer', borderRadius: 4 }}
            bg={selectedGroupId === '__recent__' ? 'var(--mantine-color-dark-5)' : undefined}
            onClick={() => setSelectedGroup('__recent__')}
            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedGroup('__recent__'); } }}
            tabIndex={0}
            role="button"
            aria-label="Show recent servers"
          >
            <IconClock size={16} />
            <Text size="sm">Recent ({recentServers.length})</Text>
          </Group>
        )}
      </Stack>

      <Divider my="md" />

      <SftpBrowser
        serverId={activeServer?.id ?? null}
        serverHost={activeServer?.host ?? null}
        onClearHistory={() => {
          modals.openConfirmModal({
            title: 'Clear History',
            children: <Text size="sm">This will remove all recent connection history. This cannot be undone.</Text>,
            labels: { confirm: 'Clear', cancel: 'Cancel' },
            confirmProps: { color: 'red' },
            onConfirm: () => clearHistory(),
          });
        }}
      />

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
          rootGroups.map(group => {
            const isExpanded = expandedGroups[group.id] ?? false;
            const groupSrvs = groupedServers.get(group.id) ?? [];
            return (
              <Box key={group.id}>
                <Group
                  gap={6}
                  p="xs"
                  style={{ cursor: 'pointer', borderRadius: 4 }}
                  bg={selectedGroupId === group.id ? 'var(--mantine-color-dark-5)' : undefined}
                  onClick={() => { setSelectedGroup(group.id); toggleGroupExpanded(group.id); }}
                  onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedGroup(group.id); toggleGroupExpanded(group.id); } }}
                  tabIndex={0}
                  role="button"
                  aria-expanded={isExpanded}
                  aria-label={`${group.name} group`}
                >
                  <IconChevronRight
                    size={12}
                    style={{ transform: isExpanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.15s' }}
                  />
                  <IconFolder size={14} />
                  <Text size="sm" style={{ flex: 1 }}>{group.name}</Text>
                </Group>
                {isExpanded && <GroupServerTree servers={groupSrvs} onOpenMenu={openContextMenu} />}
              </Box>
            );
          })
        )}

        {ungroupedServers.length > 0 && (
          <Box>
            <Text size="xs" c="dimmed" p="xs">Ungrouped</Text>
            <GroupServerTree servers={ungroupedServers} onOpenMenu={openContextMenu} />
          </Box>
        )}

        {selectedGroupId === '__recent__' && recentServers.length > 0 && (
          <Box>
            <Text size="xs" c="dimmed" p="xs">Recently connected</Text>
            <GroupServerTree servers={recentServers} onOpenMenu={openContextMenu} />
          </Box>
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

      <Divider my="md" />

      <Button
        size="xs"
        variant="light"
        leftSection={<IconPlus size={14} />}
        fullWidth
        onClick={() => modals.open({ title: 'Add Server', children: <ServerForm />, size: 'lg' })}
      >
        Add Server
      </Button>

      <Button
        size="xs"
        variant="light"
        leftSection={<IconDownload size={14} />}
        fullWidth
        mt="xs"
        onClick={() => setExportModalOpen(true)}
      >
        Export
      </Button>

      <Modal opened={exportModalOpen} onClose={() => setExportModalOpen(false)} title="Export Servers" centered>
        <Stack>
          <Text size="sm" c="dimmed">Choose an export format. All servers will be written to the selected file.</Text>
          <SegmentedControl
            value={exportFormat}
            onChange={(v) => setExportFormat(v as 'csv' | 'json')}
            data={[{ label: 'CSV', value: 'csv' }, { label: 'JSON', value: 'json' }]}
          />
          <Group justify="flex-end">
            <Button variant="subtle" onClick={() => setExportModalOpen(false)}>Cancel</Button>
            <Button onClick={async () => {
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
              } catch (e: unknown) {
                const msg = e instanceof Error ? e.message : String(e);
                notifications.show({ title: 'Export Failed', message: msg, color: 'red' });
              }
            }}>Choose File & Export</Button>
          </Group>
        </Stack>
      </Modal>
    </Box>
  );
}
