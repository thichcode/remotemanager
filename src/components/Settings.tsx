import { useEffect } from 'react';
import { Stack, NumberInput, Select, Switch, Text, Divider } from '@mantine/core';
import { useStore } from '../store/useStore';

export function Settings() {
  const { settings, loadSettings, updateSettings } = useStore();

  useEffect(() => {
    loadSettings();
  }, []);

  if (!settings) return <Text>Loading...</Text>;

  return (
    <Stack gap="md" maw={500}>
      <Text size="lg" fw={600}>Settings</Text>

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
    </Stack>
  );
}
