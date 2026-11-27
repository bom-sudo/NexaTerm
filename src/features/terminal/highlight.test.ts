import { describe, expect, it } from "vitest";
import { OutputHighlighter } from "./highlight";

const enc = new TextEncoder();
const dec = new TextDecoder();
const ESC = "\x1b";

function run(h: OutputHighlighter, ...chunks: string[]): string {
  let out = "";
  for (const c of chunks) out += dec.decode(h.process(enc.encode(c)), { stream: true });
  out += dec.decode(h.flush());
  return out;
}

// Remove SGR (colour) sequences, to check that no visible content changed.
const stripSgr = (s: string) => s.replace(/\x1b\[[0-9;]*m/g, "");

describe("OutputHighlighter", () => {
  it("passes bytes through unchanged when disabled", () => {
    const h = new OutputHighlighter({ enabled: false });
    const input = enc.encode("error 10.0.0.1 failed");
    expect(Array.from(h.process(input))).toEqual(Array.from(input));
  });

  it("colours error, warning, success words and IPv4 without changing text", () => {
    const h = new OutputHighlighter({ enabled: true });
    const text = "Connection refused from 192.168.1.20:22, warning: disk 91%, status ok\r\n";
    const out = run(h, text);
    expect(stripSgr(out)).toBe(text);
    expect(out).toContain(`${ESC}[91mrefused${ESC}[39m`);
    expect(out).toContain(`${ESC}[96m192.168.1.20:22${ESC}[39m`);
    expect(out).toContain(`${ESC}[93mwarning${ESC}[39m`);
    expect(out).toContain(`${ESC}[92mok${ESC}[39m`);
  });

  it("matches whole words only", () => {
    const h = new OutputHighlighter({ enabled: true });
    const text = "token book errorless terrorize 999.1.1.1\r\n";
    expect(run(h, text)).toBe(text);
  });

  it("never touches escape sequences, even split across chunks", () => {
    const h = new OutputHighlighter({ enabled: true });
    const out = run(h, `${ESC}]0;error in title`, `${ESC}\\`, `${ESC}[3`, "1;1H", "plain\r\n");
    expect(out).toBe(`${ESC}]0;error in title${ESC}\\${ESC}[31;1Hplain\r\n`);
  });

  it("does not override colours set by the application, and resumes after reset", () => {
    const h = new OutputHighlighter({ enabled: true });
    const out = run(h, `${ESC}[31merror here${ESC}[0m then error\r\n`);
    expect(out.startsWith(`${ESC}[31merror here${ESC}[0m then `)).toBe(true);
    expect(out).toContain(`${ESC}[91merror${ESC}[39m`);
  });

  it("is completely inactive on the alternate screen (vim, less, htop)", () => {
    const h = new OutputHighlighter({ enabled: true });
    const inside = `${ESC}[?1049herror failed 10.0.0.1${ESC}[?1049l`;
    const out = run(h, inside, " error\r\n");
    expect(out.startsWith(inside)).toBe(true);
    expect(out).toContain(`${ESC}[91merror${ESC}[39m`);
  });

  it("highlights a word split across bulk output chunks", () => {
    const h = new OutputHighlighter({ enabled: true });
    const out = run(h, "make[2]: compiling module, build fai", "led\r\n");
    expect(out).toBe(`make[2]: compiling module, build ${ESC}[91mfailed${ESC}[39m\r\n`);
  });

  it("does not delay short chunks (keystroke echo) to wait for the rest of a word", () => {
    const h = new OutputHighlighter({ enabled: true });
    expect(dec.decode(h.process(enc.encode("fai")))).toBe("fai");
  });

  it("does not hold back a prompt or a line ending", () => {
    const h = new OutputHighlighter({ enabled: true });
    const first = dec.decode(h.process(enc.encode("password: ")));
    expect(first).toBe("password: ");
    expect(dec.decode(h.process(enc.encode("\r\n")))).toBe("\r\n");
  });

  it("passes Thai text and split UTF-8 characters through byte-identical", () => {
    const h = new OutputHighlighter({ enabled: true });
    const bytes = enc.encode("สวัสดี ข้อผิดพลาด\r\n");
    const a = h.process(bytes.slice(0, 4));
    const b = h.process(bytes.slice(4));
    const c = h.flush();
    expect(Array.from([...a, ...b, ...c])).toEqual(Array.from(bytes));
  });

  it("releases held-back bytes on flush and bounds the carry buffer", () => {
    const h = new OutputHighlighter({ enabled: true });
    const held = dec.decode(h.process(enc.encode("abc fai")));
    expect(held + dec.decode(h.flush())).toBe("abc fai");
    const long = "x".repeat(200);
    const out = dec.decode(h.process(enc.encode(long)));
    expect(out.length).toBeGreaterThanOrEqual(200 - 64);
  });

  it("processes 1 MB of text quickly", () => {
    const h = new OutputHighlighter({ enabled: true });
    const line = "INFO request completed status=200 from 10.1.2.3 in 12ms ok\r\n";
    const big = enc.encode(line.repeat(Math.ceil((1024 * 1024) / line.length)));
    const t0 = performance.now();
    const out = h.process(big);
    h.flush();
    expect(performance.now() - t0).toBeLessThan(400);
    expect(out.length).toBeGreaterThan(big.length);
  });
});
