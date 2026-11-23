// Orchestrates SSH connection attempts with the interactive steps a user may need:
// host-key verification, password / passphrase prompts, and retry after auth failure.
import { useState } from "react";
import { ShieldAlert, ShieldQuestion } from "lucide-react";
import { api, ApiError, toApiError } from "../../services/api";
import { openDialog } from "../../store/ui";
import { Button, Modal, promptDialog } from "../../components/ui";
import type { HostKeyInfo } from "../../types";
import { useSessions } from "../../store/sessions";

export interface SshSecrets {
  password?: string;
  passphrase?: string;
  saveSecret?: boolean;
}

function HostKeyBody({ info, changed, close }: { info: HostKeyInfo; changed: boolean; close: (v: boolean) => void }) {
  const [ack, setAck] = useState(false);
  return (
    <Modal
      title={
        <span className="flex items-center gap-2">
          {changed ? <ShieldAlert className="text-err" size={18} /> : <ShieldQuestion className="text-warn" size={18} />}
          {changed ? "HOST KEY HAS CHANGED" : "Verify host identity"}
        </span>
      }
      onClose={() => close(false)}
      width={600}
      footer={
        <>
          <Button onClick={() => close(false)} autoFocus>
            {changed ? "Disconnect (recommended)" : "Cancel"}
          </Button>
          <Button variant={changed ? "danger" : "primary"} disabled={changed && !ack} onClick={() => close(true)}>
            {changed ? "Replace stored key and connect" : "Trust and connect"}
          </Button>
        </>
      }
    >
      <div className="space-y-3 text-[13px] leading-relaxed">
        {changed ? (
          <p className="rounded-lg border border-err/40 bg-err/10 p-3 text-fg">
            The server <b>{info.host}:{info.port}</b> presented a different host key than the one stored on this computer. This can mean the
            server was reinstalled — or that someone is intercepting the connection (man-in-the-middle). Verify the new fingerprint with the
            server administrator through a separate channel before continuing.
          </p>
        ) : (
          <p>
            This is the first connection to <b>{info.host}:{info.port}</b>. NexaTerm cannot verify the server yet. Compare the fingerprint below
            with the one published by the server administrator (e.g. <code className="font-mono">ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub</code>).
          </p>
        )}
        <div className="rounded-lg border border-line bg-elev p-3 font-mono text-[12px] select-text">
          <div className="text-dim">Key type</div>
          <div className="mb-2">{info.keyType}</div>
          {changed && info.previousFingerprint && (
            <>
              <div className="text-dim">Stored fingerprint</div>
              <div className="mb-2 break-all line-through opacity-70">{info.previousFingerprint}</div>
            </>
          )}
          <div className="text-dim">{changed ? "New fingerprint" : "Fingerprint"}</div>
          <div className="break-all text-accent">{info.fingerprint}</div>
        </div>
        {changed && (
          <label className="flex items-center gap-2 text-[12.5px]">
            <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />I verified the new fingerprint with the server administrator.
          </label>
        )}
      </div>
    </Modal>
  );
}

export function askHostKey(info: HostKeyInfo, changed: boolean): Promise<boolean> {
  return openDialog<boolean>((close) => <HostKeyBody info={info} changed={changed} close={close} />);
}

/**
 * Run `attempt` until it succeeds, resolving interactive requirements in between.
 * Returns null if the user cancelled.
 */
export async function withSshPrompts<T>(sessionId: string, attempt: (s: SshSecrets) => Promise<T>): Promise<T | null> {
  const session = useSessions.getState().byId(sessionId);
  const label = session ? `${session.username ?? ""}@${session.host ?? ""}` : "server";
  let secrets: SshSecrets = {};
  let authFailures = 0;
  for (let step = 0; step < 10; step++) {
    try {
      return await attempt(secrets);
    } catch (raw) {
      const e = toApiError(raw);
      if ((e.kind === "hostKeyUnknown" || e.kind === "hostKeyChanged") && e.details) {
        const ok = await askHostKey(e.details, e.kind === "hostKeyChanged");
        if (!ok) return null;
        await api.knownHostTrust(e.details.host, e.details.port, e.details.publicKey);
        continue;
      }
      if (e.kind === "secretRequired") {
        const kind = e.message === "passphrase" ? "passphrase" : "password";
        const r = await promptDialog({
          title: kind === "password" ? `Password for ${label}` : "Private key passphrase",
          label: kind === "password" ? "Password" : `Passphrase for ${session?.keyPath ?? "key"}`,
          secret: true,
          checkbox: "Save in Windows Credential Manager",
        });
        if (!r) return null;
        secrets = { ...secrets, [kind]: r.value, saveSecret: r.checked };
        continue;
      }
      const interactive = session && (session.authMethod === "password" || session.authMethod === "keyboardInteractive" || session.authMethod === "key");
      if (e.kind === "authFailed" && interactive && authFailures < 3 && !e.message.includes("jump host")) {
        authFailures++;
        const kind = session!.authMethod === "key" ? "passphrase" : "password";
        if (kind === "passphrase" && !e.message.includes("passphrase")) throw e; // key rejected by server: re-typing won't help
        const r = await promptDialog({
          title: `Authentication failed — try again`,
          label: kind === "password" ? `Password for ${label}` : "Private key passphrase",
          secret: true,
          checkbox: "Save in Windows Credential Manager",
        });
        if (!r) return null;
        secrets = { ...secrets, [kind]: r.value, saveSecret: r.checked };
        continue;
      }
      throw e;
    }
  }
  throw new ApiError({ kind: "other", message: "Too many attempts", details: null });
}

export function describeError(e: unknown): string {
  const err = toApiError(e);
  switch (err.kind) {
    case "timeout":
      return "Connection timed out. Check the host name, port, VPN and firewall.";
    case "authFailed":
      return `Authentication failed: ${err.message.replace(/^Authentication failed: /, "")}`;
    default:
      return err.message;
  }
}
