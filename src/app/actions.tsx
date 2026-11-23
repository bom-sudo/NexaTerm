// User-level actions shared by toolbar, menus, sidebar and shortcuts.
import { save } from "@tauri-apps/plugin-dialog";
import { api } from "../services/api";
import { useSessions } from "../store/sessions";
import { useSettings } from "../store/settings";
import { toast, openDialog } from "../store/ui";
import { useWorkspace, type PaneSpec } from "../store/workspace";
import { terminals, bufferText } from "../features/terminal/registry";
import type { Session, ShellProfile } from "../types";
import { SessionDialog } from "../features/sessions/SessionDialog";
import { QuickConnectDialog } from "../features/sessions/QuickConnect";

export function shellSpec(shell: ShellProfile, cwd: string | null = null): PaneSpec {
  return { kind: "local", program: shell.path, args: shell.args, cwd, label: shell.name, sessionId: null };
}

export function defaultShell(): ShellProfile | undefined {
  const { shells } = useSessions.getState();
  const id = useSettings.getState().settings.defaultShellId;
  return shells.find((s) => s.id === id) ?? shells[0];
}

export function newLocalTerminal(opts: { split?: boolean; shell?: ShellProfile } = {}) {
  const shell = opts.shell ?? defaultShell();
  if (!shell) return toast("error", "No local shell was detected on this system.");
  const spec = shellSpec(shell);
  const ws = useWorkspace.getState();
  if (opts.split) {
    if (!ws.splitActive(spec)) toast("warn", "A tab can hold at most 4 panes.");
  } else ws.openTerminal(spec);
}

export function sessionSpec(s: Session): PaneSpec | null {
  if (s.protocol === "ssh") return { kind: "ssh", sessionId: s.id, label: s.name };
  if (s.protocol === "local") {
    const fallback = defaultShell();
    const program = s.options.shellPath || fallback?.path;
    if (!program) return null;
    return { kind: "local", program, args: s.options.shellArgs ?? (s.options.shellPath ? [] : (fallback?.args ?? [])), cwd: s.options.cwd ?? null, label: s.name, sessionId: s.id };
  }
  return null;
}

export function connectSession(s: Session, opts: { split?: boolean } = {}) {
  const spec = sessionSpec(s);
  if (!spec) return toast("warn", `${s.protocol.toUpperCase()} sessions are not implemented yet in this version.`);
  const ws = useWorkspace.getState();
  if (opts.split) {
    if (!ws.splitActive(spec)) toast("warn", "A tab can hold at most 4 panes.");
  } else ws.openTerminal(spec);
  setTimeout(() => useSessions.getState().refresh().catch(() => {}), 1500);
}

export async function newSessionDialog(initial?: Partial<Session>) {
  const created = await openDialog<Session | null>((close) => <SessionDialog initial={initial} onClose={close} />);
  if (created) await useSessions.getState().refresh();
  return created;
}

export async function editSessionDialog(s: Session) {
  const updated = await openDialog<Session | null>((close) => <SessionDialog session={s} onClose={close} />);
  if (updated) await useSessions.getState().refresh();
}

export function quickConnectDialog() {
  return openDialog<null>((close) => <QuickConnectDialog onClose={() => close(null)} />);
}

export function openSftpForActivePane() {
  const ws = useWorkspace.getState();
  const p = ws.activePane();
  if (p?.spec.kind === "ssh" && p.terminalId && p.status === "connected") {
    ws.openSftp({ terminalId: p.terminalId, sessionId: p.spec.sessionId, title: `SFTP · ${p.spec.label}` });
  } else toast("info", "Select a connected SSH terminal first, or right-click an SSH session → Open SFTP.");
}

function activeHandle() {
  const p = useWorkspace.getState().activePane();
  return p ? terminals.get(p.id) : undefined;
}

export const terminalActions = {
  clear: () => activeHandle()?.term.clear(),
  reset: () => activeHandle()?.term.reset(),
  selectAll: () => activeHandle()?.term.selectAll(),
  scrollBottom: () => activeHandle()?.term.scrollToBottom(),
  async exportOutput() {
    const h = activeHandle();
    if (!h) return toast("info", "No active terminal.");
    const path = await save({ title: "Export terminal output", defaultPath: "terminal-output.txt", filters: [{ name: "Text", extensions: ["txt", "log"] }] });
    if (!path) return;
    await api.writeTextFile(path, bufferText(h.term));
    toast("success", `Saved ${path}`);
  },
  async toggleLogging() {
    const ws = useWorkspace.getState();
    const p = ws.activePane();
    if (!p?.terminalId) return toast("info", "No active terminal.");
    if (p.logging) {
      const path = await api.logStop(p.terminalId);
      ws.updatePane(p.id, { logging: null });
      toast("success", `Logging stopped${path ? `: ${path}` : ""}`);
    } else {
      const path = await api.logStart(p.terminalId);
      ws.updatePane(p.id, { logging: path });
      toast("success", `Logging to ${path}`);
    }
  },
  fontSize(delta: number) {
    const s = useSettings.getState();
    const next = delta === 0 ? 14 : Math.max(8, Math.min(32, s.settings.fontSize + delta));
    s.set("fontSize", next);
  },
};

export async function sendToActiveTerminal(text: string) {
  const p = useWorkspace.getState().activePane();
  if (!p?.terminalId || p.status !== "connected") {
    toast("info", "Open or select a connected terminal first.");
    return false;
  }
  await api.write(p.terminalId, text);
  terminals.get(p.id)?.term.focus();
  return true;
}
