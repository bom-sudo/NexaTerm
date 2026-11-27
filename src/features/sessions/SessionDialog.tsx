import { useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Cable, FolderOpen, KeyRound, Monitor, Network, Server } from "lucide-react";
import { useEffect } from "react";
import type { PortInfo, SerialSettings, XStatus } from "../../types";
import { api, toApiError } from "../../services/api";
import { useSessions } from "../../store/sessions";
import { Button, Field, Modal } from "../../components/ui";
import type { AuthMethod, Protocol, Session, SessionInput } from "../../types";

interface Props {
  session?: Session;
  initial?: Partial<Session>;
  onClose: (s: Session | null) => void;
}

function toInput(s?: Partial<Session>): SessionInput {
  return {
    name: s?.name ?? "",
    groupId: s?.groupId ?? null,
    protocol: (s?.protocol as Protocol) ?? "ssh",
    host: s?.host ?? "",
    port: s?.port ?? 22,
    username: s?.username ?? "",
    authMethod: (s?.authMethod as AuthMethod) ?? "password",
    keyPath: s?.keyPath ?? "",
    jumpSessionId: s?.jumpSessionId ?? null,
    options: { connectTimeoutSecs: 15, keepaliveSecs: 30, termType: "xterm-256color", retries: 0, ...(s?.options ?? {}) },
    favorite: s?.favorite ?? false,
    notes: s?.notes ?? "",
    tags: s?.tags ?? [],
  };
}

