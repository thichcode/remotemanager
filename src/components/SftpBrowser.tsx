import { Box, Text, Progress, Button, Group, ActionIcon, Tooltip, Stack, ScrollArea, Checkbox, Divider, Loader } from '@mantine/core';
import { IconFolder, IconFile, IconRefresh, IconDownload, IconChevronRight, IconTrash } from '@tabler/icons-react';
import { useEffect, useRef, useState, useCallback } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { sftpOpen, sftpList, sftpGetHome, sftpUpload, sftpDownload, getUploadProgress, cancelUpload } from '../services/tauri';
import { notifications } from '@mantine/notifications';
import type { RemoteEntry, UploadProgress } from '../types';

interface TreeNode {
  path: string;
  name: string;
  is_dir: boolean;
  children: TreeNode[] | null | undefined;
  loaded: boolean;
  expanded: boolean;
  hint: string;
}

function joinRemote(base: string, name: string): string {
  const b = base.endsWith('/') ? base.slice(0, -1) : base;
  return name.startsWith('/') ? `${b}/${name.slice(1)}` : `${b}/${name}`;
}

function toNodes(entries: RemoteEntry[], showHidden: boolean, parentPath: string): TreeNode[] {
  return entries
    .filter(e => showHidden || !e.is_hidden)
    .sort((a, b) => (a.is_dir === b.is_dir ? a.name.localeCompare(b.name) : a.is_dir ? -1 : 1))
    .map(e => ({
      path: joinRemote(parentPath, e.name),
      name: e.name,
      is_dir: e.is_dir,
      children: e.is_dir ? [] : undefined,
      loaded: false,
      expanded: false,
      hint: e.is_dir ? '' : `${formatSize(e.size)}`,
    }));
}

function formatSize(b: number): string {
  if (b >= 1024 * 1024 * 1024) return `${(b / (1024 ** 3)).toFixed(1)} GB`;
  if (b >= 1024 * 1024) return `${(b / (1024 ** 2)).toFixed(1)} MB`;
  if (b >= 1024) return `${(b / 1024).toFixed(1)} KB`;
  return `${b} B`;
}

