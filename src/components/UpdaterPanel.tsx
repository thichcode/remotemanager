import { useState } from 'react';
import { Stack, Group, Text, Button, Progress, Badge } from '@mantine/core';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { notifications } from '@mantine/notifications';

export function UpdaterPanel() {
  const [checking, setChecking] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [available, setAvailable] = useState(false);

  const handleCheck = async () => {
    setChecking(true);
    try {
      const update = await check();
      setChecking(false);
      if (update) {
        setAvailable(true);
        notifications.show({ title: 'Update Available', message: `Version ${update.version}`, color: 'blue' });
        let downloaded = 0;
        let total = 0;
        await update.downloadAndInstall((event) => {
          switch (event.event) {
            case 'Started':
              total = event.data.contentLength ?? 0;
              break;
            case 'Progress':
              downloaded += event.data.chunkLength;
              setProgress(total ? Math.round((downloaded / total) * 100) : null);
              break;
            case 'Finished':
              setProgress(100);
              break;
          }
        });
        await relaunch();
      } else {
        notifications.show({ title: 'Up to Date', message: 'You have the latest version.', color: 'green' });
      }
    } catch (e: unknown) {
      setChecking(false);
      const msg = e instanceof Error ? e.message : String(e);
      notifications.show({ title: 'Update Check Failed', message: msg, color: 'red' });
    }
  };

  return (
    <Stack>
      <Group justify="space-between">
        <Text fw={500}>Updates</Text>
        <Badge color={available ? 'blue' : 'gray'}>{available ? 'Update Available' : 'Up to Date'}</Badge>
      </Group>
      {progress !== null && <Progress value={progress} />}
      <Button onClick={handleCheck} loading={checking}>Check for Updates</Button>
    </Stack>
  );
}
