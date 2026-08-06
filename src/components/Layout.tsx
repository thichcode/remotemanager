import { AppShell, Group, Text } from '@mantine/core';
import { Sidebar } from './Sidebar';
import { ServerList } from './ServerList';
import { SearchBar } from './SearchBar';

export function Layout() {
  return (
    <AppShell
      header={{ height: 50 }}
      navbar={{ width: 250, breakpoint: 'sm' }}
      padding="md"
    >
      <AppShell.Header>
        <Group h="100%" px="md" justify="space-between">
          <Text fw={700} size="lg">Remote Manager</Text>
          <SearchBar />
        </Group>
      </AppShell.Header>

      <AppShell.Navbar p="md">
        <Sidebar />
      </AppShell.Navbar>

      <AppShell.Main>
        <ServerList />
      </AppShell.Main>
    </AppShell>
  );
}