// Remap a node: node.path is full remote path. children rendered recursively.
function renderNodes(
  nodes: TreeNode[],
  depth: number,
  handlers: {
    toggle: (n: TreeNode) => void;
    refresh: (n: TreeNode) => void;
    download: (n: TreeNode) => void;
    onDragOver: (e: React.DragEvent, n: TreeNode) => void;
    onDragLeave: () => void;
    onDrop: (e: React.DragEvent, n: TreeNode) => void;
    selected: Set<string>;
    toggleSelect: (n: TreeNode, ctrl: boolean) => void;
  },
): React.ReactNode[] {
  const out: React.ReactNode[] = [];
  for (const n of nodes) {
    const isSel = handlers.selected.has(n.path);
    out.push(
      <div key={n.path}>
        <Group
          gap={4}
          pl={depth * 14}
          px={6}
          py={2}
          style={{
            cursor: 'pointer',
            borderRadius: 4,
            background: isSel ? 'var(--mantine-color-blue-9)' : undefined,
            outline: undefined,
          }}
          onClick={(e) => handlers.toggleSelect(n, e.ctrlKey || e.metaKey)}
          draggable={false}
          onDragOver={(e) => { e.preventDefault(); e.stopPropagation(); handlers.onDragOver(e, n); }}
          onDragLeave={() => { handlers.onDragLeave(); }}
          onDrop={(e) => { e.preventDefault(); e.stopPropagation(); handlers.onDrop(e, n); }}
        >
          {n.is_dir ? (
            <>
              <IconChevronRight size={11} style={{ transform: n.expanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.12s' }} />
              <IconFolder size={14} />
              {!n.loaded && n.is_dir && <IconRefresh size={10} />}
            </>
          ) : (
            <>
              <Box style={{ width: 15 }} />
              <IconFile size={14} />
            </>
          )}
          <Text size="xs" style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={n.hint}>
            {n.name}
          </Text>
          {!n.is_dir && (
            <ActionIcon size={16} variant="subtle" title="Download" onClick={(e) => { e.stopPropagation(); handlers.download(n); }}>
              <IconDownload size={12} />
            </ActionIcon>
          )}
        </Group>
        {n.is_dir && n.expanded && n.children
          ? renderNodes(n.children, depth + 1, handlers)
          : null}
      </div>,
    );
  }
  return out;
}

interface Props {
  serverId: string | null;
  serverHost: string | null;
  onClearHistory: () => void;
}

export function SftpBrowser({ serverId, serverHost, onClearHistory }: Props) {
  const [home, setHome] = useState('');
  const [root, setRoot] = useState<TreeNode[]>([]);
  const [rootLoaded, setRootLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [showHidden, setShowHidden] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [jobId, setJobId] = useState<string | null>(null);
  const [progress, setProgress] = useState<UploadProgress | null>(null);
  const [dragging, setDragging] = useState(false);
  const timerRef = useRef<number | null>(null);
  const rootRef = useRef<TreeNode[]>([]);
  const enabled = serverId !== null;

  const stopPolling = () => {
    if (timerRef.current !== null) { window.clearInterval(timerRef.current); timerRef.current = null; }
  };
  useEffect(() => () => stopPolling(), []);

  // Imperative tree storage so drop targets can mutate the same nodes.
  useEffect(() => { rootRef.current = root; }, [root]);

  const load = useCallback(async (serverId: string) => {
    setLoading(true);
    setRootLoaded(false);
    setSelected(new Set());
    try {
      let h = await sftpGetHome(serverId);
      if (!h) h = await sftpOpen(serverId);
      setHome(h);
      const entries = await sftpList(serverId, h);
      setRoot(toNodes(entries, showHidden, h));
      setRootLoaded(true);
    } catch (e) {
      notifications.show({ title: 'SFTP browse failed', message: String(e), color: 'red' });
      setRootLoaded(true);
    } finally {
      setLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showHidden]);

  useEffect(() => { if (serverId) void load(serverId); else { setRoot([]); setRootLoaded(false); } }, [serverId, load]);

  const mutate = (fn: (nodes: TreeNode[]) => void) => {
    const nodes = rootRef.current;
    fn(nodes);
    setRoot([...nodes]);
  };

  const findNode = (nodes: TreeNode[], path: string): TreeNode | null => {
    for (const n of nodes) {
      if (n.path === path) return n;
      if (n.is_dir && n.children) {
        const f = findNode(n.children, path);
        if (f) return f;
      }
    }
    return null;
  };

  const toggleNode = async (n: TreeNode) => {
    if (!n.is_dir) return;
    mutate((nodes) => {
      const found = findNode(nodes, n.path);
      if (!found) return;
      if (!found.loaded) {
        found.loaded = true;
        found.expanded = true;
        void sftpList(serverId!, found.path).then((entries) => {
          mutate2(entries, found.path);
        }).catch((e) => notifications.show({ title: 'List failed', message: String(e), color: 'red' }));
      } else {
        found.expanded = !found.expanded;
      }
    });
  };
  const mutate2 = (entries: RemoteEntry[], path: string) => {
    const nodes = rootRef.current;
    const f = findNode(nodes, path);
    if (f) {
      f.children = toNodes(entries, showHidden, path);
      f.loaded = true;
      f.expanded = true;
    }
    setRoot([...nodes]);
  };

  const refreshNode = async (n: TreeNode) => {
    try {
      const entries = await sftpList(serverId!, n.path);
      mutate2(entries, n.path);
    } catch (e) { notifications.show({ title: 'Refresh failed', message: String(e), color: 'red' }); }
  };

  const refreshRoot = async () => {
    if (serverId) { await refreshNodeRef(home); }
  };

  const refreshNodeRef = async (path: string) => {
    try {
      const entries = await sftpList(serverId!, path === home ? home : path);
      mutate2(entries, path);
    } catch (e) { notifications.show({ title: 'Refresh failed', message: String(e), color: 'red' }); }
  };

  const toggleSelect = (n: TreeNode, ctrl: boolean) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (ctrl) {
        if (next.has(n.path)) next.delete(n.path); else next.add(n.path);
      } else {
        next.clear();
        next.add(n.path);
      }
      return next;
    });
  };

  const downloadNodes = async (paths: string[]) => {
    if (!serverId || paths.length === 0) return;
    const dir = await open({ directory: true, title: 'Choose download folder' });
    if (!dir) return;
    try {
      const id = await sftpDownload(serverId, String(dir), paths);
      startJobPoll(id, 'Download');
    } catch (e) { notifications.show({ title: 'Download failed', message: String(e), color: 'red' }); }
  };

  const downloadSelected = () => downloadNodes([...selected]);

  const onDownloadNode = (n: TreeNode) => downloadNodes([n.path]);

  const dropUpload = async (targetPath: string, localPaths: string[]) => {
    if (!serverId) return;
    try {
      const id = await sftpUpload(serverId, targetPath === home ? home : targetPath, localPaths);
      startJobPoll(id, 'Upload');
    } catch (e) { notifications.show({ title: 'Upload failed', message: String(e), color: 'red' }); }
  };

  const startJobPoll = (id: string, kind: 'Upload' | 'Download') => {
    stopPolling();
    setJobId(id);
    setProgress(null);
    timerRef.current = window.setInterval(async () => {
      try {
        const p = await getUploadProgress(id);
        if (!p) { stopPolling(); setJobId(null); return; }
        setProgress(p);
        if (p.state === 'done' || p.state === 'error' || p.state === 'cancelled') {
          stopPolling();
          if (p.state === 'done') {
            notifications.show({ title: `${kind} complete`, message: `${p.total_files} file(s)`, color: 'green' });
            void refreshNodeRef(home);
          } else if (p.state === 'error') {
            notifications.show({ title: `${kind} failed`, message: p.error ?? 'Unknown error', color: 'red' });
          }
          setTimeout(() => { setJobId(null); setProgress(null); }, 2500);
        }
      } catch { stopPolling(); }
    }, 250);
  };

  const cancelJob = async () => { if (jobId) { await cancelUpload(jobId); } };

  const onGlobalDragOver = (e: React.DragEvent) => { e.preventDefault(); setDragging(true); };
  const onGlobalDragLeave = () => { setDragging(false); };
  const onGlobalDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
    if (!enabled || !serverId) return;
    const files = e.dataTransfer?.files;
    const paths = files ? Array.from(files).map(f => (f as unknown as { path: string }).path).filter(Boolean) : [];
    if (paths.length === 0) return;
    await dropUpload(home, paths);
  };
  const onNodeDragOver = () => { setDragging(true); };
  const onNodeDrop = (e: React.DragEvent, n: TreeNode) => {
    e.preventDefault();
    e.stopPropagation();
    setDragging(false);
    if (!enabled || !serverId || !n.is_dir) return;
    const files = e.dataTransfer?.files;
    const paths = files ? Array.from(files).map(f => (f as unknown as { path: string }).path).filter(Boolean) : [];
    if (paths.length === 0) return;
    void dropUpload(n.path, paths);
  };

  // Tauri v2 drag-drop event (for OS-level file drop anywhere on webview)
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === 'over') setDragging(true);
      else if (payload.type === 'leave') setDragging(false);
      else if (payload.type === 'drop') {
        setDragging(false);
        if (enabled && serverId && payload.paths.length > 0) {
          void dropUpload(home, payload.paths);
        }
      }
    }).then((fn) => { unlisten = fn; });
    return () => { if (unlisten) unlisten(); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, serverId, home]);

  const pct = progress && progress.total_bytes > 0 ? Math.round(progress.bytes_sent * 100 / progress.total_bytes) : 0;

  return (
    <Stack gap={4}>
      <Group justify="space-between" align="center">
        <Text size="xs" fw={600} c="dimmed" tt="uppercase">SFTP Files</Text>
        <Group gap={2}>
          <Tooltip label="Show hidden files">
            <Checkbox size="xs" checked={showHidden} onChange={(e) => setShowHidden(e.currentTarget.checked)} aria-label="Show hidden files" />
          </Tooltip>
          <Tooltip label="Refresh">
            <ActionIcon size="sm" variant="subtle" onClick={refreshRoot}><IconRefresh size={14} /></ActionIcon>
          </Tooltip>
          <Tooltip label="Clear history">
            <ActionIcon size="sm" variant="subtle" onClick={onClearHistory}><IconTrash size={14} /></ActionIcon>
          </Tooltip>
        </Group>
      </Group>

      {!enabled ? (
        <Text size="xs" c="dimmed" px={6}>Open an SSH terminal to browse files.</Text>
      ) : (
        <Box
          style={{
            border: dragging ? '2px dashed var(--mantine-color-blue-5)' : '2px dashed var(--mantine-color-dark-4)',
            borderRadius: 6,
            padding: 4,
            minHeight: 60,
          }}
          onDragOver={onGlobalDragOver}
          onDragLeave={onGlobalDragLeave}
          onDrop={onGlobalDrop}
        >
          {loading ? (
            <Group justify="center" py="md"><Loader size="xs" /></Group>
          ) : !rootLoaded ? (
            <Text size="xs" c="dimmed" px={6}>SFTP unavailable.</Text>
          ) : (
            <ScrollArea.Autosize mah={380} type="auto">
              <Stack gap={0}>
                <Group gap={4} px={6} py={2}>
                  <IconFolder size={14} />
                  <Text size="xs" style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} c="dimmed">{home || serverHost}</Text>
                  <Text size="xs" c="dimmed">{serverHost}</Text>
                </Group>
                <Divider />
                {root.length === 0 && <Text size="xs" c="dimmed" px={6} py={4}>Empty directory</Text>}
                {root.map((n) => (
                  <Box key={n.path}>
                  {renderNodes([n], 1, {
                    toggle: toggleNode,
                    refresh: refreshNode,
                    download: onDownloadNode,
                    onDragOver: onNodeDragOver,
                    onDragLeave: () => setDragging(false),
                    onDrop: onNodeDrop,
                    selected,
                    toggleSelect,
                  })}
                  </Box>
                ))}
              </Stack>
            </ScrollArea.Autosize>
          )}

          {selected.size > 0 && (
            <Button size="xs" variant="light" fullWidth mt={4} leftSection={<IconDownload size={12} />} onClick={downloadSelected}>
              Download ({selected.size})
            </Button>
          )}

          {jobId && progress ? (
            <>
              <Text size="xs" mt={4}>{(progress.file_index + 1)}/{progress.total_files} {progress.current_file}</Text>
              <Progress value={pct} size="sm" />
              <Group justify="center" mt={4}>
                <Button size="xs" variant="light" color="red" onClick={cancelJob}>Cancel</Button>
              </Group>
            </>
          ) : (
            <Text size="xs" c="dimmed" px={6} mt={4}>Drop files to upload to {home}</Text>
          )}
        </Box>
      )}
    </Stack>
  );
}
