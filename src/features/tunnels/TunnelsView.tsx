import { useEffect, useState } from "react";
import { Pencil, Play, Plus, RotateCw, Square, Trash2, Waypoints } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { useSessions } from "../../store/sessions";
import { openDialog, toast } from "../../store/ui";
import { Badge, Button, EmptyState, Field, IconButton, Modal, confirmDialog } from "../../components/ui";
import { withSshPrompts } from "../ssh/connect";
import type { TunnelProfile, TunnelStatus } from "../../types";

type Draft = Omit<TunnelProfile, "id" | "autoStart">;

function TunnelDialog({ tunnel, onClose }: { tunnel?: TunnelProfile; onClose: (saved: boolean) => void }) {
  const sessions = useSessions((s) => s.sessions.filter((x) => x.protocol === "ssh"));
  const [f, setF] = useState<Draft>({
    name: tunnel?.name ?? "",
    sessionId: tunnel?.sessionId ?? sessions[0]?.id ?? "",
    kind: tunnel?.kind ?? "local",
    bindAddress: tunnel?.bindAddress ?? "127.0.0.1",
    bindPort: tunnel?.bindPort ?? 8080,
    destHost: tunnel?.destHost ?? "localhost",
    destPort: tunnel?.destPort ?? 80,
  });
  const [err, setErr] = useState<string | null>(null);
  const save = async () => {
    try {
      await api.tunnelSave(tunnel?.id ?? null, f);
      onClose(true);
    } catch (e) {
      setErr(toApiError(e).message);
    }
  };
  const help = {
    local: "Local → Remote (-L): connections to the local port are forwarded through the SSH server to the destination.",
    remote: "Remote → Local (-R): the SSH server listens on the port and forwards connections back to the destination reachable from this PC.",
    dynamic: "Dynamic SOCKS5 proxy (-D): applications using the local SOCKS5 proxy reach any host via the SSH server.",
  }[f.kind];
  return (
    <Modal
      title={tunnel ? "Edit tunnel" : "New SSH tunnel"}
      onClose={() => onClose(false)}
      width={560}
      footer={
        <>
          {err && <span className="mr-auto self-center text-[12px] text-err">{err}</span>}
          <Button onClick={() => onClose(false)}>Cancel</Button>
          <Button variant="primary" onClick={save} disabled={!f.sessionId}>
            Save
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-3">
        <div className="col-span-2">
          <Field label="Tunnel name">
            <input className="field" autoFocus value={f.name} onChange={(e) => setF({ ...f, name: e.target.value })} placeholder="Postgres via bastion" />
          </Field>
        </div>
        <Field label="SSH gateway (session)">
          <select className="field" value={f.sessionId} onChange={(e) => setF({ ...f, sessionId: e.target.value })}>
            {sessions.length === 0 && <option value="">Create an SSH session first</option>}
            {sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
              </option>
            ))}
          </select>
        </Field>
        <Field label="Type">
          <select className="field" value={f.kind} onChange={(e) => setF({ ...f, kind: e.target.value as Draft["kind"] })}>
            <option value="local">Local forwarding (-L)</option>
            <option value="remote">Remote forwarding (-R)</option>
            <option value="dynamic">Dynamic SOCKS5 (-D)</option>
          </select>
        </Field>
        <div className="col-span-2 rounded-lg border border-line bg-elev p-2 text-[12px] text-dim">{help}</div>
        <Field label={f.kind === "remote" ? "Server listen address" : "Local listen address"} hint={f.kind !== "remote" ? "127.0.0.1 keeps the port private to this PC." : undefined}>
          <input className="field font-mono" value={f.bindAddress} onChange={(e) => setF({ ...f, bindAddress: e.target.value })} />
        </Field>
        <Field label={f.kind === "remote" ? "Server port" : "Local port"}>
          <input className="field" type="number" value={f.bindPort} onChange={(e) => setF({ ...f, bindPort: Number(e.target.value) })} />
        </Field>
        {f.kind !== "dynamic" && (
          <>
            <Field label={f.kind === "remote" ? "Destination host (from this PC)" : "Destination host (from the server)"}>
              <input className="field font-mono" value={f.destHost ?? ""} onChange={(e) => setF({ ...f, destHost: e.target.value })} />
            </Field>
            <Field label="Destination port">
              <input className="field" type="number" value={f.destPort ?? 0} onChange={(e) => setF({ ...f, destPort: Number(e.target.value) })} />
            </Field>
          </>
        )}
      </div>
    </Modal>
  );
}

