# Log Keyword Highlight in SSH Terminal — Design

Date: 2026-08-14
Status: Approved

## Goal

When viewing logs (e.g. `tail -f`) inside the embedded SSH terminal, automatically
color-code special keywords (ERROR, WARN, INFO, timestamps, …) so the output is
easier to scan. Highlighting applies to **all terminal output** (auto), but is
**skipped while a full-screen TUI** (htop, vim, …) is active so those apps are
not visually broken.

## Context

- SSH terminal is rendered with `@xterm/xterm` (v5.5.0) + `@xterm/addon-fit`.
- Output flows: Rust reader thread → `app.emit("ssh://output", { sessionId, data: number[] })`
  → frontend `listen('ssh://output')` → `term.write(new Uint8Array(data))`
  (`src/components/Terminal.tsx`).
- Data arrives in arbitrary chunks (8 KB pty reads); a line may be split across chunks.
- Existing ANSI escape sequences from the remote (e.g. `grep --color`) must not be corrupted.
- Full-screen TUIs enter the alternate screen buffer via `\x1b[?1049h` (and variants);
  they exit with `\x1b[?1049l`.
- No unit-test framework exists yet (only Playwright e2e); `vitest` will be added as a devDependency.

## Approach

**Approach A (chosen): frontend byte-stream transformer.**

A pure-TS module `src/lib/logHighlight.ts` intercepts the output bytes, buffers by
line, wraps matching keywords in ANSI SGR color codes, and returns the transformed
bytes for `term.write`. No backend changes.

## Components

### 1. `src/lib/logHighlight.ts`

```ts
export class LogHighlighter {
  feed(chunk: Uint8Array): Uint8Array; // returns bytes safe for term.write
  reset(): void;
}
```

State:
- `pending: number[]` — bytes of the current incomplete line (no `\n` yet).
- `alternateScreen: boolean` — true while inside the alternate screen buffer.

`feed(chunk)`:
1. Append chunk bytes to `pending`.
2. Split on `\n`; for every complete line run `highlightLine(lineBytes)`, keep the
   trailing partial as the new `pending`.
3. Re-join the highlighted lines with `\n`, preserving the original newline count.

`highlightLine(line)`:
- If `alternateScreen` is true → return the line unchanged.
- Scan for ANSI CSI sequences (`\x1b[...`); treat everything else as plain text.
- In each plain-text segment, apply the keyword regexes, wrapping matches as
  `\x1b[<color>m<match>\x1b[0m`.
- Preserve existing escape sequences verbatim; do not nest color codes.

TUI detection (tracked across the whole stream, not just per line):
- `\x1b[?1049h`, `\x1b[?1047h`, `\x1b[?47h` → `alternateScreen = true`.
- `\x1b[?1049l`, `\x1b[?1047l`, `\x1b[?47l` → `alternateScreen = false`.

### 2. Default keyword palette

| Pattern | SGR color |
|---|---|
| `\bERROR\b` | 31 (red) |
| `\bWARN(ING)?\b` | 33 (yellow) |
| `\bINFO\b` | 32 (green) |
| `\bDEBUG\b` | 90 (bright black / gray) |
| `\bTRACE\b` | 35 (magenta) |
| timestamp `\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}` | 36 (cyan) |

Regexes are global and word-boundary delimited for the level keywords.

### 3. Integration in `src/components/Terminal.tsx`

- Create `highlightRef = useRef(new LogHighlighter())` once when xterm is initialized.
- In the `ssh://output` listener, replace
  `term.write(new Uint8Array(event.payload.data))` with
  `term.write(highlightRef.current.feed(new Uint8Array(event.payload.data)))`.
- Call `highlightRef.current.reset()` together with `term.reset()` (Ctrl+Shift+R handler).

### 4. Tests (`test/logHighlight.test.ts`, vitest)

- `ERROR` inside a line is wrapped in `\x1b[31m … \x1b[0m`.
- A keyword split across two `feed()` chunks is still highlighted.
- After `\x1b[?1049h` the input passes through untouched (no highlight).
- After `\x1b[?1049l` highlighting resumes.
- Multibyte UTF-8 characters are never split mid-codepoint.
- Pre-existing ANSI escape sequences in the output are preserved verbatim.
- INFO / WARN / DEBUG / TRACE / timestamp produce their expected colors.

## Error handling

- If a chunk ends mid-UTF-8-codepoint, the partial codepoint bytes remain in
  `pending` until the next chunk completes them; highlighting operates on complete
  lines only, so no splitting occurs.
- Invalid/partial escape sequences are treated as plain text and passed through.

## Performance

- Per-line regex on plain-text segments only; `tail -f` throughput is trivially
  handled. No per-keystroke work.

## Out of scope

- User-configurable keyword list (default set only for now).
- Backend-side highlighting.
- Highlighting inside TUI alternate-screen apps.
