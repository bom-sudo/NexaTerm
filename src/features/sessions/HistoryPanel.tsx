import { useEffect, useState } from "react";
import { Play, Trash2 } from "lucide-react";
import { api } from "../../services/api";
import { useSessions } from "../../store/sessions";
import { Badge, IconButton, confirmDialog } from "../../components/ui";
import { connectSession } from "../../app/actions";
import type { HistoryEntry } from "../../types";

export function HistoryPanel() {
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const byId = useSessions((s) => s.byId);
  const load = () => api.historyList(100).then(setItems).catch(() => {});
  useEffect(() => {
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, []);

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between p-2">
        <div className="text-[12px] text-dim">Recent connections</div>
        <IconButton
          title="Clear history"
          onClick={async () => {
            if (await confirmDialog({ title: "Clear history", message: "Remove all connection history entries?", confirmText: "Clear", danger: true })) {
              await api.historyClear();
              load();
            }
          }}
        >
          <Trash2 size={14} />
        </IconButton>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-1">
        {items.length === 0 && <div className="p-4 text-center text-[12px] text-dim">No connections yet.</div>}
        {items.map((h) => {
          const s = byId(h.sessionId);
          return (
            <div key={h.id} className="group flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-hover">
              <div className="min-w-0 flex-1">
                <div className="truncate text-[12.5px]">{h.label}</div>
                <div className="flex items-center gap-1.5 text-[11px] text-faint">
                  <span>{new Date(h.connectedAt).toLocaleString()}</span>
                  <Badge tone={h.outcome === "connected" ? "ok" : h.outcome === "auth failed" || h.outcome === "failed" ? "err" : "warn"}>{h.outcome}</Badge>
                </div>
              </div>
              {s && (
                <IconButton title="Connect again" className="hidden group-hover:inline-flex" onClick={() => connectSession(s)}>
                  <Play size={13} />
                </IconButton>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
