# SSH Log Keyword Highlight Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatically color-code log keywords (ERROR, WARN, INFO, DEBUG, TRACE, timestamps) in the embedded SSH terminal output, while skipping full-screen TUIs (htop, vim).

**Architecture:** A pure-TS `LogHighlighter` class sits in the `ssh://output` data path: it buffers bytes by line, wraps matching keywords in ANSI SGR color codes, and returns transformed bytes for `term.write`. It tracks the alternate-screen-buffer state (`\x1b[?1049h/l`) to pass through TUI output untouched. Backend (Rust) is unchanged.

**Tech Stack:** TypeScript (strict), `@xterm/xterm` v5.5, vitest (new devDependency), Vite.

---

## File Structure

- Create: `src/lib/logHighlight.ts` — the pure `LogHighlighter` class (no React/xterm imports).
- Create: `test/logHighlight.test.ts` — vitest unit tests for the class.
- Create: `vitest.config.ts` — vitest config (node env, `test/**/*.test.ts`).
- Modify: `package.json` — add `test` script + `vitest` devDependency.
- Modify: `src/components/Terminal.tsx` — instantiate highlighter, feed output through it.
- Modify (version bump): `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` — 0.8.5 → 0.8.6.

---

### Task 1: Add vitest

**Files:**
- Modify: `package.json`
- Create: `vitest.config.ts`

- [ ] **Step 1: Install vitest as devDependency**

Run: `npm install -D vitest`
Expected: vitest added to `package.json` devDependencies.

- [ ] **Step 2: Add the `test` script to `package.json`**

Inside the `"scripts"` block (currently `"tauri:build": "tauri build"`), add:

```json
"test": "vitest run"
```

- [ ] **Step 3: Create `vitest.config.ts`**

```ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['test/**/*.test.ts'],
  },
});
```

- [ ] **Step 4: Verify vitest runs (empty suite passes)**

Run: `npm test`
Expected: exits 0 with a "no test files found" or "passed" message (no error).

- [ ] **Step 5: Commit**

```bash
git add package.json vitest.config.ts
git commit -m "test: add vitest for unit tests"
```

---

### Task 2: Write failing tests for LogHighlighter

**Files:**
- Create: `test/logHighlight.test.ts`

- [ ] **Step 1: Write the test file**

```ts
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
```

- [ ] **Step 2: Run tests to verify they fail (module does not exist yet)**

Run: `npm test`
Expected: FAIL — `Cannot find module '../src/lib/logHighlight'`.

- [ ] **Step 3: Commit the failing tests**

```bash
git add test/logHighlight.test.ts
git commit -m "test: failing tests for log keyword highlighting"
```

---

### Task 3: Implement LogHighlighter

**Files:**
- Create: `src/lib/logHighlight.ts`

- [ ] **Step 1: Implement the class**

