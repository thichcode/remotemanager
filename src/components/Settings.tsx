import { useEffect, useState } from 'react';
import { Stack, NumberInput, Select, Switch, Text, Divider, Button, Badge, Group } from '@mantine/core';
import { useStore } from '../store/useStore';
import { open, save } from '@tauri-apps/plugin-dialog';
import { backup, restore, isPortable } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import { UpdaterPanel } from './UpdaterPanel';

export function Settings() {
  const { settings, loadSettings, updateSettings } = useStore();
  const [portable, setPortable] = useState<boolean | null>(null);

  useEffect(() => {
    loadSettings();
    isPortable().then(setPortable).catch(() => setPortable(false));
  }, []);

  const handleBackup = async () => {
    try {
      const path = await save({ defaultPath: 'remote-manager-backup.rmbackup', filters: [{ name: 'Remote Manager Backup', extensions: ['rmbackup'] }] });
      if (path) {
        const summary = await backup(path);
        notifications.show({ title: 'Backup Created', message: `${summary.db_size} bytes DB, ${summary.keys_count} keys`, color: 'green' });
      }
    } catch (e: any) {
      notifications.show({ title: 'Backup Failed', message: e.toString(), color: 'red' });
    }
  };

  const handleRestore = async () => {
    try {
      const path = await open({ multiple: false, filters: [{ name: 'Remote Manager Backup', extensions: ['rmbackup'] }] });
      if (path) {
        await restore(path);
        notifications.show({ title: 'Restore Complete', message: 'Data restored. Restart the app to apply.', color: 'green' });
      }
    } catch (e: any) {
      notifications.show({ title: 'Restore Failed', message: e.toString(), color: 'red' });
    }
  };

  if (!settings) return <Text>Loading...</Text>;

  return (
    <Stack gap="md" maw={500}>
      <Group justify="space-between">
        <Text size="lg" fw={600}>Settings</Text>
        {portable !== null && (
          <Badge color={portable ? 'teal' : 'gray'}>{portable ? 'Portable Mode' : 'Installed Mode'}</Badge>
        )}
      </Group>

      <Divider label="Appearance" labelPosition="center" />
      <Select
        label="Theme"
        data={[{ value: 'dark', label: 'Dark' }, { value: 'light', label: 'Light' }]}
        value={settings.theme}
        onChange={(v) => updateSettings({ ...settings, theme: v as 'light' | 'dark' })}
      />
      <NumberInput
        label="Terminal Font Size"
        value={settings.font_size}
        onChange={(v) => updateSettings({ ...settings, font_size: Number(v) })}
        min={8}
        max={32}
      />

      <Divider label="Defaults" labelPosition="center" />
      <NumberInput
        label="Default SSH Port"
        value={settings.ssh_port}
        onChange={(v) => updateSettings({ ...settings, ssh_port: Number(v) })}
        min={1}
        max={65535}
      />
      <Switch
        label="RDP Fullscreen"
        checked={settings.rdp_fullscreen}
        onChange={(e) => updateSettings({ ...settings, rdp_fullscreen: e.currentTarget.checked })}
      />
      <Switch
        label="RDP Admin Mode"
        checked={settings.rdp_admin_mode}
        onChange={(e) => updateSettings({ ...settings, rdp_admin_mode: e.currentTarget.checked })}
      />

      <Divider label="Data" labelPosition="center" />
      <Group>
        <Button onClick={handleBackup}>Backup Data</Button>
        <Button color="red" variant="light" onClick={handleRestore}>Restore from Backup</Button>
      </Group>

      <Divider label="Software Updates" labelPosition="center" />
      <UpdaterPanel />
    </Stack>
  );
}
