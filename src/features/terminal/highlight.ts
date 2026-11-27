/**
 * Output keyword highlighter for the terminal byte stream.
 *
 * ใส่สีให้คำสำคัญในผลลัพธ์ของ terminal (error / warning / success / IPv4)
 * แบบเดียวกับ syntax highlighting ของ MobaXterm โดยไม่แตะต้อง escape sequence,
 * ไม่ทับสีที่โปรแกรมกำหนดเอง และหยุดทำงานทั้งหมดเมื่อโปรแกรมใช้ alternate screen
 * (vim, less, htop)
 *
 * Design notes (technical):
 * - Works directly on bytes. Text runs are converted to a 1:1 "latin1" string
 *   (one char per byte) so regex match indices are byte offsets; non-ASCII
 *   bytes can never match the ASCII-only token patterns, so UTF-8 text is
 *   passed through byte-identical.
 * - A small VT escape-sequence state machine persists across chunks. Escape
 *   bytes are never modified; only text runs in the ground state get SGR
 *   codes inserted around tokens.
 * - Bytes may be held back at the end of a chunk (incomplete UTF-8 char,
 *   incomplete escape sequence, or a trailing partial token) and are
 *   prepended to the next chunk or released by flush().
 */

export interface HighlightOptions {
  enabled: boolean;
  /**
   * A trailing partial word is only carried over to the next chunk when the
   * incoming chunk is at least this many bytes long. Small chunks are almost
   * always keystroke echo, and holding them back would delay what the user
   * types. Bulk output chunks are far larger than this.
   */
  carryMinChunk: number;
}

const DEFAULTS: HighlightOptions = { enabled: true, carryMinChunk: 16 };

/** Error words: bright red (SGR 91). */
export const ERROR_WORDS: readonly string[] = [
  'error', 'errors', 'failed', 'failure', 'fail', 'fatal', 'denied', 'refused',
  'critical', 'panic', 'exception', 'not found',
];
/** Warning words: bright yellow (SGR 93). */
export const WARNING_WORDS: readonly string[] = [
  'warning', 'warn', 'deprecated', 'timeout', 'timed out',
];
/** Success words: bright green (SGR 92). */
export const SUCCESS_WORDS: readonly string[] = [
  'ok', 'success', 'succeeded', 'done', 'passed', 'active (running)',
];

const ESC = 0x1b;
const BEL = 0x07;
const CAN = 0x18;
const SUB = 0x1a;

/** Longest unterminated escape sequence that will be held back. */
const MAX_ESC_HOLD = 4096;
/** Longest CSI parameter string we keep for interpretation. */
const MAX_CSI_PARAMS = 256;
/** "255.255.255.255:65535" */
const MAX_IP_LEN = 21;

const S_GROUND = 0;
const S_ESC = 1;
const S_ESC_INTER = 2;
const S_CSI = 3;
const S_STR = 4;
const S_STR_ESC = 5;

