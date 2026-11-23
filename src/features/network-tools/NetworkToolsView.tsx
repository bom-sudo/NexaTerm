import { useEffect, useRef, useState } from "react";
import { Activity, Ban, Globe, Network, Play, Plug, Route, Search, Server, Wifi } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { Badge, Button, Field } from "../../components/ui";
import { uid } from "../../utils/commands";

type Tool = "ping" | "traceroute" | "dns" | "reverse" | "tcp" | "interfaces" | "public";

const TOOLS: { id: Tool; label: string; icon: React.ReactNode; desc: string }[] = [
  { id: "ping", label: "Ping", icon: <Activity size={15} />, desc: "ICMP echo via Windows ping.exe" },
  { id: "traceroute", label: "Traceroute", icon: <Route size={15} />, desc: "Route to a host via tracert.exe" },
  { id: "dns", label: "DNS lookup", icon: <Search size={15} />, desc: "Resolve a host name (system resolver)" },
  { id: "reverse", label: "Reverse DNS", icon: <Globe size={15} />, desc: "PTR lookup for an IP address" },
  { id: "tcp", label: "TCP port test", icon: <Plug size={15} />, desc: "Check if one TCP port accepts connections" },
  { id: "interfaces", label: "Local interfaces", icon: <Network size={15} />, desc: "This PC's addresses and host name" },
  { id: "public", label: "Public IP", icon: <Wifi size={15} />, desc: "Ask api.ipify.org (only when you click)" },
];

export function NetworkToolsView() {
  const [tool, setTool] = useState<Tool>("ping");
  const [host, setHost] = useState("");
  const [port, setPort] = useState(22);
  const [count, setCount] = useState(4);
  const [out, setOut] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<{ tone: "ok" | "err" | "warn"; text: string } | null>(null);
  const job = useRef<string | null>(null);
  const outRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    outRef.current?.scrollTo(0, outRef.current.scrollHeight);
  }, [out]);
  useEffect(() => () => void (job.current && api.jobCancel(job.current)), []);

  const needsHost = ["ping", "traceroute", "dns", "reverse", "tcp"].includes(tool);
  const validHost = /^[A-Za-z0-9.\-_:[\]]+$/.test(host.trim()) && !host.trim().startsWith("-");

  const run = async () => {
    if (needsHost && !validHost) return setStatus({ tone: "err", text: "Enter a valid host name or IP address." });
    setOut([]);
    setStatus(null);
    setBusy(true);
    const h = host.trim();
    const push = (l: string) => setOut((o) => [...o, l]);
    try {
      if (tool === "ping" || tool === "traceroute") {
        job.current = uid();
        const code = await api.netRunTool(tool, h, count, job.current, push);
        setStatus(code === 0 ? { tone: "ok", text: "Completed" } : { tone: "warn", text: `Finished with exit code ${code}` });
      } else if (tool === "dns") {
        const r = await api.netDns(h);
        r.addresses.forEach(push);
        setStatus({ tone: "ok", text: `${r.addresses.length} address(es) in ${r.elapsedMs} ms` });
      } else if (tool === "reverse") {
        push(await api.netReverse(h));
        setStatus({ tone: "ok", text: "Resolved" });
      } else if (tool === "tcp") {
        const r = await api.netTcpProbe(h, port, 5000);
        push(r.open ? `${h}:${port} is OPEN (${r.address}) — ${r.elapsedMs} ms` : `${h}:${port} is not reachable — ${r.error} (${r.elapsedMs} ms)`);
        setStatus(r.open ? { tone: "ok", text: "Port open" } : { tone: "err", text: "Port closed / filtered" });
      } else if (tool === "interfaces") {
        push(`Host name: ${await api.netHostname()}`);
        for (const i of await api.netInterfaces()) push(`${i.name.padEnd(40)} ${i.address.padEnd(40)} ${i.netmask ?? ""}${i.isLoopback ? "  (loopback)" : ""}`);
        setStatus({ tone: "ok", text: "Done" });
      } else if (tool === "public") {
        push(`Public IP: ${await api.netPublicIp()}`);
        setStatus({ tone: "ok", text: "Fetched from api.ipify.org" });
      }
    } catch (e) {
      const err = toApiError(e);
      setStatus({ tone: err.kind === "cancelled" ? "warn" : "err", text: err.kind === "cancelled" ? "Cancelled" : err.message });
    } finally {
      job.current = null;
      setBusy(false);
    }
  };

  const meta = TOOLS.find((t) => t.id === tool)!;
  return (
    <div className="flex h-full gap-2">
      <div className="w-56 shrink-0 rounded-lg border border-line bg-panel p-2">
        <div className="mb-2 px-2 pt-1 text-[12px] font-semibold tracking-wide text-dim uppercase">Network tools</div>
        {TOOLS.map((t) => (
          <button
            key={t.id}
            disabled={busy}
            onClick={() => {
              setTool(t.id);
              setOut([]);
              setStatus(null);
            }}
            className={`mb-0.5 flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-[12.5px] disabled:opacity-60 ${tool === t.id ? "bg-accent-soft text-accent" : "hover:bg-hover"}`}
          >
            {t.icon} {t.label}
          </button>
        ))}
      </div>
      <div className="flex min-w-0 flex-1 flex-col rounded-lg border border-line bg-panel">
        <div className="border-b border-line p-4">
          <div className="text-[14px] font-semibold">{meta.label}</div>
          <div className="mb-3 text-[12px] text-dim">{meta.desc}</div>
          <form
            className="flex items-end gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (!busy) run();
            }}
          >
            {needsHost && (
              <div className="w-80">
                <Field label={tool === "reverse" ? "IP address" : "Host name or IP"}>
                  <input autoFocus className="field font-mono" value={host} onChange={(e) => setHost(e.target.value)} placeholder={tool === "reverse" ? "8.8.8.8" : "example.com"} />
                </Field>
              </div>
            )}
            {tool === "tcp" && (
              <div className="w-28">
                <Field label="Port">
                  <input className="field" type="number" min={1} max={65535} value={port} onChange={(e) => setPort(Number(e.target.value))} />
                </Field>
              </div>
            )}
            {tool === "ping" && (
              <div className="w-28">
                <Field label="Count">
                  <input className="field" type="number" min={1} max={100} value={count} onChange={(e) => setCount(Number(e.target.value))} />
                </Field>
              </div>
            )}
            {busy ? (
              <Button type="button" variant="danger" onClick={() => job.current && api.jobCancel(job.current)} disabled={!job.current}>
                <Ban size={13} /> Cancel
              </Button>
            ) : (
              <Button type="submit" variant="primary">
                <Play size={13} /> Run
              </Button>
            )}
            {status && <Badge tone={status.tone}>{status.text}</Badge>}
          </form>
        </div>
        <pre ref={outRef} className="min-h-0 flex-1 overflow-auto p-4 font-mono text-[12.5px] leading-relaxed whitespace-pre-wrap select-text">
          {out.length ? out.join("\n") : <span className="text-faint">{busy ? "Running…" : "Output appears here."}</span>}
        </pre>
        <div className="flex items-center gap-1.5 border-t border-line px-4 py-2 text-[11px] text-faint">
          <Server size={12} /> Diagnostics target one host at a time. Only probe systems you are authorized to test.
        </div>
      </div>
    </div>
  );
}
