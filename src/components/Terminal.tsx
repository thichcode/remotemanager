import { useEffect, useRef } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { listen } from '@tauri-apps/api/event';
import { sshWrite, sshResize, sshClose } from '../services/tauri';
import { Stack, Text } from '@mantine/core';
import type { TerminalTab } from '../types';

interface Props {
  tab: TerminalTab;
  active: boolean;
}

export function Terminal({ tab, active }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const sessionRef = useRef<string | null>(tab.sessionId);

  useEffect(() => {
    sessionRef.current = tab.sessionId;
  }, [tab.sessionId]);

  // init xterm once
  useEffect(() => {
    if (termRef.current || !containerRef.current) return;
    const term = new XTerm({
      convertEol: true,
      fontFamily: 'Consolas, monospace',
      fontSize: 13,
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
      const sid = sessionRef.current;
      if (!sid) return;
      const bytes = Array.from(new TextEncoder().encode(data));
      sshWrite(sid, bytes).catch(() => {});
    });

    const unlistenOutput = listen<{ sessionId: string; data: number[] }>('ssh://output', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      term.write(new Uint8Array(event.payload.data));
    });
    const unlistenExit = listen<{ sessionId: string; code: number }>('ssh://exit', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      term.write(`\r\n\x1b[31mConnection closed (code ${event.payload.code})\x1b[0m\r\n`);
    });
    let cancelled = false;
    Promise.all([unlistenOutput, unlistenExit]).then(([a, b]) => {
      if (cancelled) { a(); b(); return; }
    });
    return () => {
      cancelled = true;
      const sid = sessionRef.current;
      if (sid) sshClose(sid).catch(() => {});
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
      const sid = sessionRef.current;
      if (sid) sshResize(sid, t.cols, t.rows).catch(() => {});
    }
  }, [active]);

  // fit on window resize
  useEffect(() => {
    const onResize = () => {
      if (active && fitRef.current && termRef.current) {
        fitRef.current.fit();
      }
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [active]);

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