function ascii(s: string): Uint8Array {
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

const SGR_ERROR = ascii('\x1b[91m');
const SGR_WARNING = ascii('\x1b[93m');
const SGR_SUCCESS = ascii('\x1b[92m');
const SGR_IP = ascii('\x1b[96m');
const SGR_RESET_FG = ascii('\x1b[39m');
const EMPTY = new Uint8Array(0);

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function alternation(words: readonly string[]): string {
  return [...words].sort((a, b) => b.length - a.length).map(escapeRe).join('|');
}

const OCTET = '(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)';
const IPV4 =
  `(?<![A-Za-z0-9_.])(?:${OCTET}\\.){3}${OCTET}` +
  `(?:/(?:3[0-2]|[12]?\\d)|:\\d{1,5})?(?![A-Za-z0-9_]|\\.\\d)`;

/** Groups: 1 = error, 2 = warning, 3 = success, 4 = IPv4. */
const TOKEN_RE = new RegExp(
  `(?<![A-Za-z0-9_])(?:(${alternation(ERROR_WORDS)})|(${alternation(WARNING_WORDS)})|(${alternation(SUCCESS_WORDS)}))(?![A-Za-z0-9_])` +
    `|(${IPV4})`,
  'gi',
);

const ALL_PHRASES: readonly string[] = [...ERROR_WORDS, ...WARNING_WORDS, ...SUCCESS_WORDS];

function isWordByte(b: number): boolean {
  return (
    (b >= 0x30 && b <= 0x39) || (b >= 0x41 && b <= 0x5a) || (b >= 0x61 && b <= 0x7a) || b === 0x5f
  );
}

function isIpByte(b: number): boolean {
  return (b >= 0x30 && b <= 0x39) || b === 0x2e || b === 0x3a || b === 0x2f;
}

/** One JS char per byte, so string indices equal byte offsets. */
function bytesToLatin1(buf: Uint8Array, a: number, b: number): string {
  const STEP = 0x2000;
  let s = '';
  for (let i = a; i < b; i += STEP) {
    const sub = buf.subarray(i, Math.min(b, i + STEP));
    s += String.fromCharCode.apply(null, sub as unknown as number[]);
  }
  return s;
}

function concatBytes(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}

export class OutputHighlighter {
  private enabled: boolean;
  private readonly carryMinChunk: number;
  /** Bytes held back from the previous call (always parsed from S_GROUND). */
  private pending: Uint8Array = EMPTY;

  // Escape parser state (persists across calls).
  private state = S_GROUND;
  private strIsOsc = false;
  private params = '';
  private paramsOverflow = false;
  private inter = '';

  // Terminal model.
  private altScreen = false;
  private fgDefault = true;
  private savedFgDefault = true;
  /** Last text byte emitted, or -1 after an escape sequence / at start. */
  private prevByte = -1;

  // Output of the current call.
  private parts: Uint8Array[] = [];
  private outLen = 0;
  private emitting = true;

  constructor(opts?: Partial<HighlightOptions>) {
    const o = { ...DEFAULTS, ...opts };
    this.enabled = o.enabled;
    this.carryMinChunk = o.carryMinChunk;
  }

  setEnabled(on: boolean): void {
    this.enabled = on;
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  /**
   * True when bytes are being held back. Callers should schedule flush() on a
   * short idle timer (for example 20-50 ms) while this is true, so held bytes
   * never stay invisible.
   */
  hasPending(): boolean {
    return this.pending.length > 0;
  }

  process(chunk: Uint8Array): Uint8Array {
    const buf = this.pending.length > 0 ? concatBytes(this.pending, chunk) : chunk;
    this.pending = EMPTY;
    if (!this.enabled) {
      // Passthrough, but keep tracking escape / alt-screen / colour state.
      this.run(buf, false, false, chunk.length);
      return buf;
    }
    this.run(buf, true, true, chunk.length);
    return this.collect();
  }

  flush(): Uint8Array {
    if (this.pending.length === 0) return EMPTY;
    const buf = this.pending;
    this.pending = EMPTY;
    if (!this.enabled) {
      this.run(buf, false, false, buf.length);
      return buf;
    }
    this.run(buf, false, true, buf.length);
    return this.collect();
  }

  // ---------------------------------------------------------------------------

  private collect(): Uint8Array {
    const parts = this.parts;
    const len = this.outLen;
    this.parts = [];
    this.outLen = 0;
    if (parts.length === 0) return EMPTY;
    if (parts.length === 1) return parts[0];
    const out = new Uint8Array(len);
    let o = 0;
    for (const p of parts) {
      out.set(p, o);
      o += p.length;
    }
    return out;
  }

  private push(p: Uint8Array): void {
    if (p.length === 0) return;
    this.parts.push(p);
    this.outLen += p.length;
  }

  private resetEscape(): void {
    this.params = '';
    this.paramsOverflow = false;
    this.inter = '';
  }

  private highlightActive(): boolean {
    return !this.altScreen && this.fgDefault;
  }

  private run(buf: Uint8Array, allowHold: boolean, emit: boolean, chunkLen: number): void {
    this.emitting = emit;
    const n = buf.length;
    let i = 0;
    // An escape continued from a previous call (not held) starts at offset 0.
    let escStart = 0;
    let escFromPrev = this.state !== S_GROUND;

    const raw = (a: number, b: number): void => {
      if (emit && b > a) this.push(buf.subarray(a, b));
    };
    const complete = (end: number): void => {
      raw(escStart, end);
      this.state = S_GROUND;
      this.prevByte = -1;
      escFromPrev = false;
    };
    const restart = (at: number): void => {
      // ESC inside an escape sequence aborts it and starts a new one.
      raw(escStart, at);
      escStart = at;
      escFromPrev = false;
      this.state = S_ESC;
      this.resetEscape();
      this.prevByte = -1;
    };

    while (i < n) {
      const st = this.state;
      if (st === S_GROUND) {
        const e = buf.indexOf(ESC, i);
        if (e < 0) {
          // Trailing text: possibly hold back a partial char / token.
          let holdFrom = n;
          if (allowHold && emit && this.highlightActive()) {
            holdFrom = this.computeHold(buf, i, n, chunkLen);
          }
          this.emitText(buf, i, holdFrom);
          if (holdFrom < n) this.pending = buf.slice(holdFrom, n);
          return;
        }
        this.emitText(buf, i, e);
        escStart = e;
        escFromPrev = false;
        this.state = S_ESC;
        this.resetEscape();
        this.prevByte = -1;
        i = e + 1;
        continue;
      }

      const b = buf[i];
      switch (st) {
        case S_ESC:
          if (b === 0x5b) {
            this.state = S_CSI;
          } else if (b === 0x5d) {
            this.state = S_STR;
            this.strIsOsc = true;
          } else if (b === 0x50 || b === 0x5f || b === 0x5e || b === 0x58) {
            this.state = S_STR;
            this.strIsOsc = false;
          } else if (b >= 0x20 && b <= 0x2f) {
            this.inter += String.fromCharCode(b);
            this.state = S_ESC_INTER;
          } else if (b === ESC) {
            restart(i);
          } else if (b === CAN || b === SUB) {
            complete(i + 1);
          } else if (b >= 0x30 && b <= 0x7e) {
            this.escEffect(b);
            complete(i + 1);
          }
          // Other C0 controls execute inside the sequence; stay in state.
          break;
        case S_ESC_INTER:
          if (b >= 0x20 && b <= 0x2f) {
            if (this.inter.length < 8) this.inter += String.fromCharCode(b);
          } else if (b >= 0x30 && b <= 0x7e) {
            complete(i + 1);
          } else if (b === ESC) {
            restart(i);
          } else if (b === CAN || b === SUB) {
            complete(i + 1);
          }
          break;
        case S_CSI:
          if (b >= 0x30 && b <= 0x3f) {
            if (this.params.length < MAX_CSI_PARAMS) this.params += String.fromCharCode(b);
            else this.paramsOverflow = true;
          } else if (b >= 0x20 && b <= 0x2f) {
            if (this.inter.length < 8) this.inter += String.fromCharCode(b);
          } else if (b >= 0x40 && b <= 0x7e) {
            this.csiEffect(b);
            complete(i + 1);
          } else if (b === ESC) {
            restart(i);
          } else if (b === CAN || b === SUB) {
            complete(i + 1);
          }
          break;
        case S_STR:
          if (b === ESC) {
            this.state = S_STR_ESC;
          } else if ((b === BEL && this.strIsOsc) || b === CAN || b === SUB) {
            complete(i + 1);
          }
          break;
        case S_STR_ESC:
          if (b === 0x5c) {
            complete(i + 1);
          } else {
            // The ESC ended the string and starts a new sequence; re-read b.
            restart(i - 1 < 0 ? 0 : i - 1);
            continue;
          }
          break;
      }
      i++;
    }

    // Ended inside an escape sequence.
    if (this.state !== S_GROUND) {
      const len = n - escStart;
      if (allowHold && emit && !escFromPrev && !this.altScreen && len <= MAX_ESC_HOLD) {
        this.pending = buf.slice(escStart, n);
        // The held bytes start with ESC and are re-parsed from ground.
        this.state = S_GROUND;
        this.resetEscape();
      } else {
        raw(escStart, n);
      }
    }
  }

  /** Index from which the tail of text run [a, n) should be held back. */
  private computeHold(buf: Uint8Array, a: number, n: number, chunkLen: number): number {
    // Incomplete UTF-8 sequence at the very end.
    for (let k = 1; k <= 3 && n - k >= a; k++) {
      const c = buf[n - k];
      if ((c & 0xc0) === 0x80) continue;
      if (c >= 0xc0) {
        const need = c >= 0xf0 ? 4 : c >= 0xe0 ? 3 : 2;
        if (need > k) return n - k;
      }
      break;
    }
    if (n === a || chunkLen < this.carryMinChunk) return n;

    const before = (pos: number): number => (pos > a ? buf[pos - 1] : this.prevByte);
    let hold = n;

    // Trailing prefix (or whole) of a keyword/phrase, starting at a word boundary.
    const avail = n - a;
    for (const phrase of ALL_PHRASES) {
      for (let k = Math.min(phrase.length, avail); k >= 1; k--) {
        const pos = n - k;
        if (pos >= hold) continue;
        let ok = true;
        for (let t = 0; t < k; t++) {
          let c = buf[pos + t];
          if (c >= 0x41 && c <= 0x5a) c |= 0x20;
          if (c !== phrase.charCodeAt(t)) {
            ok = false;
            break;
          }
        }
        if (!ok) continue;
        const p = before(pos);
        if (p < 0 || !isWordByte(p)) {
          hold = pos;
          break;
        }
      }
    }

    // Trailing partial IPv4 address (digits, dots, colon/port, slash/prefix).
    let j = n;
    while (j > a && isIpByte(buf[j - 1]) && n - j <= MAX_IP_LEN) j--;
    const runLen = n - j;
    if (runLen >= 1 && runLen <= MAX_IP_LEN && j < hold && buf[j] >= 0x30 && buf[j] <= 0x39) {
      const p = before(j);
      if (p < 0 || !(isWordByte(p) || p === 0x2e || isIpByte(p))) hold = j;
    }
    return hold;
  }

  private emitText(buf: Uint8Array, a: number, b: number): void {
    if (b <= a) return;
    if (!this.emitting || !this.highlightActive()) {
      if (this.emitting) this.push(buf.subarray(a, b));
      this.prevByte = buf[b - 1];
      return;
    }
    const pre = this.prevByte >= 0 ? String.fromCharCode(this.prevByte) : '';
    const off = pre.length;
    const s = pre + bytesToLatin1(buf, a, b);
    const re = TOKEN_RE;
    re.lastIndex = off;
    let last = a;
    let m: RegExpExecArray | null;
    while ((m = re.exec(s)) !== null) {
      const ms = a + m.index - off;
      const me = ms + m[0].length;
      const open =
        m[1] !== undefined ? SGR_ERROR
        : m[2] !== undefined ? SGR_WARNING
        : m[3] !== undefined ? SGR_SUCCESS
        : SGR_IP;
      this.push(buf.subarray(last, ms));
      this.push(open);
      this.push(buf.subarray(ms, me));
      this.push(SGR_RESET_FG);
      last = me;
    }
    this.push(buf.subarray(last, b));
    this.prevByte = buf[b - 1];
  }

  /** Two-byte ESC sequences that affect the colour model. */
  private escEffect(final: number): void {
    if (this.inter !== '') return;
    if (final === 0x37) {
      // DECSC saves rendition.
      this.savedFgDefault = this.fgDefault;
    } else if (final === 0x38) {
      // DECRC restores rendition.
      this.fgDefault = this.savedFgDefault;
    } else if (final === 0x63) {
      // RIS: full reset.
      this.altScreen = false;
      this.fgDefault = true;
      this.savedFgDefault = true;
    }
  }

  private csiEffect(final: number): void {
    if (this.paramsOverflow) return;
    const p = this.params;
    const inter = this.inter;
    if (final === 0x6d) {
      // SGR, only without private marker (CSI > 4;1 m is modifyOtherKeys).
      if (inter !== '') return;
      const c0 = p.charCodeAt(0);
      if (c0 >= 0x3c && c0 <= 0x3f) return;
      this.applySgr(p);
    } else if ((final === 0x68 || final === 0x6c) && inter === '' && p.charCodeAt(0) === 0x3f) {
      const set = final === 0x68;
      for (const mode of p.slice(1).split(';')) {
        if (mode === '1049') {
          if (set) {
            this.savedFgDefault = this.fgDefault;
            this.altScreen = true;
          } else {
            this.altScreen = false;
            this.fgDefault = this.savedFgDefault;
          }
        } else if (mode === '1047' || mode === '47') {
          this.altScreen = set;
        }
      }
    } else if (final === 0x73 && p === '' && inter === '') {
      this.savedFgDefault = this.fgDefault;
    } else if (final === 0x75 && p === '' && inter === '') {
      this.fgDefault = this.savedFgDefault;
    } else if (final === 0x70 && inter === '!') {
      // DECSTR soft reset clears rendition.
      this.fgDefault = true;
    }
  }

  private applySgr(p: string): void {
    if (p === '') {
      this.fgDefault = true;
      return;
    }
    const parts = p.split(';');
    for (let k = 0; k < parts.length; k++) {
      const part = parts[k];
      const colon = part.indexOf(':');
      const head = colon >= 0 ? part.slice(0, colon) : part;
      const v = head === '' ? 0 : parseInt(head, 10);
      if (v === 0 || v === 39) {
        this.fgDefault = true;
      } else if ((v >= 30 && v <= 37) || (v >= 90 && v <= 97)) {
        this.fgDefault = false;
      } else if (v === 38 || v === 48 || v === 58) {
        if (v === 38) this.fgDefault = false;
        // Skip extended colour arguments given as separate parameters.
        if (colon < 0) {
          const mode = parts[k + 1];
          if (mode === '5') k += 2;
          else if (mode === '2') k += 4;
        }
      }
    }
  }
}
