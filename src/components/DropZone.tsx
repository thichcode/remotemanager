import { Box, Text, Progress, Button, Group, ActionIcon, Tooltip, Stack } from '@mantine/core';
import { IconUpload, IconTrash } from '@tabler/icons-react';
import { useEffect, useRef, useState } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { uploadFiles, getUploadProgress, cancelUpload } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import type { UploadProgress } from '../types';

interface Props {
  activeServerId: string | null;
  activeServerHost: string | null;
  onClearHistory: () => void;
}

export function DropZone({ activeServerId, activeServerHost, onClearHistory }: Props) {
  const [dragging, setDragging] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<UploadProgress | null>(null);
  const timerRef = useRef<number | null>(null);

  const enabled = activeServerId !== null;

  const stopPolling = () => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  useEffect(() => () => { stopPolling(); }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebview().onDragDropEvent((event) => {
      if (disposed) return;
      const payload = event.payload;
      if (payload.type === 'over') {
        setDragging(true);
      } else if (payload.type === 'leave') {
        setDragging(false);
      } else if (payload.type === 'drop') {
        setDragging(false);
        if (!enabled || !activeServerId) return;
        const paths = payload.paths;
        if (paths.length === 0) return;
        startUpload(activeServerId, paths);
      }
    }).then((fn) => { unlisten = fn; });
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, activeServerId]);

  const startUpload = async (serverId: string, paths: string[]) => {
    try {
      const id = await uploadFiles(serverId, paths);
      setJobId(id);
      timerRef.current = window.setInterval(async () => {
        try {
          const p = await getUploadProgress(id);
          if (!p) {
            stopPolling();
            setJobId(null);
            return;
          }
          setProgress(p);
          if (p.state === 'done' || p.state === 'error' || p.state === 'cancelled') {
            stopPolling();
            if (p.state === 'done') {
              notifications.show({ title: 'Upload complete', message: `${p.current_file} uploaded to ~`, color: 'green' });
            } else if (p.state === 'error') {
              notifications.show({ title: 'Upload failed', message: p.error ?? 'Unknown error', color: 'red' });
            }
            setTimeout(() => {
              setJobId(null);
              setProgress(null);
            }, 2500);
          }
        } catch {
          stopPolling();
        }
      }, 250);
    } catch (err) {
      notifications.show({ title: 'Upload failed', message: String(err), color: 'red' });
    }
  };

  const pct = progress && progress.total_bytes > 0
    ? Math.round(progress.bytes_sent * 100 / progress.total_bytes)
    : 0;

  return (
    <Stack gap={4} mb="md">
      <Group justify="space-between" align="center">
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">Upload</Text>
        <Tooltip label="Clear history">
          <ActionIcon size="sm" variant="subtle" onClick={onClearHistory}>
            <IconTrash size={14} />
          </ActionIcon>
        </Tooltip>
      </Group>
      <Box
        p="xs"
        style={{
          border: dragging ? '2px dashed var(--mantine-color-blue-5)' : '2px dashed var(--mantine-color-dark-4)',
          borderRadius: 6,
          textAlign: 'center',
          opacity: enabled ? 1 : 0.4,
          cursor: enabled ? 'copy' : 'not-allowed',
          transition: 'border-color 0.15s',
        }}
      >
        {jobId && progress ? (
          <>
            <Text size="xs" mb={4}>{(progress.file_index + 1)}/{progress.total_files} {progress.current_file}</Text>
            <Progress value={pct} size="sm" />
            <Group justify="center" mt={4}>
              <Button size="xs" variant="light" color="red" onClick={async () => { if (jobId) { await cancelUpload(jobId); } }}>
                Cancel
              </Button>
            </Group>
          </>
        ) : (
          <>
            <IconUpload size={18} style={{ marginBottom: 4 }} />
            <Text size="xs">{enabled ? `Drop files to upload to ${activeServerHost}` : 'Open an SSH terminal to upload'}</Text>
          </>
        )}
      </Box>
    </Stack>
  );
}