import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Copy, FileDown, FolderOpen, KeyRound, Lock, LockOpen, Plus, Trash2, Upload, Eye } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { openDialog, toast } from "../../store/ui";
import { Badge, Button, EmptyState, Field, IconButton, Modal, confirmDialog } from "../../components/ui";
import type { KeyInfo, SshKeyRecord } from "../../types";

function GenerateDialog({ onClose }: { onClose: (k: KeyInfo | null) => void }) {
  const [kind, setKind] = useState<"ed25519" | "rsa">("ed25519");
  const [bits, setBits] = useState(4096);
  const [path, setPath] = useState("");
  const [comment, setComment] = useState("");
  const [pass, setPass] = useState("");
  const [pass2, setPass2] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.keysDefaultDir().then((d) => setPath(`${d}\\id_${kind}_nexaterm`));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const go = async () => {
    if (pass !== pass2) return setErr("Passphrases do not match");
    setBusy(true);
    setErr(null);
    try {
      onClose(await api.keyGenerate(path, kind, kind === "rsa" ? bits : null, comment, pass || null));
    } catch (e) {
      setErr(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal
      title="Generate SSH key pair"
      onClose={() => onClose(null)}
      width={560}
      footer={
        <>
          {err && <span className="mr-auto self-center text-[12px] text-err">{err}</span>}
          <Button onClick={() => onClose(null)}>Cancel</Button>
          <Button variant="primary" onClick={go} disabled={busy || !path}>
            {busy ? "Generating…" : "Generate"}
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-2 gap-3">
        <Field label="Type">
          <select className="field" value={kind} onChange={(e) => { const k = e.target.value as "ed25519" | "rsa"; setKind(k); setPath((p) => p.replace(/id_(ed25519|rsa)_/, `id_${k}_`)); }}>
            <option value="ed25519">Ed25519 (recommended)</option>
            <option value="rsa">RSA</option>
          </select>
        </Field>
        <Field label="RSA key size">
          <select className="field" disabled={kind !== "rsa"} value={bits} onChange={(e) => setBits(Number(e.target.value))}>
            {[2048, 3072, 4096].map((b) => <option key={b} value={b}>{b} bits</option>)}
          </select>
        </Field>
        <div className="col-span-2">
          <Field label="Private key file" hint="The public key is written next to it with .pub. Existing files are never overwritten.">
            <div className="flex gap-2">
              <input className="field font-mono" value={path} onChange={(e) => setPath(e.target.value)} />
              <Button type="button" onClick={async () => { const p = await save({ title: "Save private key as", defaultPath: path }); if (p) setPath(p); }}>
                <FolderOpen size={14} />
              </Button>
            </div>
          </Field>
        </div>
        <div className="col-span-2">
          <Field label="Comment">
            <input className="field" value={comment} onChange={(e) => setComment(e.target.value)} placeholder="you@workstation" />
          </Field>
        </div>
        <Field label="Passphrase (recommended)">
          <input className="field" type="password" value={pass} onChange={(e) => setPass(e.target.value)} />
        </Field>
        <Field label="Repeat passphrase">
          <input className="field" type="password" value={pass2} onChange={(e) => setPass2(e.target.value)} />
        </Field>
      </div>
    </Modal>
  );
}

function PassphraseDialog({ k, onClose }: { k: SshKeyRecord; onClose: (ok: boolean) => void }) {
  const [old, setOld] = useState("");
  const [n1, setN1] = useState("");
  const [n2, setN2] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const go = async () => {
    if (n1 !== n2) return setErr("New passphrases do not match");
    try {
      await api.keyChangePassphrase(k.privatePath, old || null, n1 || null);
      onClose(true);
    } catch (e) {
      setErr(toApiError(e).message);
    }
  };
  return (
    <Modal title={`Change passphrase — ${k.name}`} onClose={() => onClose(false)} width={460} footer={<>{err && <span className="mr-auto self-center text-[12px] text-err">{err}</span>}<Button onClick={() => onClose(false)}>Cancel</Button><Button variant="primary" onClick={go}>Save</Button></>}>
      <div className="space-y-3">
        {k.encrypted && <Field label="Current passphrase"><input className="field" type="password" autoFocus value={old} onChange={(e) => setOld(e.target.value)} /></Field>}
        <Field label="New passphrase" hint="Leave empty to remove the passphrase (not recommended)."><input className="field" type="password" value={n1} onChange={(e) => setN1(e.target.value)} /></Field>
        <Field label="Repeat new passphrase"><input className="field" type="password" value={n2} onChange={(e) => setN2(e.target.value)} /></Field>
      </div>
    </Modal>
  );
}

export function KeysView() {
  const [keys, setKeys] = useState<SshKeyRecord[]>([]);
  const [problems, setProblems] = useState<Record<string, string>>({});
  const load = async () => {
    const list = await api.keysList();
    setKeys(list);
    const p: Record<string, string> = {};
    await Promise.all(list.map((k) => api.keyInspect(k.privatePath).catch((e) => (p[k.id] = toApiError(e).message))));
    setProblems(p);
  };
  useEffect(() => {
    load().catch((e) => toast("error", toApiError(e).message));
  }, []);

  const showPublic = async (k: SshKeyRecord) => {
    try {
      const info = await api.keyInspect(k.privatePath);
      await openDialog<null>((close) => (
        <Modal title={`Public key — ${k.name}`} onClose={() => close(null)} width={640} footer={<><Button onClick={() => writeText(info.publicKey).then(() => toast("success", "Public key copied"))}><Copy size={13} /> Copy</Button><Button variant="primary" onClick={() => close(null)}>Close</Button></>}>
          <div className="mb-2 text-[12px] text-dim">Add this line to <code>~/.ssh/authorized_keys</code> on the server.</div>
          <pre className="rounded-lg border border-line bg-elev p-3 font-mono text-[11.5px] break-all whitespace-pre-wrap select-text">{info.publicKey}</pre>
        </Modal>
      ));
    } catch (e) {
      toast("error", toApiError(e).message);
    }
  };

  return (
    <div className="flex h-full flex-col rounded-lg border border-line bg-panel">
      <div className="flex items-center gap-2 border-b border-line px-4 py-3">
        <KeyRound size={16} className="text-accent" />
        <div className="text-[14px] font-semibold">SSH Keys</div>
        <div className="text-[12px] text-dim">Private keys stay in their files — NexaTerm only remembers the path.</div>
        <div className="flex-1" />
        <Button onClick={async () => {
          const p = await open({ title: "Import private key", defaultPath: await api.keysDefaultDir() });
          if (typeof p === "string") api.keyImport(p).then(() => { toast("success", "Key added"); load(); }).catch((e) => toast("error", toApiError(e).message));
        }}>
          <Upload size={14} /> Import existing
        </Button>
        <Button variant="primary" onClick={async () => { const k = await openDialog<KeyInfo | null>((close) => <GenerateDialog onClose={close} />); if (k) { toast("success", `Generated ${k.path}`); load(); } }}>
          <Plus size={14} /> Generate
        </Button>
      </div>
      {keys.length === 0 ? (
        <EmptyState icon={<KeyRound size={36} />} title="No keys yet">Generate an Ed25519 key, or import an existing OpenSSH / PEM private key file.</EmptyState>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          <table className="w-full text-[12.5px]">
            <thead className="sticky top-0 bg-panel-2 text-left text-[11.5px] text-dim">
              <tr>{["Name", "Type", "Fingerprint", "File", "Protection", ""].map((h) => <th key={h} className="px-3 py-2 font-medium">{h}</th>)}</tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={k.id} className="border-t border-line hover:bg-hover">
                  <td className="px-3 py-2 font-medium">{k.name}<div className="text-[11px] text-faint">{k.comment}</div></td>
                  <td className="px-3 py-2"><Badge tone="accent">{k.keyType}</Badge></td>
                  <td className="px-3 py-2 font-mono text-[11px] select-text">{k.fingerprint}</td>
                  <td className="max-w-64 truncate px-3 py-2 font-mono text-[11px] text-dim" title={k.privatePath}>
                    {k.privatePath}
                    {problems[k.id] && <div className="text-err" title={problems[k.id]}>⚠ {problems[k.id]}</div>}
                  </td>
                  <td className="px-3 py-2">{k.encrypted ? <Badge tone="ok"><Lock size={10} className="mr-1" />passphrase</Badge> : <Badge tone="warn"><LockOpen size={10} className="mr-1" />none</Badge>}</td>
                  <td className="px-3 py-2">
                    <div className="flex justify-end gap-0.5">
                      <IconButton title="View public key" onClick={() => showPublic(k)}><Eye size={14} /></IconButton>
                      <IconButton title="Copy public key" onClick={() => api.keyInspect(k.privatePath).then((i) => writeText(i.publicKey)).then(() => toast("success", "Public key copied")).catch((e) => toast("error", toApiError(e).message))}><Copy size={14} /></IconButton>
                      <IconButton title="Export public key to file" onClick={async () => {
                        const info = await api.keyInspect(k.privatePath).catch((e) => { toast("error", toApiError(e).message); return null; });
                        if (!info) return;
                        const p = await save({ title: "Export public key", defaultPath: `${k.name}.pub`, filters: [{ name: "Public key", extensions: ["pub"] }] });
                        if (p) api.writeTextFile(p, info.publicKey + "\n").then(() => toast("success", `Saved ${p}`));
                      }}><FileDown size={14} /></IconButton>
                      <IconButton title="Change passphrase" onClick={async () => { if (await openDialog<boolean>((close) => <PassphraseDialog k={k} onClose={close} />)) { toast("success", "Passphrase updated"); load(); } }}><Lock size={14} /></IconButton>
                      <IconButton title="Remove from list (file is kept)" onClick={async () => { if (await confirmDialog({ title: "Remove key from list", message: `Remove “${k.name}” from NexaTerm? The key file on disk is NOT deleted.`, confirmText: "Remove" })) { await api.keyForget(k.id); load(); } }}><Trash2 size={14} /></IconButton>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
