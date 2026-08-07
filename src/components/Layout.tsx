import { AppShell, Group, Text, SegmentedControl } from '@mantine/core';
import { Sidebar } from './Sidebar';
import { ServerList } from './ServerList';
import { SearchBar } from './SearchBar';
import { SshKeys } from './SshKeys';
import { Settings } from './Settings';
import { Credentials } from './Credentials';
import { useState } from 'react';

export type View = 'servers' | 'keys' | 'credentials' | 'settings';

export function Layout() {
  const [view, setView] = useState<View>('servers');

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
        {view === 'servers' && <ServerList />}
        {view === 'keys' && <SshKeys />}
        {view === 'credentials' && <Credentials />}
        {view === 'settings' && <Settings />}
      </AppShell.Main>
    </AppShell>
  );
}
