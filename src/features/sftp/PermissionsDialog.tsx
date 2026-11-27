import { useState } from "react";
import { api, toApiError } from "../../services/api";
import { Button, Field, Modal } from "../../components/ui";
import type { RemoteEntry } from "../../types";

const WHO = ["Owner", "Group", "Others"] as const;
const WHAT = [
  ["Read", 4],
  ["Write", 2],
  ["Execute", 1],
] as const;

/** Parse "755" / "0644" style octal text; null when invalid. */
export function parseOctal(text: string): number | null {
  const t = text.trim();
  if (!/^[0-7]{3,4}$/.test(t)) return null;
  return parseInt(t, 8);
}

/** chmod (with optional recursion) and chown by numeric uid/gid. */
export function PermissionsDialog({ sftpId, entry, onClose }: { sftpId: string; entry: RemoteEntry; onClose: (changed: boolean) => void }) {
  const initial = (entry.mode ?? 0o644) & 0o7777;
  const [mode, setMode] = useState(initial);
  const [octal, setOctal] = useState(initial.toString(8).padStart(3, "0"));
  const [recursive, setRecursive] = useState(false);
  const [uid, setUid] = useState("");
  const [gid, setGid] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const setBits = (m: number) => {
    setMode(m);
    setOctal(m.toString(8).padStart(3, "0"));
  };

  const apply = async () => {
    setErr(null);
    const m = parseOctal(octal);
    if (m === null) return setErr("Enter the mode as 3 or 4 octal digits, for example 755");
    const u = uid.trim() ? Number(uid) : null;
    const g = gid.trim() ? Number(gid) : null;
    if ((u !== null && !Number.isInteger(u)) || (g !== null && !Number.isInteger(g)) || (u ?? 0) < 0 || (g ?? 0) < 0) return setErr("User and group ids must be whole numbers");
    setBusy(true);
    try {
      if (m !== initial || recursive) await api.sftpChmod(sftpId, entry.path, m, recursive);
      if (u !== null || g !== null) await api.sftpChown(sftpId, entry.path, u, g);
      onClose(true);
    } catch (e) {
      setErr(toApiError(e).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={`Permissions: ${entry.name}`}
      onClose={() => onClose(false)}
      width={470}
      footer={
        <>
          {err && <span className="mr-auto self-center text-[12px] text-err">{err}</span>}
          <Button onClick={() => onClose(false)}>Cancel</Button>
          <Button variant="primary" onClick={apply} disabled={busy}>
            Apply
          </Button>
        </>
      }
    >
      <div className="space-y-4">
        <div className="font-mono text-[12px] text-dim select-text">
          {entry.path} · {entry.permissions} · owner {entry.owner ?? "?"}:{entry.group ?? "?"}
        </div>
        <table className="w-full text-[12.5px]">
          <thead>
            <tr className="text-left text-dim">
              <th className="py-1 font-medium" />
              {WHAT.map(([w]) => (
                <th key={w} className="py-1 font-medium">
                  {w}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {WHO.map((who, i) => {
              const shift = (2 - i) * 3;
              return (
                <tr key={who}>
                  <td className="py-1">{who}</td>
                  {WHAT.map(([w, bit]) => {
                    const mask = bit << shift;
                    return (
                      <td key={w} className="py-1">
                        <input type="checkbox" checked={(mode & mask) !== 0} onChange={(e) => setBits(e.target.checked ? mode | mask : mode & ~mask)} aria-label={`${who} ${w}`} />
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
        <Field label="Mode (octal)">
          <input
            className="field !w-32 font-mono"
            value={octal}
            onChange={(e) => {
              setOctal(e.target.value);
              const m = parseOctal(e.target.value);
              if (m !== null) setMode(m);
            }}
          />
        </Field>
        {entry.isDir && (
          <label className="flex items-center gap-2 text-[12.5px]">
            <input type="checkbox" checked={recursive} onChange={(e) => setRecursive(e.target.checked)} />
            Apply to everything inside this folder (symbolic links are skipped)
          </label>
        )}
        <div className="grid grid-cols-2 gap-3">
          <Field label="New owner user id (uid)" hint="Leave empty to keep. Usually requires root.">
            <input className="field font-mono" value={uid} onChange={(e) => setUid(e.target.value)} placeholder="1000" />
          </Field>
          <Field label="New group id (gid)">
            <input className="field font-mono" value={gid} onChange={(e) => setGid(e.target.value)} placeholder="1000" />
          </Field>
        </div>
      </div>
    </Modal>
  );
}
