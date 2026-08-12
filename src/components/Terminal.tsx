import { useEffect, useRef } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { listen } from '@tauri-apps/api/event';
import { sshWrite, sshResize } from '../services/tauri';
import { Stack, Text } from '@mantine/core';
import { useStore } from '../store/useStore';
import type { SessionTab } from '../types';

function exitCodeMessage(code: number): string {
  const messages: Record<number, string> = {
    1: 'General error',
    2: 'Misuse of shell builtins',
    126: 'Command not executable',
    127: 'Command not found',
    128: 'Invalid exit argument',
    129: 'SIGHUP (terminal closed)',
    130: 'SIGINT (Ctrl+C)',
    137: 'SIGKILL',
    143: 'SIGTERM',
    255: 'SSH error / connection failed',
  };
  return messages[code] ?? `Exit code ${code}`;
}

interface Props {
  tab: SessionTab;
  active: boolean;
}

export function Terminal({ tab, active }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<string | null>(tab.sessionId);
  const pendingRef = useRef<number[][]>([]);
  const fontSize = useStore((s) => s.settings?.font_size ?? 13);

  useEffect(() => {
    sessionRef.current = tab.sessionId;
    if (tab.sessionId && pendingRef.current.length) {
      const pending = pendingRef.current;
      pendingRef.current = [];
      for (const bytes of pending) {
        sshWrite(tab.sessionId, bytes).catch(() => {});
      }
    }
  }, [tab.sessionId]);

  // init xterm once
  useEffect(() => {
    if (termRef.current || !containerRef.current) return;
    const term = new XTerm({
      convertEol: true,
      fontFamily: 'Consolas, monospace',
      fontSize,
      cursorBlink: true,
      theme: { background: '#0d1117', foreground: '#e6edf3' },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;

    term.onData((data) => {
      const bytes = Array.from(new TextEncoder().encode(data));
      const sid = sessionRef.current;
      if (sid) {
        sshWrite(sid, bytes).catch(() => {});
      } else {
        pendingRef.current.push(bytes);
      }
    });

    const unlistenOutput = listen<{ sessionId: string; data: number[] }>('ssh://output', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      term.write(new Uint8Array(event.payload.data));
    });
    const unlistenExit = listen<{ sessionId: string; code: number }>('ssh://exit', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      term.write(`\r\n\x1b[31mConnection closed (${exitCodeMessage(event.payload.code)})\x1b[0m\r\n`);
      (term.options as { readonly?: boolean }).readonly = true;
    });
    let cancelled = false;
    const unlisteners: (() => void)[] = [];
    Promise.all([unlistenOutput, unlistenExit])
      .then(([a, b]) => {
        if (cancelled) { a(); b(); return; }
        unlisteners.push(a, b);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisteners.forEach((u) => u());
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, []);

  // fit + resize when becoming active
  useEffect(() => {
    if (active && fitRef.current && termRef.current) {
      const t = termRef.current;
      fitRef.current.fit();
      t.focus();
      const sid = sessionRef.current;
      if (sid) sshResize(sid, t.cols, t.rows).catch(() => {});
    }
  }, [active]);

  // fit when the container actually has size (window resizes and when the
  // servers view becomes visible again after a view switch)
  useEffect(() => {
    if (!containerRef.current) return;
    const fit = () => {
      const el = containerRef.current;
      if (!active || !fitRef.current || !termRef.current || !el || el.offsetParent === null) return;
      fitRef.current.fit();
      const t = termRef.current;
      const sid = sessionRef.current;
      if (sid) sshResize(sid, t.cols, t.rows).catch(() => {});
    };
    const observer = new ResizeObserver(fit);
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [active]);

  // update font size when settings change
  useEffect(() => {
    if (termRef.current) {
      termRef.current.options.fontSize = fontSize;
      fitRef.current?.fit();
    }
  }, [fontSize]);

  if (tab.status === 'closed' && !tab.sessionId) {
    return (
      <Stack align="center" justify="center" h="100%">
        <Text c="dimmed">Session failed to start.</Text>
      </Stack>
    );
  }

  return (
    <div
      ref={containerRef}
      style={{ height: '100%', padding: 8, background: '#0d1117' }}
    />
  );
}
