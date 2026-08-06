import { AppShell, Group, Text, SegmentedControl } from '@mantine/core';
import { Sidebar } from './Sidebar';
import { ServerList } from './ServerList';
import { SearchBar } from './SearchBar';
import { SshKeys } from './SshKeys';
import { useState } from 'react';

export function Layout() {
  const [view, setView] = useState<'servers' | 'keys'>('servers');

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
              onChange={(v) => setView(v as 'servers' | 'keys')}
              data={[
                { label: 'Servers', value: 'servers' },
                { label: 'SSH Keys', value: 'keys' },
              ]}
            />
          </Group>
          <SearchBar />
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <Sidebar />
      </AppShell.Navbar>

      <AppShell.Main>
        {view === 'servers' ? <ServerList /> : <SshKeys />}
      </AppShell.Main>
    </AppShell>
  );
}
