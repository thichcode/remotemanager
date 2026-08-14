import { useEffect, useRef } from 'react';
import { useCallback } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { listen } from '@tauri-apps/api/event';
import { sshWrite, sshResize } from '../services/tauri';
import { LogHighlighter } from '../lib/logHighlight';
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
  const lastSizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const fitRafRef = useRef<number | null>(null);
  const fontSize = useStore((s) => s.settings?.font_size ?? 13);
  const highlightRef = useRef<LogHighlighter | null>(null);

  const sendResize = useCallback((t: XTerm) => {
    const sid = sessionRef.current;
    if (!sid) return;
    const prev = lastSizeRef.current;
    if (prev && prev.cols === t.cols && prev.rows === t.rows) return;
    lastSizeRef.current = { cols: t.cols, rows: t.rows };
    sshResize(sid, t.cols, t.rows).catch(() => {});
  }, []);

  const fit = useCallback(() => {
    const t = termRef.current;
    const f = fitRef.current;
    const el = containerRef.current;
    if (!t || !f || !el || el.offsetParent === null) return;
    f.fit();
    sendResize(t);
  }, [sendResize]);

  // debounce fit through a single rAF per frame so resize storms don't
  // resize the pty repeatedly (prevents TUI apps like htop from jumping)
  const scheduleFit = useCallback(() => {
    if (fitRafRef.current !== null) return;
    fitRafRef.current = requestAnimationFrame(() => {
      fitRafRef.current = null;
      fit();
    });
  }, [fit]);

  useEffect(() => {
    return () => {
      if (fitRafRef.current !== null) {
        cancelAnimationFrame(fitRafRef.current);
        fitRafRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    sessionRef.current = tab.sessionId;
    highlightRef.current?.reset();
    if (tab.sessionId) {
      // The pty is spawned with a default 80x24 size before the session id
      // exists. If the first fit() ran while sessionId was still null, the
      // resize was dropped and the pty keeps the wrong size, garbling all
      // multi-line output. Re-fit now that the session is connected.
      scheduleFit();
      if (pendingRef.current.length) {
        const pending = pendingRef.current;
        pendingRef.current = [];
        for (const bytes of pending) {
          sshWrite(tab.sessionId, bytes).catch(() => {});
        }
      }
    }
  }, [tab.sessionId, scheduleFit]);

  // init xterm once
  useEffect(() => {
    if (termRef.current || !containerRef.current) return;
    const term = new XTerm({
      fontFamily: 'Consolas, monospace',
      fontSize,
      cursorBlink: true,
      theme: { background: '#0d1117', foreground: '#e6edf3' },
    });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(containerRef.current);
    termRef.current = term;
    fitRef.current = fitAddon;
    highlightRef.current = new LogHighlighter();

    // FitAddon measures the cell size to compute cols/rows. If the font is
    // not loaded yet, xterm falls back to a generic metric, cols are
    // over-estimated and the pty is resized to a wrong width, which garbles
    // all multi-line output. Re-fit once the fonts are ready.
    if (typeof document !== 'undefined' && 'fonts' in document) {
      document.fonts.ready.then(() => {
        if (termRef.current === term) scheduleFit();
      }).catch(() => {});
    }
    scheduleFit();

    term.onData((data) => {
      const bytes = Array.from(new TextEncoder().encode(data));
      const sid = sessionRef.current;
      if (sid) {
        sshWrite(sid, bytes).catch(() => {});
      } else {
        pendingRef.current.push(bytes);
      }
    });

    term.attachCustomKeyEventHandler((e) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'R') {
        term.reset();
        highlightRef.current?.reset();
        scheduleFit();
        return false;
      }
      return true;
    });

    const unlistenOutput = listen<{ sessionId: string; data: number[] }>('ssh://output', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      const hl = highlightRef.current;
      const data = new Uint8Array(event.payload.data);
      term.write(hl ? hl.feed(data) : data);
    });
    const unlistenExit = listen<{ sessionId: string; code: number }>('ssh://exit', (event) => {
      if (event.payload.sessionId !== sessionRef.current) return;
      const hl = highlightRef.current;
      if (hl) term.write(hl.flush());
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
      highlightRef.current = null;
    };
  }, []);

  // fit + resize when becoming active
  useEffect(() => {
    if (active) {
      scheduleFit();
      termRef.current?.focus();
    }
  }, [active, scheduleFit]);

  // fit when the container actually has size (window resizes and when the
  // servers view becomes visible again after a view switch)
  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver(() => {
      if (active) scheduleFit();
    });
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [active, scheduleFit]);

  // update font size when settings change
  useEffect(() => {
    if (termRef.current) {
      termRef.current.options.fontSize = fontSize;
      scheduleFit();
    }
  }, [fontSize, scheduleFit]);

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
