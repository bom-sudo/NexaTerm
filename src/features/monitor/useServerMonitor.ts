import { useEffect, useRef, useState } from "react";
import { api } from "../../services/api";
import { computeView, type MonitorView, type RemoteStats } from "./serverStats";

const INTERVAL_MS = 3000;

export type MonitorState =
  | { kind: "off" }
  | { kind: "loading" }
  | { kind: "unsupported"; os: string }
  | { kind: "ok"; view: MonitorView };

/**
 * Polls the server behind an SSH terminal every 3 s while the window is visible.
 * Stops for non-Linux servers; a failed sample just keeps the last values.
 */
export function useServerMonitor(terminalId: string | undefined, enabled: boolean): MonitorState {
  const [state, setState] = useState<MonitorState>({ kind: "off" });
  const prev = useRef<{ stats: RemoteStats; at: number } | null>(null);

  useEffect(() => {
    prev.current = null;
    if (!enabled || !terminalId) {
      setState({ kind: "off" });
      return;
    }
    setState({ kind: "loading" });
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;

    const tick = async () => {
      if (stopped) return;
      if (!document.hidden) {
        try {
          const stats = await api.sshMonitorSample(terminalId);
          if (stopped) return;
          if (!stats.supported) {
            setState({ kind: "unsupported", os: stats.os });
            return; // no point polling a server without /proc
          }
          const now = performance.now();
          const view = computeView(prev.current?.stats ?? null, stats, prev.current ? now - prev.current.at : 0);
          prev.current = { stats, at: now };
          setState({ kind: "ok", view });
        } catch {
          // Connection busy or closed: keep what is shown; the pane status shows real disconnects.
        }
      }
      timer = setTimeout(tick, INTERVAL_MS);
    };
    void tick();
    return () => {
      stopped = true;
      if (timer) clearTimeout(timer);
    };
  }, [terminalId, enabled]);

  return state;
}
