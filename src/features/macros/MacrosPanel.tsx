import { useEffect, useState } from "react";
import { AlertTriangle, CircleDot, Play, Square, Trash2 } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { openDialog, toast } from "../../store/ui";
import { useWorkspace } from "../../store/workspace";
import { useMacroRecorder, describeSteps } from "../../store/macros";
import { Button, IconButton, Modal, confirmDialog, promptDialog } from "../../components/ui";
import { dangerReasons } from "../../utils/commands";
import { terminals } from "../terminal/registry";
import type { MacroProfile } from "../../types";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Start recording the active terminal, or stop and save the recording. */
export async function toggleMacroRecording(onSaved?: () => void) {
  const rec = useMacroRecorder.getState();
  if (rec.paneId) {
    const steps = rec.stop();
    if (!steps.length) return toast("info", "Nothing was typed; the recording was discarded.");
    const r = await promptDialog({ title: "Save macro", label: "Macro name" });
    if (!r?.value) return toast("info", "Recording discarded.");
    try {
      await api.macroSave(null, r.value, steps);
      toast("success", `Macro "${r.value}" saved (${steps.length} steps)`);
      onSaved?.();
    } catch (e) {
      toast("error", toApiError(e).message);
    }
    return;
  }
  const pane = useWorkspace.getState().activePane();
  if (!pane?.terminalId || pane.status !== "connected") return toast("info", "Select a connected terminal to record.");
  rec.start(pane.id);
  toast("info", `Recording keystrokes in "${pane.spec.label}". Press the record button again to stop.`);
}

/** Show exactly what the macro will send, then play it into the active terminal. */
export async function runMacro(m: MacroProfile) {
  const pane = useWorkspace.getState().activePane();
  if (!pane?.terminalId || pane.status !== "connected") return toast("info", "Select a connected terminal first.");
  const text = m.steps.map((s) => (s.kind === "text" ? s.text : "")).join("");
  const danger = dangerReasons(text);
  const remote = pane.spec.kind !== "local";
  const ok = await openDialog<boolean>((close) => (
    <Modal
      title={`Run macro "${m.name}"`}
      onClose={() => close(false)}
      width={580}
      footer={
        <>
          <Button onClick={() => close(false)}>Cancel</Button>
          <Button variant={danger.length ? "danger" : "primary"} onClick={() => close(true)}>
            <Play size={13} /> Run in {pane.spec.label}
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-[13px]">
        <div>
          This sends the following to {remote ? <b className="text-warn">the remote session {pane.spec.label}</b> : <b>{pane.spec.label}</b>}:
        </div>
        <pre className="max-h-64 overflow-auto rounded-lg border border-line bg-elev p-3 font-mono text-[12px] whitespace-pre-wrap select-text">{describeSteps(m.steps)}</pre>
        {danger.length > 0 && (
          <div className="flex gap-2 rounded-lg border border-err/40 bg-err/10 p-3">
            <AlertTriangle size={16} className="shrink-0 text-err" /> Potentially destructive: {danger.join("; ")}.
          </div>
        )}
      </div>
    </Modal>
  ));
  if (!ok) return;
  const tid = pane.terminalId;
  for (const step of m.steps) {
    const current = useWorkspace.getState().allPanes().find((x) => x.pane.id === pane.id)?.pane;
    if (!current || current.terminalId !== tid || current.status !== "connected") return toast("warn", "Macro stopped: the terminal was closed or disconnected.");
    if (step.kind === "wait") await wait(step.ms);
    else await api.write(tid, step.text);
  }
  terminals.get(pane.id)?.term.focus();
}

export function MacrosPanel() {
  const [macros, setMacros] = useState<MacroProfile[]>([]);
  const recording = useMacroRecorder((s) => s.paneId);
  const recSteps = useMacroRecorder((s) => s.steps.length);
  const load = () => api.macrosList().then(setMacros).catch((e) => toast("error", toApiError(e).message));
  useEffect(() => {
    load();
  }, []);

  return (
    <div className="flex h-full flex-col">
      <div className="space-y-2 p-2">
        <Button variant={recording ? "danger" : "primary"} className="w-full justify-center" onClick={() => toggleMacroRecording(load)}>
          {recording ? <Square size={13} /> : <CircleDot size={13} />}
          {recording ? `Stop and save (${recSteps} steps)` : "Record from active terminal"}
        </Button>
        <p className="px-1 text-[11.5px] leading-snug text-faint">Records what you type, including pauses. Playback always shows the full content first.</p>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-1 pb-2">
        {macros.length === 0 && <div className="p-4 text-center text-[12px] text-dim">No macros yet.</div>}
        {macros.map((m) => (
          <div key={m.id} className="group flex items-center gap-1 rounded-md px-2 py-1.5 hover:bg-hover" onDoubleClick={() => runMacro(m)}>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[12.5px]">{m.name}</div>
              <div className="truncate font-mono text-[11px] text-faint">{m.steps.length} steps</div>
            </div>
            <IconButton title="Run in active terminal" className="hidden !h-6 !w-6 group-hover:inline-flex" onClick={() => runMacro(m)}>
              <Play size={12} />
            </IconButton>
            <IconButton
              title="Delete"
              className="hidden !h-6 !w-6 group-hover:inline-flex"
              onClick={async () => {
                if (await confirmDialog({ title: "Delete macro", message: `Delete "${m.name}"?`, confirmText: "Delete", danger: true })) {
                  await api.macroDelete(m.id);
                  load();
                }
              }}
            >
              <Trash2 size={12} />
            </IconButton>
          </div>
        ))}
      </div>
    </div>
  );
}
