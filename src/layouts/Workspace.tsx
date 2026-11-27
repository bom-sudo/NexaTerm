import { useRef } from "react";
import { Monitor, Plus, Server, Zap } from "lucide-react";
import { useWorkspace, type Tab } from "../store/workspace";
import { TerminalPane } from "../features/terminal/TerminalPane";
import { SftpBrowser } from "../features/sftp/SftpBrowser";
import { RemoteEditor } from "../features/sftp/RemoteEditor";
import { TunnelsView } from "../features/tunnels/TunnelsView";
import { NetworkToolsView } from "../features/network-tools/NetworkToolsView";
import { KeysView } from "../features/ssh-keys/KeysView";
import { SettingsView } from "../features/settings/SettingsView";
import { KnownHostsView } from "../features/settings/KnownHostsView";
import { Button } from "../components/ui";
import { useSettings } from "../store/settings";
import { newLocalTerminal, newSessionDialog, quickConnectDialog } from "../app/actions";

type Rect = [number, number, number, number]; // x, y, w, h (fractions)

function paneRects(n: number, layout: string, c: number, r: number): Rect[] {
  if (n <= 1) return [[0, 0, 1, 1]];
  if (n === 2 && layout === "rows") return [[0, 0, 1, r], [0, r, 1, 1 - r]];
  if (n === 2) return [[0, 0, c, 1], [c, 0, 1 - c, 1]];
  const rects: Rect[] = [[0, 0, c, r], [c, 0, 1 - c, r], [0, r, n === 3 ? 1 : c, 1 - r]];
  if (n === 4) rects.push([c, r, 1 - c, 1 - r]);
  return rects;
}

function TerminalTabView({ tab, visible }: { tab: Extract<Tab, { kind: "terminal" }>; visible: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  const setRatios = useWorkspace((s) => s.setRatios);
  const n = tab.panes.length;
  const { col, row } = tab.ratios;
  const rects = paneRects(n, tab.layout, col, row);
  const showV = n === 2 ? tab.layout !== "rows" : n >= 3;
  const showH = n === 2 ? tab.layout === "rows" : n >= 3;
  // SFTP side panel uses the active pane's SSH connection (or the first connected SSH pane).
  const sshPanes = tab.panes.filter((p) => p.spec.kind === "ssh" && p.status === "connected" && p.terminalId);
  const dockPane = sshPanes.find((p) => p.id === tab.activePane) ?? sshPanes[0];

  const drag = (axis: "col" | "row") => (e: React.MouseEvent) => {
    e.preventDefault();
    const rect = box.current!.getBoundingClientRect();
    const move = (ev: MouseEvent) => {
      const v = axis === "col" ? (ev.clientX - rect.left) / rect.width : (ev.clientY - rect.top) / rect.height;
      const clamped = Math.min(0.85, Math.max(0.15, v));
      setRatios(tab.id, axis === "col" ? { col: clamped, row } : { col, row: clamped });
    };
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.style.cursor = "";
    };
    document.body.style.cursor = axis === "col" ? "col-resize" : "row-resize";
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  return (
    <div className="flex h-full w-full gap-1">
    <div ref={box} className="relative h-full min-w-0 flex-1">
      {tab.panes.map((p, i) => {
        const [x, y, w, h] = rects[i];
        return (
          <div key={p.id} className="absolute p-[3px]" style={{ left: `${x * 100}%`, top: `${y * 100}%`, width: `${w * 100}%`, height: `${h * 100}%` }}>
            <TerminalPane tabId={tab.id} pane={p} active={tab.activePane === p.id} visible={visible} />
          </div>
        );
      })}
      {showV && (
        <div
          className="absolute z-10 w-[6px] -translate-x-1/2 cursor-col-resize hover:bg-accent/40"
          style={{ left: `${col * 100}%`, top: 0, height: n === 3 ? `${row * 100}%` : "100%" }}
          onMouseDown={drag("col")}
          title="Drag to resize"
        />
      )}
      {showH && <div className="absolute z-10 h-[6px] w-full -translate-y-1/2 cursor-row-resize hover:bg-accent/40" style={{ top: `${row * 100}%`, left: 0 }} onMouseDown={drag("row")} title="Drag to resize" />}
    </div>
    {tab.sftpDock && (
      <div className="h-full w-[min(440px,42%)] min-w-[300px] shrink-0 p-[3px]">
        {dockPane ? (
          <SftpBrowser key={dockPane.terminalId} tabId={tab.id} terminalId={dockPane.terminalId} followPath={dockPane.cwd} compact />
        ) : (
          <div className="flex h-full items-center justify-center rounded-lg border border-line bg-panel p-6 text-center text-[12.5px] text-dim">
            Connect an SSH session in this tab to browse its files here.
          </div>
        )}
      </div>
    )}
    </div>
  );
}

function Welcome() {
  const theme = useSettings((s) => s.settings.theme);
  return (
    <div className="flex h-full items-center justify-center">
      <div className="w-[520px] text-center">
        <img src={theme === "light" ? "/nexaterm-wordmark-light.svg" : "/nexaterm-wordmark-dark.svg"} alt="NexaTerm — Remote Administration Workstation" className="mx-auto mb-2 h-[104px]" />
        <div className="mb-6 text-dim">One Workspace. Every Remote Connection.</div>
        <div className="grid grid-cols-3 gap-3">
          <Button className="flex-col !py-4" onClick={() => newLocalTerminal()}>
            <Monitor size={20} className="text-accent" /> Local terminal
            <span className="text-[11px] text-faint">Ctrl+T</span>
          </Button>
          <Button className="flex-col !py-4" onClick={() => newSessionDialog()}>
            <Plus size={20} className="text-accent" /> New session
            <span className="text-[11px] text-faint">Ctrl+Shift+N</span>
          </Button>
          <Button className="flex-col !py-4" onClick={() => quickConnectDialog()}>
            <Zap size={20} className="text-accent" /> Quick connect
            <span className="text-[11px] text-faint">user@host</span>
          </Button>
        </div>
        <div className="mt-6 flex items-center justify-center gap-2 text-[12px] text-faint">
          <Server size={13} /> Double-click a saved session in the sidebar to connect.
        </div>
      </div>
    </div>
  );
}

export function Workspace() {
  const { tabs, activeTab } = useWorkspace();
  if (tabs.length === 0) return <Welcome />;
  return (
    <div className="relative h-full w-full">
      {tabs.map((t) => {
        const visible = t.id === activeTab;
        return (
          <div key={t.id} className="absolute inset-0 p-1" style={{ visibility: visible ? "visible" : "hidden", zIndex: visible ? 1 : 0 }}>
            {t.kind === "terminal" && <TerminalTabView tab={t} visible={visible} />}
            {t.kind === "sftp" && <SftpBrowser tabId={t.id} terminalId={t.terminalId} sessionId={t.sessionId} />}
            {t.kind === "editor" && <RemoteEditor tabId={t.id} sftpId={t.sftpId} path={t.path} />}
            {t.kind === "tool" && t.tool === "tunnels" && <TunnelsView />}
            {t.kind === "tool" && t.tool === "network" && <NetworkToolsView />}
            {t.kind === "tool" && t.tool === "keys" && <KeysView />}
            {t.kind === "tool" && t.tool === "settings" && <SettingsView />}
            {t.kind === "tool" && t.tool === "knownHosts" && <KnownHostsView />}
          </div>
        );
      })}
    </div>
  );
}
