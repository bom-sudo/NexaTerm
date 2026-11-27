import { ArrowDown, ArrowUp, Clock, Cpu, HardDrive, MemoryStick } from "lucide-react";
import type { ReactNode } from "react";
import { formatKb, formatRate, formatUptime, level, type MonitorView } from "./serverStats";
import type { MonitorState } from "./useServerMonitor";

const barColor = { ok: "bg-ok", warn: "bg-warn", err: "bg-err" } as const;

function Meter({ icon, label, pct, text, title }: { icon: ReactNode; label: string; pct: number | null; text: string; title: string }) {
  return (
    <div className="flex items-center gap-1.5 border-l border-line px-2.5" title={title}>
      <span className="text-faint">{icon}</span>
      <span className="text-faint">{label}</span>
      <span className="relative h-1.5 w-12 overflow-hidden rounded-full bg-panel-2" aria-hidden="true">
        {pct != null && <span className={`absolute inset-y-0 left-0 rounded-full ${barColor[level(pct)]}`} style={{ width: `${Math.max(2, pct)}%` }} />}
      </span>
      <span className="tabular-nums text-dim">{text}</span>
    </div>
  );
}

function Details(v: MonitorView) {
  const memTitle = `Memory used ${formatKb(v.memUsedKb)} of ${formatKb(v.memTotalKb)}` + (v.swapTotalKb > 0 ? ` · swap ${formatKb(v.swapUsedKb)} of ${formatKb(v.swapTotalKb)}` : "");
  return (
    <>
      <Meter
        icon={<Cpu size={12} />}
        label="CPU"
        pct={v.cpuPct}
        text={v.cpuPct == null ? "…" : `${Math.round(v.cpuPct)}%`}
        title={`CPU usage across ${v.cpus || "?"} core(s) · load average ${v.load1.toFixed(2)}`}
      />
      <Meter icon={<MemoryStick size={12} />} label="RAM" pct={v.memPct} text={`${formatKb(v.memUsedKb)} / ${formatKb(v.memTotalKb)}`} title={memTitle} />
      <Meter
        icon={<HardDrive size={12} />}
        label="Disk /"
        pct={v.diskPct}
        text={`${Math.round(v.diskPct)}%`}
        title={`Root file system: ${formatKb(v.diskUsedKb)} used of ${formatKb(v.diskTotalKb)}`}
      />
      <div className="flex items-center gap-1.5 border-l border-line px-2.5 tabular-nums text-dim" title="Network traffic of the server (all interfaces except loopback)">
        <ArrowDown size={12} className="text-faint" />
        {v.rxPerSec == null ? "…" : formatRate(v.rxPerSec)}
        <ArrowUp size={12} className="text-faint" />
        {v.txPerSec == null ? "…" : formatRate(v.txPerSec)}
      </div>
      <div className="flex items-center gap-1.5 border-l border-line px-2.5 text-dim" title="Time since the server started">
        <Clock size={12} className="text-faint" />
        {formatUptime(v.uptimeSecs)}
      </div>
    </>
  );
}

export function ServerMonitorBar({ state }: { state: MonitorState }) {
  if (state.kind === "off") return null;
  if (state.kind === "loading") return <div className="border-l border-line px-2.5 text-faint">Reading server stats…</div>;
  if (state.kind === "unsupported")
    return (
      <div className="border-l border-line px-2.5 text-faint" title="The resource monitor reads /proc, which only Linux servers provide.">
        Monitor: not available on {state.os || "this server"}
      </div>
    );
  return Details(state.view);
}