export function TunnelsView() {
  const [tunnels, setTunnels] = useState<TunnelProfile[]>([]);
  const [status, setStatus] = useState<Record<string, TunnelStatus>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const byId = useSessions((s) => s.byId);

  const load = () => api.tunnelsList().then(setTunnels).catch((e) => toast("error", toApiError(e).message));
  const poll = () => api.tunnelStatuses().then((l) => setStatus(Object.fromEntries(l.map((s) => [s.id, s])))).catch(() => {});
  useEffect(() => {
    load();
    poll();
    const t = setInterval(poll, 2000);
    return () => clearInterval(t);
  }, []);

  const start = async (t: TunnelProfile) => {
    setBusy((b) => ({ ...b, [t.id]: true }));
    try {
      const r = await withSshPrompts(t.sessionId, (s) => api.tunnelStart(t.id, s.password, s.passphrase));
      if (r !== null) toast("success", `Tunnel “${t.name}” started`);
    } catch (e) {
      toast("error", `${t.name}: ${toApiError(e).message}`);
    } finally {
      setBusy((b) => ({ ...b, [t.id]: false }));
      poll();
    }
  };
  const stop = async (t: TunnelProfile) => {
    await api.tunnelStop(t.id).catch((e) => toast("error", toApiError(e).message));
    poll();
  };
  const edit = async (t?: TunnelProfile) => {
    if (await openDialog<boolean>((close) => <TunnelDialog tunnel={t} onClose={close} />)) load();
  };

  return (
    <div className="flex h-full flex-col rounded-lg border border-line bg-panel">
      <div className="flex items-center gap-2 border-b border-line px-4 py-3">
        <Waypoints size={16} className="text-accent" />
        <div className="text-[14px] font-semibold">SSH Tunnels</div>
        <div className="text-[12px] text-dim">Tunnels only run when you start them. Each running tunnel uses its own SSH connection.</div>
        <div className="flex-1" />
        <Button variant="primary" onClick={() => edit()}>
          <Plus size={14} /> New tunnel
        </Button>
      </div>
      {tunnels.length === 0 ? (
        <EmptyState icon={<Waypoints size={36} />} title="No tunnels yet">
          Forward a local port through an SSH server (-L), expose a local service on the server (-R), or run a SOCKS5 proxy (-D).
        </EmptyState>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full text-[12.5px]">
            <thead className="sticky top-0 bg-panel-2 text-left text-[11.5px] text-dim">
              <tr>
                {["Name", "Type", "Listen", "Destination", "Gateway", "Status", ""].map((h) => (
                  <th key={h} className="px-3 py-2 font-medium">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {tunnels.map((t) => {
                const st = status[t.id];
                const running = !!st;
                const s = byId(t.sessionId);
                return (
                  <tr key={t.id} className="border-t border-line hover:bg-hover">
                    <td className="px-3 py-2 font-medium">{t.name}</td>
                    <td className="px-3 py-2"><Badge tone="accent">{t.kind === "local" ? "-L local" : t.kind === "remote" ? "-R remote" : "-D SOCKS5"}</Badge></td>
                    <td className="px-3 py-2 font-mono text-[12px]">{t.bindAddress}:{t.bindPort}</td>
                    <td className="px-3 py-2 font-mono text-[12px]">{t.kind === "dynamic" ? "any (SOCKS5)" : `${t.destHost}:${t.destPort}`}</td>
                    <td className="px-3 py-2 text-dim">{s ? `${s.name}` : "(deleted session)"}</td>
                    <td className="px-3 py-2">
                      {busy[t.id] ? <Badge tone="warn">starting…</Badge> : running ? (
                        <span className="flex items-center gap-2">
                          <Badge tone={st.state === "running" ? "ok" : "err"}>{st.state}</Badge>
                          <span className="text-[11px] text-dim">{st.activeConnections} active / {st.totalConnections} total</span>
                          {st.error && <span className="max-w-48 truncate text-[11px] text-err" title={st.error}>{st.error}</span>}
                        </span>
                      ) : <Badge>stopped</Badge>}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-0.5">
                        {running ? (
                          <>
                            <IconButton title="Stop" onClick={() => stop(t)}><Square size={14} /></IconButton>
                            <IconButton title="Restart" onClick={async () => { await stop(t); await start(t); }}><RotateCw size={14} /></IconButton>
                          </>
                        ) : (
                          <IconButton title="Start" disabled={busy[t.id]} onClick={() => start(t)}><Play size={14} /></IconButton>
                        )}
                        <IconButton title={running ? "Stop the tunnel to edit" : "Edit"} disabled={running} onClick={() => edit(t)}><Pencil size={14} /></IconButton>
                        <IconButton
                          title="Delete"
                          onClick={async () => {
                            if (await confirmDialog({ title: "Delete tunnel", message: `Delete tunnel “${t.name}”?${running ? " It will be stopped." : ""}`, confirmText: "Delete", danger: true })) {
                              await api.tunnelDelete(t.id);
                              load();
                              poll();
                            }
                          }}
                        >
                          <Trash2 size={14} />
                        </IconButton>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
