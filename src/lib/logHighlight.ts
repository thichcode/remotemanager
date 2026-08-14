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
