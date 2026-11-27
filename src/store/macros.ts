import { create } from "zustand";
import type { MacroStep } from "../types";

/** Pauses shorter than this are not recorded; longer ones are capped. */
const MIN_PAUSE_MS = 700;
const MAX_PAUSE_MS = 10_000;

interface MacroRecorder {
  /** Pane being recorded, or null. */
  paneId: string | null;
  steps: MacroStep[];
  lastAt: number;
  start: (paneId: string) => void;
  record: (paneId: string, data: string) => void;
  stop: () => MacroStep[];
}

/** Append input to a recording: merge consecutive text, insert waits for real pauses. */
export function appendStep(steps: MacroStep[], data: string, gapMs: number): MacroStep[] {
  const out = steps.slice();
  if (out.length > 0 && gapMs >= MIN_PAUSE_MS) out.push({ kind: "wait", ms: Math.min(Math.round(gapMs), MAX_PAUSE_MS) });
  const last = out[out.length - 1];
  if (last && last.kind === "text") out[out.length - 1] = { kind: "text", text: last.text + data };
  else out.push({ kind: "text", text: data });
  return out;
}

export const useMacroRecorder = create<MacroRecorder>((set, get) => ({
  paneId: null,
  steps: [],
  lastAt: 0,
  start: (paneId) => set({ paneId, steps: [], lastAt: performance.now() }),
  record: (paneId, data) => {
    const s = get();
    if (s.paneId !== paneId) return;
    const now = performance.now();
    set({ steps: appendStep(s.steps, data, now - s.lastAt), lastAt: now });
  },
  stop: () => {
    const steps = get().steps;
    set({ paneId: null, steps: [], lastAt: 0 });
    return steps;
  },
}));

/** Text a macro will send, for the confirmation preview. Control characters are made visible. */
export function describeSteps(steps: MacroStep[]): string {
  return steps
    .map((s) =>
      s.kind === "wait"
        ? `[wait ${(s.ms / 1000).toFixed(1)} s]`
        : s.text.replace(/\r/g, "⏎\n").replace(/[\x00-\x08\x0b-\x1f\x7f]/g, (c) => `^${String.fromCharCode(c.charCodeAt(0) ^ 0x40)}`),
    )
    .join("\n");
}
