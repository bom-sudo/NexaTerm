// Global keyboard shortcuts. Terminal panes call handleGlobalShortcut from xterm's key handler;
// everything else is handled by the window listener installed in installShortcuts().
import { useWorkspace } from "../store/workspace";
import { useUi } from "../store/ui";
import { useSettings } from "../store/settings";

type Handler = () => void;
const registry: { newSession?: Handler; newTerminal?: Handler } = {};

export function registerShortcutHandlers(h: typeof registry) {
  Object.assign(registry, h);
}

export function handleGlobalShortcut(e: KeyboardEvent): boolean {
  const ws = useWorkspace.getState();
  const ctrl = e.ctrlKey && !e.altKey;
  const k = e.key.toLowerCase();
  const inTerminal = (e.target as HTMLElement | null)?.closest?.(".xterm") != null;
  const passThrough = useSettings.getState().settings.shellKeysPassThrough && inTerminal;

  const act = (fn: () => void) => {
    e.preventDefault();
    fn();
    return true;
  };

  if (ctrl && e.shiftKey && k === "n") return act(() => registry.newSession?.());
  if (ctrl && k === "t" && (e.shiftKey || !passThrough)) return act(() => registry.newTerminal?.());
  if (ctrl && k === "w" && (e.shiftKey || !passThrough)) return act(() => ws.activeTab && ws.closeTab(ws.activeTab));
  if (ctrl && e.key === "Tab") return act(() => ws.cycle(e.shiftKey ? -1 : 1));
  if (ctrl && e.shiftKey && k === "f") return act(() => useUi.getState().focusSessionSearch());
  if (ctrl && !e.shiftKey && k === "f" && !inTerminal) return act(() => ws.requestTerminalSearch());
  if (ctrl && (e.key === "+" || e.key === "=")) return act(() => fontSize(1));
  if (ctrl && e.key === "-") return act(() => fontSize(-1));
  if (ctrl && e.key === "0") return act(() => fontSize(0));
  return false;
}

function fontSize(delta: number) {
  const s = useSettings.getState();
  s.set("fontSize", delta === 0 ? 14 : Math.max(8, Math.min(32, s.settings.fontSize + delta)));
}

export function installShortcuts(): () => void {
  const onKey = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement | null;
    if (target?.closest?.(".xterm")) return; // xterm's own handler already ran
    // Block browser-only accelerators that make no sense in a desktop app (reload, print, find bar).
    if (e.key === "F5" || (e.ctrlKey && ["r", "p", "u", "g", "j"].includes(e.key.toLowerCase()) && !e.shiftKey)) {
      e.preventDefault();
      return;
    }
    handleGlobalShortcut(e);
  };
  window.addEventListener("keydown", onKey, true);
  return () => window.removeEventListener("keydown", onKey, true);
}
