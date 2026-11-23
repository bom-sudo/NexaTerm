import { useEffect, useMemo, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { AlertTriangle, Copy, Pencil, Play, Plus, Search, SquareTerminal, Trash2 } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { openDialog, toast } from "../../store/ui";
import { useWorkspace } from "../../store/workspace";
import { Button, Field, IconButton, Modal, confirmDialog, useContextMenu } from "../../components/ui";
import { applyVariables, dangerReasons, parseVariables } from "../../utils/commands";
import { sendToActiveTerminal } from "../../app/actions";
import type { Snippet } from "../../types";

const CATEGORIES = ["General", "Linux Administration", "Windows Administration", "Network", "Docker", "Database", "Service Management", "Logs", "Security Diagnostics"];

function SnippetEditor({ snippet, onClose }: { snippet?: Snippet; onClose: (saved: boolean) => void }) {
  const [f, setF] = useState({ name: snippet?.name ?? "", category: snippet?.category ?? "General", command: snippet?.command ?? "", description: snippet?.description ?? "" });
  const [err, setErr] = useState<string | null>(null);
  const save = async () => {
    try {
      await api.snippetSave(snippet?.id ?? null, { ...f, description: f.description || null });
      onClose(true);
    } catch (e) {
      setErr(toApiError(e).message);
    }
  };
  return (
    <Modal
      title={snippet ? "Edit snippet" : "New snippet"}
      onClose={() => onClose(false)}
      width={560}
      footer={
        <>
          {err && <span className="mr-auto self-center text-[12px] text-err">{err}</span>}
          <Button onClick={() => onClose(false)}>Cancel</Button>
          <Button variant="primary" onClick={save}>
            Save
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <Field label="Name">
          <input className="field" autoFocus value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} />
        </Field>
        <Field label="Category">
          <input className="field" list="snippet-cats" value={f.category} onChange={(e) => setF({ ...f, category: e.target.value })} />
          <datalist id="snippet-cats">
            {CATEGORIES.map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>
        </Field>
        <Field label="Command" hint={<>Variables: <code>{"{{name}}"}</code> or <code>{"{{name:default}}"}</code> — you are asked for values when running.</>}>
          <textarea className="field font-mono" rows={4} value={f.command} onChange={(e) => setF({ ...f, command: e.target.value })} />
        </Field>
        <Field label="Description">
          <input className="field" value={f.description} onChange={(e) => setF({ ...f, description: e.target.value })} />
        </Field>
      </div>
    </Modal>
  );
}

function RunDialog({ snippet, onClose }: { snippet: Snippet; onClose: (cmd: string | null) => void }) {
  const vars = parseVariables(snippet.command);
  const [values, setValues] = useState<Record<string, string>>(Object.fromEntries(vars.map((v) => [v.name, v.defaultValue])));
  const [enter, setEnter] = useState(true);
  const cmd = applyVariables(snippet.command, values);
  const danger = dangerReasons(cmd);
  const pane = useWorkspace.getState().activePane();
  const remote = pane?.spec.kind === "ssh";
  return (
    <Modal
      title={`Run “${snippet.name}”`}
      onClose={() => onClose(null)}
      width={560}
      footer={
        <>
          <Button onClick={() => onClose(null)}>Cancel</Button>
          <Button variant={danger.length ? "danger" : "primary"} onClick={() => onClose(cmd + (enter ? "\r" : ""))} disabled={!pane}>
            <Play size={13} /> Run in {pane ? pane.spec.label : "—"}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        {vars.map((v) => (
          <Field key={v.name} label={v.name}>
            <input className="field font-mono" value={values[v.name] ?? ""} onChange={(e) => setValues({ ...values, [v.name]: e.target.value })} />
          </Field>
        ))}
        <div>
          <div className="label">This will be sent to {remote ? <b className="text-warn">remote server {pane?.host ?? pane?.spec.label}</b> : "the active terminal"}:</div>
          <pre className="max-h-40 overflow-auto rounded-lg border border-line bg-elev p-3 font-mono text-[12px] whitespace-pre-wrap select-text">{cmd}</pre>
        </div>
        {danger.length > 0 && (
          <div className="flex gap-2 rounded-lg border border-err/40 bg-err/10 p-3 text-[12.5px]">
            <AlertTriangle size={16} className="shrink-0 text-err" />
            <div>Potentially destructive: {danger.join("; ")}.</div>
          </div>
        )}
        {!pane && <div className="text-[12px] text-err">Open or select a terminal first.</div>}
        <label className="flex items-center gap-2 text-[12.5px]">
          <input type="checkbox" checked={enter} onChange={(e) => setEnter(e.target.checked)} /> Press Enter after inserting
        </label>
      </div>
    </Modal>
  );
}

export async function runSnippet(s: Snippet) {
  const cmd = await openDialog<string | null>((close) => <RunDialog snippet={s} onClose={close} />);
  if (cmd) await sendToActiveTerminal(cmd);
}

export function SnippetsPanel() {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [q, setQ] = useState("");
  const [cat, setCat] = useState("");
  const menu = useContextMenu();
  const load = () => api.snippetsList().then(setSnippets).catch((e) => toast("error", toApiError(e).message));
  useEffect(() => {
    load();
  }, []);

  const cats = useMemo(() => [...new Set(snippets.map((s) => s.category))].sort(), [snippets]);
  const list = snippets.filter((s) => (!cat || s.category === cat) && (!q || `${s.name} ${s.command} ${s.description ?? ""}`.toLowerCase().includes(q.toLowerCase())));
  const grouped = cats.map((c) => [c, list.filter((s) => s.category === c)] as const).filter(([, l]) => l.length);

  const edit = async (s?: Snippet) => {
    if (await openDialog<boolean>((close) => <SnippetEditor snippet={s} onClose={close} />)) load();
  };
  const del = async (s: Snippet) => {
    if (await confirmDialog({ title: "Delete snippet", message: `Delete “${s.name}”?`, confirmText: "Delete", danger: true })) {
      await api.snippetDelete(s.id);
      load();
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="space-y-2 p-2">
        <div className="relative">
          <Search size={13} className="absolute top-2.5 left-2.5 text-faint" />
          <input className="field !pl-8" placeholder="Search commands" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <div className="flex gap-1">
          <Button variant="primary" className="!px-2" onClick={() => edit()}>
            <Plus size={14} /> New
          </Button>
          <select className="field !py-1 !text-[11.5px]" value={cat} onChange={(e) => setCat(e.target.value)}>
            <option value="">All categories</option>
            {cats.map((c) => (
              <option key={c}>{c}</option>
            ))}
          </select>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-1 pb-2">
        {grouped.length === 0 && <div className="p-4 text-center text-[12px] text-dim">No snippets{q ? " match" : ""}.</div>}
        {grouped.map(([c, items]) => (
          <div key={c} className="mb-2">
            <div className="px-2 py-1 text-[11px] font-semibold tracking-wide text-dim uppercase">{c}</div>
            {items.map((s) => (
              <div
                key={s.id}
                className="group rounded-md px-2 py-1.5 hover:bg-hover"
                onDoubleClick={() => runSnippet(s)}
                onContextMenu={(e) =>
                  menu.open(e, [
                    { label: "Run in active terminal…", icon: <SquareTerminal size={13} />, onClick: () => runSnippet(s) },
                    { label: "Copy command", icon: <Copy size={13} />, onClick: () => writeText(s.command).then(() => toast("success", "Copied")) },
                    { label: "Edit…", icon: <Pencil size={13} />, onClick: () => edit(s) },
                    { label: "Delete", icon: <Trash2 size={13} />, danger: true, onClick: () => del(s) },
                  ])
                }
              >
                <div className="flex items-center gap-1">
                  <div className="min-w-0 flex-1 truncate text-[12.5px]">{s.name}</div>
                  <IconButton title="Run in active terminal" className="hidden !h-6 !w-6 group-hover:inline-flex" onClick={() => runSnippet(s)}>
                    <Play size={12} />
                  </IconButton>
                  <IconButton title="Copy" className="hidden !h-6 !w-6 group-hover:inline-flex" onClick={() => writeText(s.command).then(() => toast("success", "Copied"))}>
                    <Copy size={12} />
                  </IconButton>
                </div>
                <div className="truncate font-mono text-[11px] text-faint" title={s.command}>
                  {s.command}
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>
      {menu.node}
    </div>
  );
}
