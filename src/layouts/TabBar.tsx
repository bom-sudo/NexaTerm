import { useState } from "react";
import { ChevronDown, FileText, FolderSync, Monitor, Network, Server, Settings, ShieldCheck, KeyRound, Waypoints, X } from "lucide-react";
import { useWorkspace, type Tab } from "../store/workspace";
import { promptDialog, useContextMenu, confirmDialog } from "../components/ui";
import { useSettings } from "../store/settings";

export function tabIcon(t: Tab) {
  const cls = "shrink-0";
  if (t.kind === "terminal") {
    const ssh = t.panes.some((p) => p.spec.kind === "ssh");
    return ssh ? <Server size={13} className={`${cls} text-accent`} /> : <Monitor size={13} className={`${cls} text-accent-2`} />;
  }
  if (t.kind === "sftp") return <FolderSync size={13} className={`${cls} text-accent`} />;
  if (t.kind === "editor") return <FileText size={13} className={cls} />;
  const m = { tunnels: Waypoints, network: Network, keys: KeyRound, settings: Settings, knownHosts: ShieldCheck };
  const I = m[t.tool];
  return <I size={13} className={cls} />;
}

function statusDot(t: Tab) {
  if (t.kind !== "terminal") return null;
  const states = t.panes.map((p) => p.status);
  const color = states.includes("error") || states.includes("lost") ? "bg-err" : states.includes("connecting") ? "bg-warn" : states.every((s) => s === "exited") ? "bg-faint" : "bg-ok";
  return <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${color}`} />;
}

export async function closeTabWithConfirm(t: Tab) {
  const ws = useWorkspace.getState();
  const live = t.kind === "terminal" && t.panes.some((p) => p.spec.kind === "ssh" && p.status === "connected");
  if (live && useSettings.getState().settings.confirmCloseConnected) {
    const ok = await confirmDialog({ title: "Close connected session?", message: `“${t.title}” has an active SSH connection. Closing the tab disconnects it.`, confirmText: "Disconnect & close" });
    if (!ok) return;
  }
  ws.closeTab(t.id);
}

export function TabBar() {
  const { tabs, activeTab, activate, renameTab, closeTab } = useWorkspace();
  const [listOpen, setListOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const menu = useContextMenu();

  const rename = async (t: Tab) => {
    const r = await promptDialog({ title: "Rename tab", label: "Title", initial: t.title });
    if (r?.value) renameTab(t.id, r.value);
  };

  return (
    <div className="flex h-9 shrink-0 items-end gap-0.5 border-b border-line bg-elev px-1">
      <div className="flex min-w-0 flex-1 items-end gap-0.5 overflow-x-auto">
        {tabs.map((t) => (
          <div
            key={t.id}
            onMouseDown={(e) => {
              if (e.button === 1) {
                e.preventDefault();
                closeTabWithConfirm(t);
              } else activate(t.id);
            }}
            onDoubleClick={() => rename(t)}
            onContextMenu={(e) =>
              menu.open(e, [
                { label: "Rename…", onClick: () => rename(t) },
                { label: "Close", onClick: () => closeTabWithConfirm(t) },
                { label: "Close other tabs", onClick: () => tabs.filter((x) => x.id !== t.id).forEach((x) => closeTab(x.id)) },
              ])
            }
            className={`group flex h-8 max-w-[220px] min-w-[110px] cursor-default items-center gap-1.5 rounded-t-lg border border-b-0 px-2.5 text-[12.5px] ${
              t.id === activeTab ? "border-line bg-bg text-fg" : "border-transparent text-dim hover:bg-hover"
            }`}
            title={t.title}
          >
            {tabIcon(t)}
            <span className="min-w-0 flex-1 truncate">{t.title}</span>
            {statusDot(t)}
            <button
              className="rounded p-0.5 text-faint opacity-0 group-hover:opacity-100 hover:bg-hover hover:text-fg"
              onMouseDown={(e) => e.stopPropagation()}
              onClick={() => closeTabWithConfirm(t)}
              title="Close tab (Ctrl+W)"
            >
              <X size={12} />
            </button>
          </div>
        ))}
      </div>
      {tabs.length > 0 && (
        <div className="relative mb-1">
          <button className="flex h-7 items-center gap-1 rounded-md px-2 text-[12px] text-dim hover:bg-hover" title="Search open tabs" onClick={() => setListOpen((v) => !v)}>
            {tabs.length} <ChevronDown size={13} />
          </button>
          {listOpen && (
            <div className="absolute top-8 right-0 z-40 w-72 rounded-lg border border-line-strong bg-panel p-1 shadow-2xl" onMouseLeave={() => setListOpen(false)}>
              <input autoFocus className="field mb-1" placeholder="Filter tabs…" value={filter} onChange={(e) => setFilter(e.target.value)} onKeyDown={(e) => e.key === "Escape" && setListOpen(false)} />
              <div className="max-h-80 overflow-auto">
                {tabs
                  .filter((t) => t.title.toLowerCase().includes(filter.toLowerCase()))
                  .map((t) => (
                    <button
                      key={t.id}
                      className={`flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-[12.5px] hover:bg-hover ${t.id === activeTab ? "text-accent" : ""}`}
                      onClick={() => {
                        activate(t.id);
                        setListOpen(false);
                        setFilter("");
                      }}
                    >
                      {tabIcon(t)} <span className="truncate">{t.title}</span>
                    </button>
                  ))}
              </div>
            </div>
          )}
        </div>
      )}
      {menu.node}
    </div>
  );
}
