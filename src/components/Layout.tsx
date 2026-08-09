import { AppShell, Group, Text, SegmentedControl, Tabs, ActionIcon } from '@mantine/core';
import { IconX, IconServer } from '@tabler/icons-react';
import { Sidebar } from './Sidebar';
import { SearchBar } from './SearchBar';
import { SshKeys } from './SshKeys';
import { Settings } from './Settings';
import { Credentials } from './Credentials';
import { Terminal } from './Terminal';
import { RdpSession } from './RdpSession';
import { useStore } from '../store/useStore';
import { useState } from 'react';

export type View = 'servers' | 'keys' | 'credentials' | 'settings';

export function Layout() {
  const [view, setView] = useState<View>('servers');
  const sessionTabs = useStore((s) => s.sessionTabs);
  const activeSessionTabId = useStore((s) => s.activeSessionTabId);
  const focusSessionTab = useStore((s) => s.focusSessionTab);
  const closeSessionTab = useStore((s) => s.closeSessionTab);
  const showTabs = sessionTabs.length > 0;

  return (
    <AppShell
      header={{ height: 50 }}
      navbar={{ width: 280, breakpoint: 'sm' }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between">
          <Group gap="lg">
            <Text fw={700} size="lg">Remote Manager</Text>
            <SegmentedControl
              size="xs"
              value={view}
              onChange={(v) => setView(v as View)}
              data={[
                { label: 'Servers', value: 'servers' },
                { label: 'SSH Keys', value: 'keys' },
                { label: 'Credentials', value: 'credentials' },
                { label: 'Settings', value: 'settings' },
              ]}
            />
          </Group>
          {view === 'servers' && <SearchBar />}
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <Sidebar />
      </AppShell.Navbar>

      <AppShell.Main>
        {/* Servers view: kept mounted always so sessions survive view switches */}
        <div
          style={{
            display: view === 'servers' ? 'block' : 'none',
            height: 'calc(100dvh - var(--app-shell-header-offset, 0px) - var(--app-shell-footer-offset, 0px) - calc(var(--app-shell-padding, 0px) * 2))',
          }}
        >
          {showTabs ? (
            <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
              <Tabs value={activeSessionTabId ?? undefined} onChange={(v) => v && focusSessionTab(v)} variant="outline">
                <Tabs.List>
                  {sessionTabs.map((tab) => (
                    <Tabs.Tab
                      key={tab.id}
                      value={tab.id}
                      rightSection={
                        <ActionIcon
                          size="xs"
                          variant="subtle"
                          aria-label={`Close ${tab.protocol} session ${tab.title}`}
                          onClick={(e) => { e.stopPropagation(); closeSessionTab(tab.id); }}
                        >
                          <IconX size={12} />
                        </ActionIcon>
                      }
                    >
                      <Text size="xs" w={120} truncate>{tab.title}</Text>
                    </Tabs.Tab>
                  ))}
                </Tabs.List>
              </Tabs>
              <div style={{ flex: 1, minHeight: 0 }}>
                {sessionTabs.map((tab) => (
                  <div key={tab.id} style={{ display: tab.id === activeSessionTabId ? 'block' : 'none', height: '100%' }}>
                    {tab.protocol === 'ssh' ? (
                      <Terminal tab={tab} active={tab.id === activeSessionTabId} />
                    ) : (
                      <RdpSession tab={tab} />
                    )}
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <Group c="dimmed" gap="md">
                <IconServer size={32} style={{ opacity: 0.3 }} />
                <Text>Select a server from the sidebar to connect</Text>
              </Group>
            </div>
          )}
        </div>

        {view === 'keys' && <SshKeys />}
        {view === 'credentials' && <Credentials />}
        {view === 'settings' && <Settings />}
      </AppShell.Main>
    </AppShell>
  );
}
