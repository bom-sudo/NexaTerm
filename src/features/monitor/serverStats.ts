// Turns two raw samples from the server into the numbers shown in the status bar.

export interface RemoteStats {
  supported: boolean;
  os: string;
  cpuTotal: number;
  cpuIdle: number;
  cpus: number;
  memTotalKb: number;
  memAvailableKb: number;
  swapTotalKb: number;
  swapFreeKb: number;
  diskTotalKb: number;
  diskUsedKb: number;
  uptimeSecs: number;
  load1: number;
  rxBytes: number;
  txBytes: number;
}

export interface MonitorView {
  /** null until two samples exist (CPU and network need a difference). */
  cpuPct: number | null;
  memUsedKb: number;
  memTotalKb: number;
  memPct: number;
  swapUsedKb: number;
  swapTotalKb: number;
  diskUsedKb: number;
  diskTotalKb: number;
  diskPct: number;
  rxPerSec: number | null;
  txPerSec: number | null;
  uptimeSecs: number;
  load1: number;
  cpus: number;
}

const pct = (part: number, whole: number) => (whole > 0 ? Math.min(100, Math.max(0, (part / whole) * 100)) : 0);

export function computeView(prev: RemoteStats | null, cur: RemoteStats, elapsedMs: number): MonitorView {
  let cpuPct: number | null = null;
  let rxPerSec: number | null = null;
  let txPerSec: number | null = null;
  if (prev) {
    const dTotal = cur.cpuTotal - prev.cpuTotal;
    const dIdle = cur.cpuIdle - prev.cpuIdle;
    if (dTotal > 0) cpuPct = pct(dTotal - dIdle, dTotal);
    const secs = elapsedMs / 1000;
    // Counters can reset (interface restarted): show nothing rather than a negative rate.
    if (secs > 0 && cur.rxBytes >= prev.rxBytes && cur.txBytes >= prev.txBytes) {
      rxPerSec = (cur.rxBytes - prev.rxBytes) / secs;
      txPerSec = (cur.txBytes - prev.txBytes) / secs;
    }
  }
  const memUsedKb = Math.max(0, cur.memTotalKb - cur.memAvailableKb);
  return {
    cpuPct,
    memUsedKb,
    memTotalKb: cur.memTotalKb,
    memPct: pct(memUsedKb, cur.memTotalKb),
    swapUsedKb: Math.max(0, cur.swapTotalKb - cur.swapFreeKb),
    swapTotalKb: cur.swapTotalKb,
    diskUsedKb: cur.diskUsedKb,
    diskTotalKb: cur.diskTotalKb,
    diskPct: pct(cur.diskUsedKb, cur.diskTotalKb),
    rxPerSec,
    txPerSec,
    uptimeSecs: cur.uptimeSecs,
    load1: cur.load1,
    cpus: cur.cpus,
  };
}

/** 1536 KB -> "1.5 GB" style for memory and disk (values in KiB). */
export function formatKb(kb: number): string {
  const gb = kb / 1024 / 1024;
  if (gb >= 100) return `${Math.round(gb)} GB`;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  return `${Math.round(kb / 1024)} MB`;
}

export function formatRate(bytesPerSec: number): string {
  if (bytesPerSec >= 1024 * 1024) return `${(bytesPerSec / 1024 / 1024).toFixed(1)} MB/s`;
  if (bytesPerSec >= 1024) return `${Math.round(bytesPerSec / 1024)} KB/s`;
  return `${Math.round(bytesPerSec)} B/s`;
}

export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/** Colour level for a usage bar. */
export function level(p: number): "ok" | "warn" | "err" {
  return p >= 90 ? "err" : p >= 75 ? "warn" : "ok";
}