export function SessionDialog({ session, initial, onClose }: Props) {
  const { groups, sessions, shells } = useSessions();
  const [f, setF] = useState<SessionInput>(() => toInput(session ?? initial));
  const [password, setPassword] = useState<string | undefined>(undefined);
  const [passphrase, setPassphrase] = useState<string | undefined>(undefined);
  const [tagText, setTagText] = useState((session?.tags ?? initial?.tags ?? []).join(", "));
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tab, setTab] = useState<"general" | "advanced">("general");

  const set = <K extends keyof SessionInput>(k: K, v: SessionInput[K]) => setF((x) => ({ ...x, [k]: v }));
  const setOpt = (k: keyof SessionInput["options"], v: unknown) => setF((x) => ({ ...x, options: { ...x.options, [k]: v } }));
  const jumpCandidates = useMemo(() => sessions.filter((s) => s.protocol === "ssh" && s.id !== session?.id), [sessions, session]);
  const shellMatch = shells.find((s) => s.path === f.options.shellPath);

  const browseKey = async () => {
    const p = await open({ title: "Select private key", multiple: false, defaultPath: await api.keysDefaultDir() });
    if (typeof p === "string") set("keyPath", p);
  };

  const submit = async () => {
    setErr(null);
    setBusy(true);
    try {
      const input: SessionInput = {
        ...f,
        name:
          f.name.trim() ||
          (f.protocol === "ssh" ? `${f.username}@${f.host}` : f.protocol === "telnet" ? `telnet ${f.host}` : f.protocol === "serial" ? (f.options.serial?.port ?? "Serial") : "Local terminal"),
        tags: tagText.split(",").map((t) => t.trim()).filter(Boolean),
        port: f.protocol === "ssh" ? Number(f.port) || 22 : f.protocol === "telnet" ? Number(f.port) || 23 : null,
        host: f.protocol === "ssh" || f.protocol === "telnet" ? f.host : null,
        options: f.protocol === "serial" ? { ...f.options, serial: f.options.serial ?? DEFAULT_SERIAL } : f.options,
        username: f.protocol === "ssh" ? f.username : null,
        authMethod: f.protocol === "ssh" ? f.authMethod : null,
        keyPath: f.protocol === "ssh" && f.authMethod === "key" ? f.keyPath : null,
        password,
        passphrase,
      };
      const saved = session ? await api.sessionUpdate(session.id, input) : await api.sessionCreate(input);
      onClose(saved);
    } catch (e) {
      setErr(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const ssh = f.protocol === "ssh";
  return (
    <Modal
      title={session ? `Edit session — ${session.name}` : "New session"}
      onClose={() => onClose(null)}
      width={640}
      footer={
        <>
          {err && <div className="mr-auto self-center text-[12px] text-err">{err}</div>}
          <Button onClick={() => onClose(null)}>Cancel</Button>
          <Button variant="primary" onClick={submit} disabled={busy}>
            {session ? "Save" : "Create session"}
          </Button>
        </>
      }
    >
      <form
        className="space-y-4"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="flex gap-2">
          {(
            [
              ["ssh", "SSH", <Server size={15} key="s" />],
              ["local", "Local terminal", <Monitor size={15} key="l" />],
              ["telnet", "Telnet", <Network size={15} key="t" />],
              ["serial", "Serial (COM)", <Cable size={15} key="c" />],
            ] as const
          ).map(([p, label, icon]) => (
            <button
              type="button"
              key={p}
              onClick={() => set("protocol", p)}
              className={`flex flex-1 items-center justify-center gap-2 rounded-lg border px-3 py-2 text-[12.5px] ${
                f.protocol === p ? "border-accent bg-accent-soft text-accent" : "border-line text-dim hover:bg-hover"
              }`}
            >
              {icon}
              {label}
            </button>
          ))}
        </div>
        <div className="flex gap-1 border-b border-line">
          {(["general", "advanced"] as const).map((t) => (
            <button type="button" key={t} onClick={() => setTab(t)} className={`-mb-px border-b-2 px-3 py-1.5 text-[12.5px] capitalize ${tab === t ? "border-accent text-fg" : "border-transparent text-dim"}`}>
              {t}
            </button>
          ))}
        </div>

        {tab === "general" && (
          <div className="grid grid-cols-2 gap-3">
            <div className="col-span-2">
              <Field label="Session name">
                <input className="field" autoFocus value={f.name} onChange={(e) => set("name", e.target.value)} placeholder={ssh ? "web-01 (production)" : "PowerShell"} />
              </Field>
            </div>
            {f.protocol === "telnet" && (
              <div className="col-span-2 grid grid-cols-[1fr_100px] gap-3">
                <Field label="Host / IP address" hint="Telnet sends everything, including passwords, unencrypted. Use it only for devices that offer nothing else.">
                  <input className="field" value={f.host ?? ""} onChange={(e) => set("host", e.target.value)} placeholder="switch-01.lan" />
                </Field>
                <Field label="Port">
                  <input className="field" type="number" min={1} max={65535} value={f.port ?? 23} onChange={(e) => set("port", Number(e.target.value))} />
                </Field>
              </div>
            )}
            {f.protocol === "serial" && (
              <SerialFields value={f.options.serial ?? DEFAULT_SERIAL} onChange={(v) => setOpt("serial", v)} />
            )}
            {ssh ? (
              <>
                <div className="col-span-2 grid grid-cols-[1fr_100px] gap-3">
                  <Field label="Host / IP address">
                    <input className="field" value={f.host ?? ""} onChange={(e) => set("host", e.target.value)} placeholder="server.example.com" />
                  </Field>
                  <Field label="Port">
                    <input className="field" type="number" min={1} max={65535} value={f.port ?? 22} onChange={(e) => set("port", Number(e.target.value))} />
                  </Field>
                </div>
                <Field label="Username">
                  <input className="field" value={f.username ?? ""} onChange={(e) => set("username", e.target.value)} placeholder="root" />
                </Field>
                <Field label="Authentication">
                  <select className="field" value={f.authMethod ?? "password"} onChange={(e) => set("authMethod", e.target.value as AuthMethod)}>
                    <option value="password">Password</option>
                    <option value="key">Private key</option>
                    <option value="agent">SSH agent (Windows OpenSSH)</option>
                    <option value="keyboardInteractive">Keyboard-interactive</option>
                  </select>
                </Field>
                {(f.authMethod === "password" || f.authMethod === "keyboardInteractive") && (
                  <div className="col-span-2">
                    <Field
                      label="Password"
                      hint={
                        session?.hasPassword && password === undefined ? (
                          <span>
                            A password is saved in Windows Credential Manager.{" "}
                            <button type="button" className="text-accent hover:underline" onClick={() => setPassword("")}>
                              Remove it
                            </button>
                          </span>
                        ) : password === "" && session?.hasPassword ? (
                          "Saved password will be removed."
                        ) : (
                          "Optional. Stored only in Windows Credential Manager. Leave empty to be asked on connect."
                        )
                      }
                    >
                      <input className="field" type="password" autoComplete="new-password" value={password ?? ""} onChange={(e) => setPassword(e.target.value)} placeholder={session?.hasPassword ? "•••••••• (unchanged)" : ""} />
                    </Field>
                  </div>
                )}
                {f.authMethod === "key" && (
                  <>
                    <div className="col-span-2">
                      <Field label="Private key file">
                        <div className="flex gap-2">
                          <input className="field font-mono" value={f.keyPath ?? ""} onChange={(e) => set("keyPath", e.target.value)} placeholder="C:\Users\you\.ssh\id_ed25519" />
                          <Button type="button" onClick={browseKey} title="Browse">
                            <FolderOpen size={14} />
                          </Button>
                        </div>
                      </Field>
                    </div>
                    <div className="col-span-2">
                      <Field
                        label="Key passphrase"
                        hint={
                          session?.hasPassphrase && passphrase === undefined ? (
                            <span>
                              Saved in Credential Manager.{" "}
                              <button type="button" className="text-accent hover:underline" onClick={() => setPassphrase("")}>
                                Remove it
                              </button>
                            </span>
                          ) : (
                            "Only for encrypted keys. Leave empty to be asked when needed."
                          )
                        }
                      >
                        <input className="field" type="password" autoComplete="new-password" value={passphrase ?? ""} onChange={(e) => setPassphrase(e.target.value)} />
                      </Field>
                    </div>
                  </>
                )}
                {f.authMethod === "agent" && (
                  <div className="col-span-2 flex items-start gap-2 rounded-lg border border-line bg-elev p-3 text-[12px] text-dim">
                    <KeyRound size={14} className="mt-0.5 shrink-0" />
                    Uses keys loaded into the Windows "OpenSSH Authentication Agent" service (ssh-add). The service must be running.
                  </div>
                )}
                <div className="col-span-2">
                  <Field label="Jump host (ProxyJump)" hint="Connect through another saved SSH session. Its credentials must be saved.">
                    <select className="field" value={f.jumpSessionId ?? ""} onChange={(e) => set("jumpSessionId", e.target.value || null)}>
                      <option value="">None — connect directly</option>
                      {jumpCandidates.map((s) => (
                        <option key={s.id} value={s.id}>
                          {s.name} ({s.username}@{s.host})
                        </option>
                      ))}
                    </select>
                  </Field>
                </div>
              </>
            ) : f.protocol === "local" ? (
              <>
                <div className="col-span-2">
                  <Field label="Shell">
                    <select
                      className="field"
                      value={shellMatch?.id ?? (f.options.shellPath ? "custom" : "")}
                      onChange={(e) => {
                        const sh = shells.find((s) => s.id === e.target.value);
                        if (sh) setF((x) => ({ ...x, options: { ...x.options, shellPath: sh.path, shellArgs: sh.args } }));
                        else if (e.target.value === "") setF((x) => ({ ...x, options: { ...x.options, shellPath: null, shellArgs: null } }));
                        else setF((x) => ({ ...x, options: { ...x.options, shellPath: x.options.shellPath ?? "", shellArgs: [] } }));
                      }}
                    >
                      <option value="">Default shell (from Settings)</option>
                      {shells.map((s) => (
                        <option key={s.id} value={s.id}>
                          {s.name}
                        </option>
                      ))}
                      <option value="custom">Custom executable…</option>
                    </select>
                  </Field>
                </div>
                {f.options.shellPath !== null && f.options.shellPath !== undefined && !shellMatch && (
                  <>
                    <div className="col-span-2">
                      <Field label="Executable path">
                        <input className="field font-mono" value={f.options.shellPath ?? ""} onChange={(e) => setOpt("shellPath", e.target.value)} placeholder="C:\Program Files\...\shell.exe" />
                      </Field>
                    </div>
                    <div className="col-span-2">
                      <Field label="Arguments" hint="One argument per line (passed as structured arguments, never through a shell).">
                        <textarea className="field font-mono" rows={2} value={(f.options.shellArgs ?? []).join("\n")} onChange={(e) => setOpt("shellArgs", e.target.value.split("\n").filter((a) => a !== ""))} />
                      </Field>
                    </div>
                  </>
                )}
                <div className="col-span-2">
                  <Field label="Starting directory">
                    <input className="field font-mono" value={f.options.cwd ?? ""} onChange={(e) => setOpt("cwd", e.target.value || null)} placeholder="%USERPROFILE%" />
                  </Field>
                </div>
              </>
            ) : null}
            <Field label="Folder">
              <select className="field" value={f.groupId ?? ""} onChange={(e) => set("groupId", e.target.value || null)}>
                <option value="">(none)</option>
                {groups.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.name}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Tags (comma separated)">
              <input className="field" value={tagText} onChange={(e) => setTagText(e.target.value)} placeholder="prod, web" />
            </Field>
            <label className="col-span-2 flex items-center gap-2 text-[12.5px]">
              <input type="checkbox" checked={f.favorite} onChange={(e) => set("favorite", e.target.checked)} /> Add to favorites
            </label>
          </div>
        )}

        {tab === "advanced" && (
          <div className="grid grid-cols-2 gap-3">
            {ssh && (
              <>
                <Field label="Connection timeout (s)">
                  <input className="field" type="number" min={1} max={300} value={f.options.connectTimeoutSecs ?? 15} onChange={(e) => setOpt("connectTimeoutSecs", Number(e.target.value))} />
                </Field>
                <Field label="Keepalive interval (s, 0 = off)">
                  <input className="field" type="number" min={0} max={3600} value={f.options.keepaliveSecs ?? 30} onChange={(e) => setOpt("keepaliveSecs", Number(e.target.value))} />
                </Field>
                <Field label="Connection retries" hint="Network failures only; auth errors never retry.">
                  <input className="field" type="number" min={0} max={5} value={f.options.retries ?? 0} onChange={(e) => setOpt("retries", Number(e.target.value))} />
                </Field>
                <Field label="Terminal type">
                  <select className="field" value={f.options.termType ?? "xterm-256color"} onChange={(e) => setOpt("termType", e.target.value)}>
                    {["xterm-256color", "xterm", "vt100", "linux", "screen-256color", "tmux-256color"].map((t) => (
                      <option key={t}>{t}</option>
                    ))}
                  </select>
                </Field>
                <div className="col-span-2">
                  <Field label="Startup directory">
                    <input className="field font-mono" value={f.options.startupDir ?? ""} onChange={(e) => setOpt("startupDir", e.target.value || null)} placeholder="/var/www" />
                  </Field>
                </div>
                <div className="col-span-2">
                  <Field label="Startup command" hint="Typed into the shell after login, exactly as if you typed it.">
                    <input className="field font-mono" value={f.options.startupCommand ?? ""} onChange={(e) => setOpt("startupCommand", e.target.value || null)} placeholder="sudo -i" />
                  </Field>
                </div>
                <div className="col-span-2 space-y-2 rounded-lg border border-line p-3">
                  <Check label="Compression (zlib)" hint="Helps on slow links; costs CPU on fast ones." checked={!!f.options.compression} onChange={(v) => setOpt("compression", v)} />
                  <Check
                    label="Follow terminal folder in the SFTP side panel"
                    hint="Installs a small prompt hook (bash/zsh) when the shell starts, so the side panel follows cd."
                    checked={!!f.options.followTerminalFolder}
                    onChange={(v) => setOpt("followTerminalFolder", v)}
                  />
                  <Check
                    label="Forward SSH agent (ssh -A)"
                    hint="Lets this server use your Windows OpenSSH agent keys to log in further. Enable only for servers you trust: their admins could use your keys while you are connected."
                    checked={!!f.options.agentForward}
                    onChange={(v) => setOpt("agentForward", v)}
                  />
                  <Check label="X11 forwarding" hint="Show Linux GUI programs on this PC through a local X server." checked={!!f.options.x11Forward} onChange={(v) => setOpt("x11Forward", v)} />
                  {f.options.x11Forward && (
                    <div className="ml-6 space-y-2">
                      <Field label="Local X display">
                        <input className="field !w-48 font-mono" value={f.options.x11Display ?? "localhost:0"} onChange={(e) => setOpt("x11Display", e.target.value)} />
                      </Field>
                      <XServerStatus />
                    </div>
                  )}
                </div>
              </>
            )}
            <div className="col-span-2">
              <Field label="Notes">
                <textarea className="field" rows={3} value={f.notes ?? ""} onChange={(e) => set("notes", e.target.value)} />
              </Field>
            </div>
          </div>
        )}
        <button type="submit" className="hidden" />
      </form>
    </Modal>
  );
}

const DEFAULT_SERIAL: SerialSettings = { port: "COM1", baud: 9600, dataBits: 8, parity: "none", stopBits: "one", flow: "none" };

function Check({ label, hint, checked, onChange }: { label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex items-start gap-2 text-[12.5px]">
      <input type="checkbox" className="mt-1" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        {label}
        {hint && <span className="block text-[11px] text-faint">{hint}</span>}
      </span>
    </label>
  );
}

function SerialFields({ value, onChange }: { value: SerialSettings; onChange: (v: SerialSettings) => void }) {
  const [ports, setPorts] = useState<PortInfo[]>([]);
  useEffect(() => {
    api.serialPorts().then(setPorts).catch(() => setPorts([]));
  }, []);
  const set = <K extends keyof SerialSettings>(k: K, v: SerialSettings[K]) => onChange({ ...value, [k]: v });
  return (
    <div className="col-span-2 grid grid-cols-3 gap-3">
      <Field label="Port" hint={ports.length ? `${ports.length} port(s) found` : "No COM ports detected; you can still type one"}>
        <input className="field font-mono" list="nx-com-ports" value={value.port} onChange={(e) => set("port", e.target.value.toUpperCase())} />
        <datalist id="nx-com-ports">
          {ports.map((p) => (
            <option key={p.name} value={p.name}>
              {p.description ?? ""}
            </option>
          ))}
        </datalist>
      </Field>
      <Field label="Speed (baud)">
        <select className="field" value={value.baud} onChange={(e) => set("baud", Number(e.target.value))}>
          {[1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600].map((b) => (
            <option key={b}>{b}</option>
          ))}
        </select>
      </Field>
      <Field label="Data bits">
        <select className="field" value={value.dataBits} onChange={(e) => set("dataBits", Number(e.target.value))}>
          {[5, 6, 7, 8].map((b) => (
            <option key={b}>{b}</option>
          ))}
        </select>
      </Field>
      <Field label="Parity">
        <select className="field" value={value.parity} onChange={(e) => set("parity", e.target.value as SerialSettings["parity"])}>
          {["none", "odd", "even", "mark", "space"].map((p) => (
            <option key={p}>{p}</option>
          ))}
        </select>
      </Field>
      <Field label="Stop bits">
        <select className="field" value={value.stopBits} onChange={(e) => set("stopBits", e.target.value as SerialSettings["stopBits"])}>
          <option value="one">1</option>
          <option value="onePointFive">1.5</option>
          <option value="two">2</option>
        </select>
      </Field>
      <Field label="Flow control">
        <select className="field" value={value.flow} onChange={(e) => set("flow", e.target.value as SerialSettings["flow"])}>
          <option value="none">None</option>
          <option value="xonXoff">XON/XOFF</option>
          <option value="rtsCts">RTS/CTS</option>
        </select>
      </Field>
    </div>
  );
}

function XServerStatus() {
  const [st, setSt] = useState<XStatus | null>(null);
  const refresh = () => api.x11Status().then(setSt).catch(() => setSt(null));
  useEffect(() => {
    refresh();
  }, []);
  if (!st) return null;
  if (st.running) return <div className="text-[12px] text-ok">An X server is running on display :0.</div>;
  return (
    <div className="rounded-lg border border-warn/40 bg-warn/10 p-2 text-[12px]">
      No X server is running on this PC.{" "}
      {st.installed.length > 0 ? (
        st.installed.map((x) => (
          <button key={x.id} type="button" className="ml-1 text-accent hover:underline" onClick={() => api.x11Start(x.id).then(() => setTimeout(refresh, 1500))}>
            Start {x.name}
          </button>
        ))
      ) : (
        <span>Install a free X server such as VcXsrv, Xming or X410, start it with access control disabled, then connect again.</span>
      )}
    </div>
  );
}
