import { useState } from "react";
import { api, toApiError } from "../../services/api";
import { useSessions } from "../../store/sessions";
import { Button, Field, Modal } from "../../components/ui";
import { connectSession } from "../../app/actions";
import type { AuthMethod } from "../../types";

/** Parses `user@host:port`, `host`, `host:port`, `user@[v6]:port`. */
export function parseTarget(text: string): { user: string; host: string; port: number } | null {
  const t = text.trim().replace(/^ssh:\/\//, "");
  const m = /^(?:([^@\s]+)@)?(\[[^\]]+\]|[^:\s]+)(?::(\d{1,5}))?$/.exec(t);
  if (!m) return null;
  const port = m[3] ? Number(m[3]) : 22;
  if (port < 1 || port > 65535) return null;
  return { user: m[1] ?? "", host: m[2].replace(/^\[|\]$/g, ""), port };
}

export function QuickConnectDialog({ onClose }: { onClose: () => void }) {
  const [target, setTarget] = useState("");
  const [auth, setAuth] = useState<AuthMethod>("password");
  const [err, setErr] = useState<string | null>(null);

  const go = async () => {
    const p = parseTarget(target);
    if (!p || !p.user) return setErr("Use the form user@host or user@host:port");
    try {
      const s = await api.sessionCreate({
        name: `${p.user}@${p.host}${p.port !== 22 ? `:${p.port}` : ""}`,
        groupId: null,
        protocol: "ssh",
        host: p.host,
        port: p.port,
        username: p.user,
        authMethod: auth,
        keyPath: null,
        jumpSessionId: null,
        options: { connectTimeoutSecs: 15, keepaliveSecs: 30 },
        favorite: false,
        notes: null,
        tags: ["quick-connect"],
      });
      await useSessions.getState().refresh();
      onClose();
      connectSession(s);
    } catch (e) {
      setErr(toApiError(e).message);
    }
  };

  return (
    <Modal title="Quick Connect (SSH)" onClose={onClose} width={480}>
      <form
        className="space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          go();
        }}
      >
        <Field label="Target" hint="Saved to your sessions (tag: quick-connect) so you can reconnect later. Passwords are only saved if you choose to.">
          <input autoFocus className="field font-mono" placeholder="user@host:22" value={target} onChange={(e) => setTarget(e.target.value)} />
        </Field>
        <Field label="Authentication">
          <select className="field" value={auth} onChange={(e) => setAuth(e.target.value as AuthMethod)}>
            <option value="password">Password (prompt)</option>
            <option value="agent">SSH agent</option>
            <option value="keyboardInteractive">Keyboard-interactive</option>
          </select>
        </Field>
        {err && <div className="text-[12px] text-err">{err}</div>}
        <div className="flex justify-end gap-2">
          <Button type="button" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" type="submit">
            Connect
          </Button>
        </div>
      </form>
    </Modal>
  );
}
