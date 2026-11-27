import { create } from "zustand";
import { uid } from "../utils/commands";

export type PaneStatus = "connecting" | "connected" | "exited" | "lost" | "error";

/** What a terminal pane should launch. */
export type PaneSpec =
  | { kind: "local"; program: string; args: string[]; cwd: string | null; label: string; sessionId: string | null }
  | { kind: "ssh"; sessionId: string; label: string }
  | { kind: "session"; sessionId: string; label: string; protocol: "telnet" | "serial" };

export interface Pane {
  id: string;
  spec: PaneSpec;
  /** Changes force the pane to relaunch (reconnect). */
  generation: number;
  terminalId?: string;
  status: PaneStatus;
  error?: string;
  host?: string;
  username?: string;
  connectMs?: number;
  logging?: string | null;
  /** Remote working directory reported by the shell (OSC 7). */
  cwd?: string;
}

export type Layout = "single" | "columns" | "rows" | "grid";
export type ToolKind = "tunnels" | "network" | "keys" | "settings" | "knownHosts";

export type Tab =
  | { id: string; kind: "terminal"; title: string; panes: Pane[]; layout: Layout; activePane: string; ratios: { col: number; row: number }; sftpDock?: boolean }
  | { id: string; kind: "sftp"; title: string; terminalId?: string; sessionId?: string }
  | { id: string; kind: "editor"; title: string; sftpId: string; path: string }
  | { id: string; kind: "tool"; title: string; tool: ToolKind };

interface WorkspaceStore {
  tabs: Tab[];
  activeTab: string | null;
  multiExec: { enabled: boolean; selected: string[]; mirrorTyping: boolean };
  terminalSearch: number;

  openTerminal: (spec: PaneSpec) => void;
  splitActive: (spec: PaneSpec) => boolean;
  setLayout: (tabId: string, layout: Layout) => void;
  setRatios: (tabId: string, ratios: { col: number; row: number }) => void;
  closePane: (tabId: string, paneId: string) => void;
  updatePane: (paneId: string, patch: Partial<Pane>) => void;
  reconnectPane: (paneId: string) => void;
  setActivePane: (tabId: string, paneId: string) => void;
  openTool: (tool: ToolKind, title: string) => void;
  openSftp: (opts: { terminalId?: string; sessionId?: string; title: string }) => void;
  openEditor: (sftpId: string, path: string) => void;
  closeTab: (id: string) => void;
  activate: (id: string) => void;
  cycle: (dir: 1 | -1) => void;
  renameTab: (id: string, title: string) => void;
  setMultiExec: (patch: Partial<WorkspaceStore["multiExec"]>) => void;
  toggleSftpDock: (tabId: string) => void;
  requestTerminalSearch: () => void;
  allPanes: () => { tab: Tab; pane: Pane }[];
  activePane: () => Pane | undefined;
}

const MAX_PANES = 4;

function newPane(spec: PaneSpec): Pane {
  return { id: uid(), spec, generation: 0, status: "connecting" };
}

function layoutFor(n: number, current: Layout): Layout {
  if (n <= 1) return "single";
  if (n === 2) return current === "rows" ? "rows" : "columns";
  return "grid";
}