```ts
const KEYWORD_RULES: { pattern: RegExp; color: number }[] = [
  { pattern: /\bERROR\b/g, color: 31 },
  { pattern: /\bWARN(?:ING)?\b/g, color: 33 },
  { pattern: /\bINFO\b/g, color: 32 },
  { pattern: /\bDEBUG\b/g, color: 90 },
  { pattern: /\bTRACE\b/g, color: 35 },
  { pattern: /\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}\b/g, color: 36 },
];

const ALT_ON = ['\x1b[?1049h', '\x1b[?1047h', '\x1b[?47h'];
const ALT_OFF = ['\x1b[?1049l', '\x1b[?1047l', '\x1b[?47l'];
const ALT_SEQ_MAX = 8; // "\x1b[?1049h" is the longest

const decoder = new TextDecoder('utf-8');
const encoder = new TextEncoder();

export class LogHighlighter {
  private pending: number[] = [];
  private alternateScreen = false;
  private altTail: number[] = [];

  reset(): void {
    this.pending = [];
    this.alternateScreen = false;
    this.altTail = [];
  }

  feed(chunk: Uint8Array): Uint8Array {
    this.updateAlternateScreen(chunk);
    const result: number[] = [];
    for (let i = 0; i < chunk.length; i++) {
      const byte = chunk[i];
      if (byte === 0x0a) {
        result.push(...this.highlightLine(new Uint8Array(this.pending)));
        this.pending = [];
        result.push(0x0a);
      } else {
        this.pending.push(byte);
      }
    }
    return new Uint8Array(result);
  }

  /** Emit any remaining partial line (used when the session closes). */
  flush(): Uint8Array {
    const line = new Uint8Array(this.pending);
    this.pending = [];
    if (line.length === 0) return new Uint8Array(0);
    return this.highlightLine(line);
  }

  private updateAlternateScreen(chunk: Uint8Array): void {
    const combined = new Uint8Array([...this.altTail, ...chunk]);
    const text = decoder.decode(combined);
    for (const seq of ALT_OFF) {
      if (text.includes(seq)) this.alternateScreen = false;
    }
    for (const seq of ALT_ON) {
      if (text.includes(seq)) this.alternateScreen = true;
    }
    this.altTail = Array.from(chunk.slice(-ALT_SEQ_MAX));
  }

  private highlightLine(line: Uint8Array): Uint8Array {
    if (this.alternateScreen) return line;

    const out: number[] = [];
    let i = 0;
    let segStart = 0;
    const n = line.length;

    const flushText = (end: number) => {
      if (end > segStart) {
        out.push(...this.highlightText(line.subarray(segStart, end)));
      }
    };

    while (i < n) {
      if (line[i] === 0x1b && i + 1 < n && line[i + 1] === 0x5b) {
        flushText(i);
        let j = i + 2;
        while (j < n && !(line[j] >= 0x40 && line[j] <= 0x7e)) j++;
        const csiEnd = j < n ? j + 1 : n;
        out.push(...line.subarray(i, csiEnd));
        i = csiEnd;
        segStart = i;
      } else {
        i++;
      }
    }
    flushText(n);
    return new Uint8Array(out);
  }

  private highlightText(segment: Uint8Array): Uint8Array {
    if (segment.length === 0) return segment;
    let text = decoder.decode(segment);
    for (const rule of KEYWORD_RULES) {
      text = text.replace(rule.pattern, (m) => `\x1b[${rule.color}m${m}\x1b[0m`);
    }
    return encoder.encode(text);
  }
}
```

Note: the CSI scan in `highlightLine` treats any `\x1b[` … final byte (0x40–0x7E) as an escape sequence and copies it verbatim, so pre-existing ANSI output is preserved and never re-matched.

- [ ] **Step 2: Run tests to verify they pass**

Run: `npm test`
Expected: all 11 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src/lib/logHighlight.ts
git commit -m "feat: log keyword highlighter for terminal output"
```

---

### Task 4: Integrate into Terminal.tsx

**Files:**
- Modify: `src/components/Terminal.tsx`

- [ ] **Step 1: Import the highlighter**

Add to the imports at the top (after the `sshResize` import on line 6):

```ts
import { LogHighlighter } from '../lib/logHighlight';
```

- [ ] **Step 2: Add a ref for the highlighter**

After `const fontSize = useStore(...)` (line 40), add:

```ts
const highlightRef = useRef<LogHighlighter | null>(null);
```

- [ ] **Step 3: Instantiate when the terminal is created**

In the init `useEffect`, right after `fitRef.current = fitAddon;` (line 103), add:

```ts
highlightRef.current = new LogHighlighter();
```

- [ ] **Step 4: Route output through the highlighter**

Replace the body of the `ssh://output` listener (currently line 127):

```ts
term.write(new Uint8Array(event.payload.data));
```

with:

```ts
const hl = highlightRef.current;
const data = new Uint8Array(event.payload.data);
term.write(hl ? hl.feed(data) : data);
```

- [ ] **Step 5: Flush remaining buffer on exit**

In the `ssh://exit` listener, immediately before the existing `term.write(\`\r\n\x1b[31mConnection closed ...\`` call, add:

```ts
const hl = highlightRef.current;
if (hl) term.write(hl.flush());
```

- [ ] **Step 6: Reset highlighter on Ctrl+Shift+R**

In the `attachCustomKeyEventHandler` handler where `term.reset()` is called (line 118), add after it:

```ts
highlightRef.current?.reset();
```

- [ ] **Step 7: Reset highlighter when the session changes**

In the `useEffect` keyed on `tab.sessionId` (line 79), after `sessionRef.current = tab.sessionId;`, add:

```ts
highlightRef.current?.reset();
```

- [ ] **Step 8: Clear the ref on cleanup**

In the init effect's cleanup function, after `fitRef.current = null;` (line 148), add:

```ts
highlightRef.current = null;
```

- [ ] **Step 9: Build and verify**

Run: `npm run build`
Expected: build succeeds with no TypeScript errors.

- [ ] **Step 10: Run e2e tests**

Run: `npx playwright test`
Expected: 14 tests pass.

- [ ] **Step 11: Commit**

```bash
git add src/components/Terminal.tsx
git commit -m "feat: highlight log keywords in SSH terminal output"
```

---

### Task 5: Bump version and push

**Files:**
- Modify: `package.json` (0.8.5 → 0.8.6)
- Modify: `src-tauri/Cargo.toml` (0.8.5 → 0.8.6)
- Modify: `src-tauri/tauri.conf.json` (0.8.5 → 0.8.6)

- [ ] **Step 1: Bump `package.json`**

Change `"version": "0.8.5"` to `"version": "0.8.6"`.

- [ ] **Step 2: Bump `src-tauri/Cargo.toml`**

Change `version = "0.8.5"` to `version = "0.8.6"`.

- [ ] **Step 3: Bump `src-tauri/tauri.conf.json`**

Change `"version": "0.8.5"` to `"version": "0.8.6"`.

- [ ] **Step 4: Commit and push**

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.8.6"
git push origin main
```

- [ ] **Step 5: Final verification**

Run: `npm test && npm run build`
Expected: unit tests pass, build succeeds, `git status` clean.
