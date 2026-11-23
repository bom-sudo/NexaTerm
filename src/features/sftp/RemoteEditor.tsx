import { useEffect, useState } from "react";
import { Loader2, Save, RotateCw } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { toast } from "../../store/ui";
import { useWorkspace } from "../../store/workspace";
import { Button, EmptyState, confirmDialog } from "../../components/ui";

/**
 * Basic remote text editor. Saving uploads to a temporary file next to the original, copies the
 * original permissions onto it, and atomically renames it into place (see sftp::write_text_atomic).
 */
export function RemoteEditor({ tabId, sftpId, path }: { tabId: string; sftpId: string; path: string }) {
  const [original, setOriginal] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [mode, setMode] = useState<number | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const dirty = original !== null && text !== original;
  const renameTab = useWorkspace((s) => s.renameTab);
  const name = path.split("/").pop() || path;

  const load = async () => {
    setErr(null);
    try {
      const f = await api.sftpReadText(sftpId, path);
      setOriginal(f.content);
      setText(f.content);
      setMode(f.mode);
    } catch (e) {
      setErr(toApiError(e).message);
    }
  };
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sftpId, path]);
  useEffect(() => renameTab(tabId, dirty ? `● ${name}` : name), [dirty, name, tabId, renameTab]);

  const save = async () => {
    setSaving(true);
    try {
      await api.sftpWriteText(sftpId, path, text);
      setOriginal(text);
      toast("success", `Saved ${path}`);
    } catch (e) {
      toast("error", `Save failed — the original file was not modified. ${toApiError(e).message}`);
    } finally {
      setSaving(false);
    }
  };

  if (err) return <EmptyState icon={<RotateCw size={28} />} title="Cannot open file"><div className="mb-3 select-text">{err}</div><Button onClick={load}>Retry</Button></EmptyState>;
  if (original === null) return <EmptyState icon={<Loader2 size={28} className="animate-spin" />} title={`Loading ${name}…`} />;

  return (
    <div className="flex h-full flex-col rounded-lg border border-line bg-panel">
      <div className="flex items-center gap-2 border-b border-line px-3 py-2">
        <div className="min-w-0 flex-1 truncate font-mono text-[12px] text-dim" title={path}>
          {path}
          {mode != null && <span className="ml-2 text-faint">mode {(mode & 0o7777).toString(8)}</span>}
        </div>
        <Button
          variant="ghost"
          onClick={async () => {
            if (!dirty || (await confirmDialog({ title: "Discard changes?", message: "Reload the file from the server and lose your edits?", confirmText: "Reload", danger: true }))) load();
          }}
        >
          <RotateCw size={13} /> Reload
        </Button>
        <Button variant="primary" disabled={!dirty || saving} onClick={save}>
          <Save size={13} /> {saving ? "Saving…" : "Save (Ctrl+S)"}
        </Button>
      </div>
      <textarea
        className="min-h-0 flex-1 resize-none bg-transparent p-3 font-mono text-[13px] leading-relaxed text-fg outline-none"
        spellCheck={false}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.ctrlKey && e.key.toLowerCase() === "s") {
            e.preventDefault();
            if (dirty) save();
          }
          if (e.key === "Tab") {
            e.preventDefault();
            const t = e.currentTarget;
            const { selectionStart: a, selectionEnd: b } = t;
            setText(text.slice(0, a) + "\t" + text.slice(b));
            requestAnimationFrame(() => (t.selectionStart = t.selectionEnd = a + 1));
          }
        }}
      />
    </div>
  );
}