export const useWorkspace = create<WorkspaceStore>((set, get) => ({
  tabs: [],
  activeTab: null,
  multiExec: { enabled: false, selected: [], mirrorTyping: false },
  terminalSearch: 0,

  openTerminal: (spec) => {
    const pane = newPane(spec);
    const tab: Tab = { id: uid(), kind: "terminal", title: spec.label, panes: [pane], layout: "single", activePane: pane.id, ratios: { col: 0.5, row: 0.5 } };
    set({ tabs: [...get().tabs, tab], activeTab: tab.id });
  },

  splitActive: (spec) => {
    const t = get().tabs.find((x) => x.id === get().activeTab);
    if (!t || t.kind !== "terminal") {
      get().openTerminal(spec);
      return true;
    }
    if (t.panes.length >= MAX_PANES) return false;
    const pane = newPane(spec);
    const panes = [...t.panes, pane];
    set({ tabs: get().tabs.map((x) => (x.id === t.id ? { ...t, panes, layout: layoutFor(panes.length, t.layout), activePane: pane.id } : x)) });
    return true;
  },

  setLayout: (tabId, layout) => set({ tabs: get().tabs.map((t) => (t.id === tabId && t.kind === "terminal" ? { ...t, layout } : t)) }),
  setRatios: (tabId, ratios) => set({ tabs: get().tabs.map((t) => (t.id === tabId && t.kind === "terminal" ? { ...t, ratios } : t)) }),

  closePane: (tabId, paneId) => {
    const t = get().tabs.find((x) => x.id === tabId);
    if (!t || t.kind !== "terminal") return;
    const panes = t.panes.filter((p) => p.id !== paneId);
    if (panes.length === 0) return get().closeTab(tabId);
    const activePane = t.activePane === paneId ? panes[panes.length - 1].id : t.activePane;
    set({
      tabs: get().tabs.map((x) => (x.id === tabId ? { ...t, panes, activePane, layout: layoutFor(panes.length, t.layout) } : x)),
      multiExec: { ...get().multiExec, selected: get().multiExec.selected.filter((s) => s !== paneId) },
    });
  },

  updatePane: (paneId, patch) =>
    set({
      tabs: get().tabs.map((t) =>
        t.kind === "terminal" && t.panes.some((p) => p.id === paneId) ? { ...t, panes: t.panes.map((p) => (p.id === paneId ? { ...p, ...patch } : p)) } : t,
      ),
    }),

  reconnectPane: (paneId) => {
    const p = get().allPanes().find((x) => x.pane.id === paneId)?.pane;
    if (p) get().updatePane(paneId, { generation: p.generation + 1, status: "connecting", error: undefined, terminalId: undefined });
  },

  setActivePane: (tabId, paneId) => set({ tabs: get().tabs.map((t) => (t.id === tabId && t.kind === "terminal" ? { ...t, activePane: paneId } : t)) }),

  openTool: (tool, title) => {
    const existing = get().tabs.find((t) => t.kind === "tool" && t.tool === tool);
    if (existing) return set({ activeTab: existing.id });
    const tab: Tab = { id: uid(), kind: "tool", tool, title };
    set({ tabs: [...get().tabs, tab], activeTab: tab.id });
  },

  openSftp: ({ terminalId, sessionId, title }) => {
    const tab: Tab = { id: uid(), kind: "sftp", title, terminalId, sessionId };
    set({ tabs: [...get().tabs, tab], activeTab: tab.id });
  },

  openEditor: (sftpId, path) => {
    const existing = get().tabs.find((t) => t.kind === "editor" && t.sftpId === sftpId && t.path === path);
    if (existing) return set({ activeTab: existing.id });
    const tab: Tab = { id: uid(), kind: "editor", sftpId, path, title: path.split("/").pop() || path };
    set({ tabs: [...get().tabs, tab], activeTab: tab.id });
  },

  closeTab: (id) => {
    const tabs = get().tabs;
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx < 0) return;
    const closing = tabs[idx];
    const next = tabs.filter((t) => t.id !== id);
    const paneIds = closing.kind === "terminal" ? closing.panes.map((p) => p.id) : [];
    set({
      tabs: next,
      activeTab: get().activeTab === id ? (next[Math.min(idx, next.length - 1)]?.id ?? null) : get().activeTab,
      multiExec: { ...get().multiExec, selected: get().multiExec.selected.filter((s) => !paneIds.includes(s)) },
    });
  },

  activate: (id) => set({ activeTab: id }),
  cycle: (dir) => {
    const { tabs, activeTab } = get();
    if (tabs.length < 2) return;
    const i = tabs.findIndex((t) => t.id === activeTab);
    set({ activeTab: tabs[(i + dir + tabs.length) % tabs.length].id });
  },
  renameTab: (id, title) => set({ tabs: get().tabs.map((t) => (t.id === id ? { ...t, title } : t)) }),
  setMultiExec: (patch) => set({ multiExec: { ...get().multiExec, ...patch } }),
  toggleSftpDock: (tabId) => set({ tabs: get().tabs.map((t) => (t.id === tabId && t.kind === "terminal" ? { ...t, sftpDock: !t.sftpDock } : t)) }),
  requestTerminalSearch: () => set({ terminalSearch: get().terminalSearch + 1 }),

  allPanes: () => get().tabs.flatMap((tab) => (tab.kind === "terminal" ? tab.panes.map((pane) => ({ tab, pane })) : [])),
  activePane: () => {
    const t = get().tabs.find((x) => x.id === get().activeTab);
    return t?.kind === "terminal" ? t.panes.find((p) => p.id === t.activePane) : undefined;
  },
}));
