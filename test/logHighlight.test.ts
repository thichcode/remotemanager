import { describe, it, expect } from 'vitest';
import { LogHighlighter } from '../src/lib/logHighlight';

const enc = (s: string) => new TextEncoder().encode(s);
const dec = (b: Uint8Array) => new TextDecoder().decode(b);

describe('LogHighlighter', () => {
  it('wraps ERROR in red and timestamp in cyan', () => {
    const hl = new LogHighlighter();
    const out = dec(hl.feed(enc('2024-01-01 10:00:00 [ERROR] boom\n')));
    expect(out).toContain('\x1b[31mERROR\x1b[0m');
    expect(out).toContain('\x1b[36m2024-01-01 10:00:00\x1b[0m');
  });

  it('colors each keyword level', () => {
    const hl = new LogHighlighter();
    const out = dec(hl.feed(enc('WARN INFO DEBUG TRACE\n')));
    expect(out).toContain('\x1b[33mWARN\x1b[0m');
    expect(out).toContain('\x1b[32mINFO\x1b[0m');
    expect(out).toContain('\x1b[90mDEBUG\x1b[0m');
    expect(out).toContain('\x1b[35mTRACE\x1b[0m');
  });

  it('matches WARNING as well as WARN', () => {
    const hl = new LogHighlighter();
    const out = dec(hl.feed(enc('WARNING\n')));
    expect(out).toContain('\x1b[33mWARNING\x1b[0m');
  });

  it('does not emit partial lines until newline arrives', () => {
    const hl = new LogHighlighter();
    const a = hl.feed(enc('line with ER'));
    expect(dec(a)).toBe('');
    const b = hl.feed(enc('ROR here\n'));
    expect(dec(b)).toContain('\x1b[31mERROR\x1b[0m');
  });

  it('passes through TUI alternate screen output unchanged', () => {
    const hl = new LogHighlighter();
    hl.feed(enc('\x1b[?1049h'));
    const out = dec(hl.feed(enc('ERROR in htop\n')));
    expect(out).toContain('ERROR');
    expect(out).not.toContain('\x1b[31mERROR');
  });

  it('resumes highlighting after leaving alternate screen', () => {
    const hl = new LogHighlighter();
    hl.feed(enc('\x1b[?1049h'));
    hl.feed(enc('\x1b[?1049l'));
    const out = dec(hl.feed(enc('ERROR\n')));
    expect(out).toContain('\x1b[31mERROR\x1b[0m');
  });

  it('detects alternate screen even when the escape spans chunks', () => {
    const hl = new LogHighlighter();
    hl.feed(enc('\x1b[?1049'));
    hl.feed(enc('h'));
    const out = dec(hl.feed(enc('ERROR in htop\n')));
    expect(out).not.toContain('\x1b[31mERROR');
  });

  it('does not split multibyte UTF-8 across chunks', () => {
    const hl = new LogHighlighter();
    const a = hl.feed(enc('ERROR '));
    const b = hl.feed(new Uint8Array([0xc6])); // first byte of "ơ" (U+01A1)
    const c = hl.feed(new Uint8Array([0xa1, 0x0a])); // second byte + newline
    const out = dec(new Uint8Array([...a, ...b, ...c]));
    expect(out).toContain('\x1b[31mERROR\x1b[0m');
    expect(out).toContain('ơ');
  });

  it('preserves pre-existing ANSI escape sequences', () => {
    const hl = new LogHighlighter();
    const out = dec(hl.feed(enc('\x1b[1;32mgreen ERROR\x1b[0m\n')));
    expect(out).toContain('\x1b[1;32m');
    expect(out).toContain('\x1b[0m');
    expect(out).toContain('\x1b[31mERROR\x1b[0m');
  });

  it('flush emits a final line without trailing newline', () => {
    const hl = new LogHighlighter();
    hl.feed(enc('ERROR'));
    expect(dec(hl.flush())).toContain('\x1b[31mERROR\x1b[0m');
  });

  it('reset clears pending state', () => {
    const hl = new LogHighlighter();
    hl.feed(enc('ERROR'));
    hl.reset();
    expect(dec(hl.flush())).toBe('');
  });
});
