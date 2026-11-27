import { Clapperboard, History, KeyRound, Network, Server, Settings, ShieldCheck, SquareCode, Waypoints, Zap } from "lucide-react";
import { useUi } from "../store/ui";
import { useWorkspace } from "../store/workspace";
import { SessionsPanel } from "../features/sessions/SessionsPanel";
import { HistoryPanel } from "../features/sessions/HistoryPanel";
import { SnippetsPanel } from "../features/snippets/SnippetsPanel";
import { MacrosPanel } from "../features/macros/MacrosPanel";
import { quickConnectDialog } from "../app/actions";

function RailButton({ title, active, onClick, children }: { title: string; active?: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      title={title}
      aria-label={title}
      onClick={onClick}
      className={`relative flex h-10 w-10 items-center justify-center rounded-lg transition-colors ${active ? "bg-accent-soft text-accent" : "text-dim hover:bg-hover hover:text-fg"}`}
    >
      {active && <span className="absolute top-2 bottom-2 -left-[7px] w-[3px] rounded-r bg-accent" />}
      {children}
    </button>
  );
}

export function Sidebar() {
  const { sidebarView, setSidebarView } = useUi();
  const openTool = useWorkspace((s) => s.openTool);
  const titles = { sessions: "Sessions", snippets: "Command Snippets", history: "Recent Connections", macros: "Macros" };
  return (
    <div className="flex h-full shrink-0">
      <div className="flex w-[52px] flex-col items-center gap-1 border-r border-line bg-elev py-2">
        <img src="/nexaterm-icon.svg" alt="NexaTerm" className="mb-2 h-9 w-9" title="NexaTerm" />
        <RailButton title="Quick connect" onClick={() => quickConnectDialog()}>
          <Zap size={18} />
        </RailButton>
        <RailButton title="Sessions, favorites and folders" active={sidebarView === "sessions"} onClick={() => setSidebarView("sessions")}>
          <Server size={18} />
        </RailButton>
        <RailButton title="Recent connections" active={sidebarView === "history"} onClick={() => setSidebarView("history")}>
          <History size={18} />
        </RailButton>
        <RailButton title="Command snippets" active={sidebarView === "snippets"} onClick={() => setSidebarView("snippets")}>
          <SquareCode size={18} />
        </RailButton>
        <RailButton title="Macros (record and replay keystrokes)" active={sidebarView === "macros"} onClick={() => setSidebarView("macros")}>
          <Clapperboard size={18} />
        </RailButton>
        <div className="my-1 h-px w-6 bg-line" />
        <RailButton title="SSH tunnels" onClick={() => openTool("tunnels", "SSH Tunnels")}>
          <Waypoints size={18} />
        </RailButton>
        <RailButton title="SSH keys" onClick={() => openTool("keys", "SSH Keys")}>
          <KeyRound size={18} />
        </RailButton>
        <RailButton title="Network tools" onClick={() => openTool("network", "Network Tools")}>
          <Network size={18} />
        </RailButton>
        <RailButton title="Known hosts" onClick={() => openTool("knownHosts", "Known Hosts")}>
          <ShieldCheck size={18} />
        </RailButton>
        <div className="flex-1" />
        <RailButton title="Settings" onClick={() => openTool("settings", "Settings")}>
          <Settings size={18} />
        </RailButton>
      </div>
      <div className="flex w-[264px] flex-col border-r border-line bg-panel">
        <div className="flex h-10 shrink-0 items-center border-b border-line px-3 text-[12px] font-semibold tracking-wide text-dim uppercase">{titles[sidebarView]}</div>
        <div className="min-h-0 flex-1">
          {sidebarView === "sessions" && <SessionsPanel />}
          {sidebarView === "history" && <HistoryPanel />}
          {sidebarView === "snippets" && <SnippetsPanel />}
          {sidebarView === "macros" && <MacrosPanel />}
        </div>
      </div>
    </div>
  );
}
