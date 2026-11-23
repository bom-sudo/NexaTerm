import { useEffect, useState } from "react";
import { ShieldCheck, Trash2 } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { toast } from "../../store/ui";
import { EmptyState, IconButton, confirmDialog } from "../../components/ui";
import type { KnownHost } from "../../types";

export function KnownHostsView() {
  const [hosts, setHosts] = useState<KnownHost[]>([]);
  const [q, setQ] = useState("");
  const load = () => api.knownHostsList().then(setHosts).catch((e) => toast("error", toApiError(e).message));
  useEffect(() => {
    load();
  }, []);
  const list = hosts.filter((h) => `${h.host} ${h.fingerprint}`.toLowerCase().includes(q.toLowerCase()));
  return (
    <div className="flex h-full flex-col rounded-lg border border-line bg-panel">
      <div className="flex items-center gap-2 border-b border-line px-4 py-3">
        <ShieldCheck size={16} className="text-accent" />
        <div className="text-[14px] font-semibold">Known Hosts</div>
        <div className="text-[12px] text-dim">Server keys you have trusted. Removing one makes NexaTerm ask again on the next connection.</div>
        <div className="flex-1" />
        <input className="field !w-64" placeholder="Filter…" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      {hosts.length === 0 ? (
        <EmptyState icon={<ShieldCheck size={36} />} title="No trusted hosts yet">Host keys are added here when you accept a server's fingerprint.</EmptyState>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full text-[12.5px]">
            <thead className="sticky top-0 bg-panel-2 text-left text-[11.5px] text-dim">
              <tr>{["Host", "Port", "Key type", "SHA-256 fingerprint", "Trusted", ""].map((h) => <th key={h} className="px-3 py-2 font-medium">{h}</th>)}</tr>
            </thead>
            <tbody>
              {list.map((h) => (
                <tr key={`${h.host}:${h.port}`} className="border-t border-line hover:bg-hover">
                  <td className="px-3 py-2 font-mono">{h.host}</td>
                  <td className="px-3 py-2">{h.port}</td>
                  <td className="px-3 py-2">{h.keyType}</td>
                  <td className="px-3 py-2 font-mono text-[11px] select-text">{h.fingerprint}</td>
                  <td className="px-3 py-2 text-dim">{new Date(h.addedAt).toLocaleString()}</td>
                  <td className="px-3 py-2 text-right">
                    <IconButton title="Remove" onClick={async () => { if (await confirmDialog({ title: "Remove known host", message: `Forget the host key of ${h.host}:${h.port}? You will be asked to verify it again.`, confirmText: "Remove", danger: true })) { await api.knownHostRemove(h.host, h.port); load(); } }}>
                      <Trash2 size={14} />
                    </IconButton>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
