import { useState } from "react";
import { KeyRound } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../../services/api";
import { openDialog } from "../../store/ui";
import { Button, Field, Modal } from "../../components/ui";
import type { KbdPrompt } from "../../types";

function Body({ req, close }: { req: KbdPrompt; close: (v: string[] | null) => void }) {
  const [values, setValues] = useState<string[]>(req.prompts.map(() => ""));
  return (
    <Modal
      title={
        <span className="flex items-center gap-2">
          <KeyRound size={16} className="text-accent" /> {req.name || "Additional authentication"}
        </span>
      }
      onClose={() => close(null)}
      width={460}
    >
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          close(values);
        }}
      >
        <div className="text-[12.5px] text-dim">
          {req.username}@{req.host}
          {req.instructions && <div className="mt-1 whitespace-pre-wrap text-fg">{req.instructions}</div>}
        </div>
        {req.prompts.map((p, i) => (
          <Field key={i} label={p.text.trim() || `Answer ${i + 1}`}>
            <input
              className="field font-mono"
              autoFocus={i === 0}
              type={p.echo ? "text" : "password"}
              autoComplete="one-time-code"
              value={values[i]}
              onChange={(e) => setValues(values.map((v, j) => (j === i ? e.target.value : v)))}
            />
          </Field>
        ))}
        <div className="flex justify-end gap-2">
          <Button type="button" onClick={() => close(null)}>
            Cancel login
          </Button>
          <Button variant="primary" type="submit">
            Continue
          </Button>
        </div>
      </form>
    </Modal>
  );
}

/** Show keyboard-interactive questions (password, one-time code ...) raised by SSH logins. */
export function installKbdPromptListener(): () => void {
  let off: (() => void) | undefined;
  listen<KbdPrompt>("ssh-prompt", async (e) => {
    const req = e.payload;
    const answers = await openDialog<string[] | null>((close) => <Body req={req} close={close} />);
    await api.sshPromptAnswer(req.id, answers);
  }).then((u) => (off = u));
  return () => off?.();
}
