import { useEffect, useMemo, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Search } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { useSettings, TERMINAL_THEMES, type Settings } from "../../store/settings";
import { useSessions } from "../../store/sessions";
import { useWorkspace } from "../../store/workspace";
import { toast } from "../../store/ui";
import { Button, confirmDialog } from "../../components/ui";
import type { AppInfo } from "../../types";

interface Row {
  section: string;
  label: string;
  hint?: string;
  control: React.ReactNode;
}

export function SettingsView() {
  const { settings, set } = useSettings();
  const shells = useSessions((s) => s.shells);
  const [q, setQ] = useState("");
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => {});
  }, []);

  const sel = <K extends keyof Settings>(k: K, options: [string, string][]) => (
    <select className="field !w-64" value={String(settings[k])} onChange={(e) => set(k, e.target.value as Settings[K])}>
      {options.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
    </select>
  );
  const num = <K extends keyof Settings>(k: K, min: number, max: number, step = 1) => (
    <input className="field !w-32" type="number" min={min} max={max} step={step} value={Number(settings[k])} onChange={(e) => { const v = Number(e.target.value); if (v >= min && v <= max) set(k, v as Settings[K]); }} />
  );
  const chk = <K extends keyof Settings>(k: K) => <input type="checkbox" className="h-4 w-4" checked={Boolean(settings[k])} onChange={(e) => set(k, e.target.checked as Settings[K])} />;

  const rows: Row[] = [
    { section: "Appearance", label: "Application theme", control: sel("theme", [["dark", "Dark (default)"], ["light", "Light"]]) },
    { section: "Appearance", label: "Terminal color theme", control: sel("terminalTheme", Object.entries(TERMINAL_THEMES).map(([k, v]) => [k, v.name])) },
    { section: "Terminal", label: "Font family", hint: "Any installed monospace font, CSS syntax.", control: <input className="field !w-80 font-mono" value={settings.fontFamily} onChange={(e) => set("fontFamily", e.target.value)} /> },
    { section: "Terminal", label: "Font size", hint: "Ctrl+Plus / Ctrl+Minus / Ctrl+0", control: num("fontSize", 8, 32) },
    { section: "Terminal", label: "Line height", control: num("lineHeight", 1, 2, 0.05) },
    { section: "Terminal", label: "Cursor style", control: sel("cursorStyle", [["block", "Block"], ["bar", "Bar"], ["underline", "Underline"]]) },
    { section: "Terminal", label: "Blinking cursor", control: chk("cursorBlink") },
    { section: "Terminal", label: "Scrollback lines", hint: "Applies to newly written output.", control: num("scrollback", 100, 200000, 100) },
    { section: "Terminal", label: "Copy on select", control: chk("copyOnSelect") },
    { section: "Terminal", label: "Right-click copies selection / pastes", control: chk("rightClickPaste") },
    { section: "Terminal", label: "Highlight errors, warnings and IP addresses in output", hint: "Colours words like error, failed, warning, ok and IPv4 addresses. Never applied inside full-screen programs (vim, less, htop) or to text a program already coloured.", control: chk("highlightOutput") },
    { section: "Terminal", label: "Confirm before pasting several lines", hint: "Multi-line text may run several commands at once.", control: chk("confirmMultilinePaste") },
    { section: "Terminal", label: "Send Ctrl+W / Ctrl+T to the shell", hint: "When on, use Ctrl+Shift+W / Ctrl+Shift+T for tabs while a terminal has focus (bash uses Ctrl+W to delete a word).", control: chk("shellKeysPassThrough") },
    { section: "Local shells", label: "Default shell", control: sel("defaultShellId", shells.map((s) => [s.id, s.name])) },
    { section: "Connections", label: "Confirm before closing connected SSH tabs", control: chk("confirmCloseConnected") },
    {
      section: "Data",
      label: "Back up database",
      hint: "Copies sessions, folders, snippets, tunnels and known hosts. Passwords stay in Credential Manager and are not in the backup.",
      control: <Button onClick={async () => { const p = await save({ title: "Back up NexaTerm database", defaultPath: `nexaterm-backup-${new Date().toISOString().slice(0, 10)}.db`, filters: [{ name: "Database", extensions: ["db"] }] }); if (p) api.dbBackup(p).then(() => toast("success", `Backup written to ${p}`)).catch((e) => toast("error", toApiError(e).message)); }}>Back up…</Button>,
    },
    {
      section: "Data",
      label: "Restore database",
      hint: "Replaces current data. A safety copy of the current database is saved first.",
      control: (
        <Button onClick={async () => {
          const p = await open({ title: "Restore NexaTerm backup", filters: [{ name: "Database", extensions: ["db"] }] });
          if (typeof p !== "string") return;
          if (!(await confirmDialog({ title: "Restore backup?", message: "All current sessions, snippets and tunnels will be replaced by the backup's content. Open tabs are not affected.", confirmText: "Restore", danger: true }))) return;
          try {
            await api.dbRestore(p);
            await Promise.all([useSessions.getState().refresh(), useSettings.getState().load()]);
            toast("success", "Backup restored");
          } catch (e) {
            toast("error", toApiError(e).message);
          }
        }}>Restore…</Button>
      ),
    },
    { section: "Data", label: "Known hosts", control: <Button onClick={() => useWorkspace.getState().openTool("knownHosts", "Known Hosts")}>Manage known hosts…</Button> },
    { section: "Data", label: "Show welcome screen again", control: <Button onClick={() => set("onboarded", false)}>Show</Button> },
  ];

  const filtered = useMemo(() => rows.filter((r) => !q || `${r.section} ${r.label} ${r.hint ?? ""}`.toLowerCase().includes(q.toLowerCase())), [rows, q]);
  const sections = [...new Set(filtered.map((r) => r.section))];

  return (
    <div className="flex h-full flex-col rounded-lg border border-line bg-panel">
      <div className="flex items-center gap-3 border-b border-line px-4 py-3">
        <div className="text-[14px] font-semibold">Settings</div>
        <div className="relative ml-auto">
          <Search size={13} className="absolute top-2.5 left-2.5 text-faint" />
          <input autoFocus className="field !w-72 !pl-8" placeholder="Search settings…" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-6 py-4">
        {sections.map((s) => (
          <div key={s} className="mb-6 max-w-3xl">
            <div className="mb-2 text-[11.5px] font-semibold tracking-wide text-accent uppercase">{s}</div>
            <div className="divide-y divide-line rounded-lg border border-line">
              {filtered.filter((r) => r.section === s).map((r) => (
                <div key={r.label} className="flex items-center gap-4 px-4 py-3">
                  <div className="flex-1">
                    <div className="text-[13px]">{r.label}</div>
                    {r.hint && <div className="text-[11.5px] text-faint">{r.hint}</div>}
                  </div>
                  {r.control}
                </div>
              ))}
            </div>
          </div>
        ))}
        {filtered.length === 0 && <div className="text-dim">No settings match “{q}”.</div>}
        {info && (
          <div className="mt-2 max-w-3xl rounded-lg border border-line p-4 text-[12px] text-dim select-text">
            <div className="mb-1 font-semibold text-fg">About NexaTerm {info.version}</div>
            Free and open-source (MIT). No telemetry, no accounts, no paid tiers.
            <div className="mt-2 font-mono text-[11px]">Data: {info.dataDir}{info.portable ? " (portable mode)" : ""} · schema v{info.schemaVersion} · {info.os}</div>
          </div>
        )}
      </div>
    </div>
  );
}
