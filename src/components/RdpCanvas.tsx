import { useEffect, useRef, useCallback } from 'react';
import { Text, Stack, Loader, Badge, Paper, Group } from '@mantine/core';
import { IconDeviceDesktop } from '@tabler/icons-react';
import type { SessionTab } from '../types';

const MSG_FRAME = 0x01;
const MSG_CLOSED = 0x02;
const MSG_MOUSE = 0x10;
const MSG_KEYBOARD = 0x11;

interface RdpCanvasProps {
  tab: SessionTab;
}

export function RdpCanvas({ tab }: RdpCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const sendMouseEvent = useCallback((x: number, y: number, buttonMask: number, eventType: number) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return;
    const buf = new ArrayBuffer(7);
    const view = new DataView(buf);
    view.setUint8(0, MSG_MOUSE);
    view.setUint16(1, x, true);
    view.setUint16(3, y, true);
    view.setUint8(5, buttonMask);
    view.setUint8(6, eventType);
    wsRef.current.send(buf);
  }, []);

  const sendKeyboardEvent = useCallback((scanCode: number, down: boolean) => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return;
    const buf = new ArrayBuffer(4);
    const view = new DataView(buf);
    view.setUint8(0, MSG_KEYBOARD);
    view.setUint16(1, scanCode, true);
    view.setUint8(3, down ? 1 : 0);
    wsRef.current.send(buf);
  }, []);

  useEffect(() => {
    if (tab.protocol !== 'rdp' || !tab.wsPort || tab.status !== 'connected') return;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const wsUrl = `ws://127.0.0.1:${tab.wsPort}`;
    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      // Connection established; initial size will be set on first frame
    };

    ws.onmessage = (event) => {
      if (!(event.data instanceof ArrayBuffer)) return;
      const data = new Uint8Array(event.data);
      if (data.length < 1) return;

      const msgType = data[0];

      if (msgType === MSG_FRAME && data.length >= 13) {
        const width = new DataView(data.buffer, data.byteOffset).getUint16(1, true);
        const height = new DataView(data.buffer, data.byteOffset).getUint16(3, true);
        const x = new DataView(data.buffer, data.byteOffset).getUint32(5, true);
        const y = new DataView(data.buffer, data.byteOffset).getUint32(9, true);
        const bgra = data.slice(13);

        if (canvas.width !== width || canvas.height !== height) {
          canvas.width = width;
          canvas.height = height;
        }

        // Convert BGRA to RGBA (force opaque alpha)
        const rgba = new Uint8ClampedArray(bgra.length);
        for (let i = 0; i < bgra.length; i += 4) {
          rgba[i] = bgra[i + 2];     // R
          rgba[i + 1] = bgra[i + 1]; // G
          rgba[i + 2] = bgra[i];     // B
          rgba[i + 3] = 0xFF;        // A (opaque)
        }

        const imageData = new ImageData(rgba, width, height);
        ctx.putImageData(imageData, x, y);
      } else if (msgType === MSG_CLOSED) {
        ws.close();
      }
    };

    ws.onerror = () => {
      console.error('RDP WebSocket error');
    };

    ws.onclose = () => {
      wsRef.current = null;
    };

    return () => {
      ws.close();
      wsRef.current = null;
    };
  }, [tab.protocol, tab.wsPort, tab.status]);

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const buttonMask = e.button === 0 ? 0x01 : e.button === 2 ? 0x02 : 0x04;
    sendMouseEvent(x, y, buttonMask, 1);
  }, [sendMouseEvent]);

  const handleMouseUp = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const buttonMask = e.button === 0 ? 0x01 : e.button === 2 ? 0x02 : 0x04;
    sendMouseEvent(x, y, buttonMask, 2);
  }, [sendMouseEvent]);

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    sendMouseEvent(x, y, 0, 0);
  }, [sendMouseEvent]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    sendKeyboardEvent(e.keyCode, true);
  }, [sendKeyboardEvent]);

  const handleKeyUp = useCallback((e: React.KeyboardEvent) => {
    e.preventDefault();
    sendKeyboardEvent(e.keyCode, false);
  }, [sendKeyboardEvent]);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
  }, []);

  if (tab.status === 'connecting') {
    return (
      <Paper p="xl" h="100%" style={{ display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <Stack align="center" gap="md">
          <IconDeviceDesktop size={48} style={{ opacity: 0.3 }} />
          <Text size="lg" fw={600}>{tab.title}</Text>
          <Group gap="sm">
            <Badge color="blue" size="lg">Connecting...</Badge>
            <Loader size="sm" />
          </Group>
        </Stack>
      </Paper>
    );
  }

  return (
    <div
      ref={containerRef}
      style={{ width: '100%', height: '100%', overflow: 'hidden', background: '#000' }}
    >
      <canvas
        ref={canvasRef}
        style={{ display: 'block', cursor: 'default' }}
        tabIndex={0}
        onMouseDown={handleMouseDown}
        onMouseUp={handleMouseUp}
        onMouseMove={handleMouseMove}
        onKeyDown={handleKeyDown}
        onKeyUp={handleKeyUp}
        onContextMenu={handleContextMenu}
      />
    </div>
  );
}
