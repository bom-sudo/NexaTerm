import { useState } from "react";
import { AlertTriangle, CheckSquare, Radio, Send, Square, X } from "lucide-react";
import { useWorkspace } from "../../store/workspace";
import { api } from "../../services/api";
import { toast } from "../../store/ui";
import { Button, confirmDialog } from "../../components/ui";
import { dangerReasons } from "../../utils/commands";

/** Multi-execution: choose target terminals and broadcast a command or mirrored keystrokes. */
export function MultiExecBar() {
  const ws = useWorkspace();
  const { selected, mirrorTyping } = ws.multiExec;
  const [cmd, setCmd] = useState("");
  const [results, setResults] = useState<Record<string, string | null>>({});
  const candidates = ws.allPanes().filter((x) => x.pane.status === "connected" && x.pane.terminalId);

  const toggle = (id: string) => ws.setMultiExec({ selected: selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id] });

  const send = async () => {
    const targets = candidates.filter((c) => selected.includes(c.pane.id));
    if (!cmd.trim() || targets.length === 0) return toast("info", "Type a command and select at least one terminal.");
    const danger = dangerReasons(cmd);
    const remote = targets.filter((t) => t.pane.spec.kind === "ssh").length;
    const ok = await confirmDialog({
      title: danger.length ? "⚠ Potentially destructive command" : `Send to ${targets.length} terminal(s)?`,
      danger: danger.length > 0,
      confirmText: `Execute on ${targets.length}`,
      message: (
        <div className="space-y-2">
          {danger.length > 0 && (
            <div className="flex gap-2 rounded-lg border border-err/40 bg-err/10 p-2">
              <AlertTriangle size={16} className="shrink-0 text-err" /> {danger.join("; ")}
            </div>
          )}
          <pre className="rounded-lg border border-line bg-elev p-2 font-mono text-[12px] whitespace-pre-wrap select-text">{cmd}</pre>
          <div>
            Targets ({remote} remote):
            <ul className="mt-1 list-disc pl-5 text-dim">
              {targets.map((t) => (
                <li key={t.pane.id}>
                  {t.pane.spec.label}
                  {t.pane.host ? ` — ${t.pane.username}@${t.pane.host}` : ""}
                </li>
              ))}
            </ul>
          </div>
        </div>
      ),
    });
    if (!ok) return;
    const res = await api.broadcast(targets.map((t) => t.pane.terminalId!), cmd + "\r");
    const byTerm: Record<string, string | null> = {};
    for (const [tid, err] of res) {
      const pane = targets.find((t) => t.pane.terminalId === tid);
      if (pane) byTerm[pane.pane.id] = err;
    }
    setResults(byTerm);
    const failed = res.filter(([, e]) => e).length;
    toast(failed ? "warn" : "success", failed ? `Sent to ${res.length - failed}, failed on ${failed}` : `Sent to ${res.length} terminal(s)`);
    setCmd("");
  };

  return (
    <div className="shrink-0 border-b-2 border-warn bg-warn/10 px-3 py-2">
      <div className="flex items-center gap-2">
        <Radio size={15} className="text-warn" />
        <span className="text-[12px] font-semibold text-warn">MULTI-EXECUTION</span>
        <div className="flex min-w-0 flex-1 flex-wrap items-center gap-1">
          {candidates.length === 0 && <span className="text-[12px] text-dim">No connected terminals.</span>}
          {candidates.map(({ pane }) => {
            const on = selected.includes(pane.id);
            const r = results[pane.id];
            return (
              <button
                key={pane.id}
                onClick={() => toggle(pane.id)}
                className={`flex items-center gap-1 rounded-md border px-2 py-0.5 text-[11.5px] ${on ? "border-warn bg-warn/20 text-fg" : "border-line text-dim hover:bg-hover"}`}
                title={r ? `Last send failed: ${r}` : undefined}
              >
                {on ? <CheckSquare size={12} /> : <Square size={12} />}
                {pane.spec.label}
                {r !== undefined && <span className={r ? "text-err" : "text-ok"}>{r ? "✗" : "✓"}</span>}
              </button>
            );
          })}
        </div>
        <Button variant="ghost" className="!px-2 !py-1" onClick={() => ws.setMultiExec({ selected: candidates.map((c) => c.pane.id) })}>
          Select all
        </Button>
        <Button variant="ghost" className="!px-2 !py-1" onClick={() => ws.setMultiExec({ selected: [] })}>
          None
        </Button>
        <button className="text-dim hover:text-fg" title="Stop multi-execution" onClick={() => ws.setMultiExec({ enabled: false, mirrorTyping: false })}>
          <X size={16} />
        </button>
      </div>
      <form
        className="mt-2 flex items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          send();
        }}
      >
        <input className="field font-mono" placeholder="Command to broadcast to the selected terminals (confirmation required)" value={cmd} onChange={(e) => setCmd(e.target.value)} />
        <Button variant="primary" type="submit" disabled={!selected.length}>
          <Send size={13} /> Broadcast
        </Button>
        <label className="flex shrink-0 items-center gap-1.5 text-[12px] text-dim" title="Every keystroke typed in a selected terminal is also sent to the other selected terminals">
          <input type="checkbox" checked={mirrorTyping} onChange={(e) => ws.setMultiExec({ mirrorTyping: e.target.checked })} /> Mirror typing
        </label>
      </form>
    </div>
  );
}
