import { AppShell, Group, Text, SegmentedControl, Tabs, ActionIcon } from '@mantine/core';
import { IconX } from '@tabler/icons-react';
import { Sidebar } from './Sidebar';
import { ServerList } from './ServerList';
import { SearchBar } from './SearchBar';
import { SshKeys } from './SshKeys';
import { Settings } from './Settings';
import { Credentials } from './Credentials';
import { Terminal } from './Terminal';
import { useStore } from '../store/useStore';
import { useState } from 'react';

export type View = 'servers' | 'keys' | 'credentials' | 'settings';

export function Layout() {
  const [view, setView] = useState<View>('servers');
  const { terminalTabs, activeTerminalTabId, focusTerminalTab, closeTerminalTab } = useStore();
  const showTerminal = view === 'servers' && terminalTabs.length > 0;

  return (
    <AppShell
      header={{ height: 50 }}
      navbar={{ width: 250, breakpoint: 'sm' }}
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
        {/* Servers view: kept mounted always so terminal sessions survive view switches */}
        <div style={{ display: view === 'servers' ? 'flex' : 'none', gap: 16, height: '100%', minHeight: 0 }}>
          <div style={{ flex: showTerminal ? '0 0 40%' : '1 1 auto', minWidth: 0, overflowY: 'auto' }}>
            <ServerList />
          </div>
          {showTerminal && (
            <div style={{ flex: '1 1 auto', minWidth: 0, display: 'flex', flexDirection: 'column' }}>
              <Tabs value={activeTerminalTabId ?? undefined} onChange={(v) => v && focusTerminalTab(v)} variant="outline">
                <Tabs.List>
                  {terminalTabs.map((tab) => (
                    <Tabs.Tab
                      key={tab.id}
                      value={tab.id}
                      rightSection={
                        <ActionIcon
                          size="xs"
                          variant="subtle"
                          aria-label={`Close terminal ${tab.title}`}
                          onClick={(e) => { e.stopPropagation(); closeTerminalTab(tab.id); }}
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
                {terminalTabs.map((tab) => (
                  <div key={tab.id} style={{ display: tab.id === activeTerminalTabId ? 'block' : 'none', height: '100%' }}>
                    <Terminal tab={tab} active={tab.id === activeTerminalTabId} />
                  </div>
                ))}
              </div>
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
