import { FileInput, Lock, Monitor, Server, ShieldCheck, WifiOff } from "lucide-react";
import { Button, Modal } from "../components/ui";
import { useSettings } from "../store/settings";
import { newLocalTerminal, newSessionDialog } from "./actions";
import { api } from "../services/api";
import { toast } from "../store/ui";
import { useSessions } from "../store/sessions";
import { isWindows, secretStore } from "../utils/platform";

export function Onboarding() {
  const done = () => useSettings.getState().set("onboarded", true);
  const point = (icon: React.ReactNode, title: string, text: string) => (
    <div className="flex gap-3">
      <div className="mt-0.5 text-accent">{icon}</div>
      <div>
        <div className="font-semibold">{title}</div>
        <div className="text-[12.5px] text-dim">{text}</div>
      </div>
    </div>
  );
  return (
    <Modal title="Welcome to NexaTerm" onClose={done} width={560}>
      <div className="space-y-4 text-[13px]">
        <p className="text-dim">A free, open-source workspace for terminals, SSH, SFTP, tunnels and network diagnostics. A few things worth knowing:</p>
        {point(<WifiOff size={18} />, "Offline & private", "No account, no telemetry, no background network traffic. Everything is stored locally.")}
        {point(<Lock size={18} />, isWindows ? "Secrets stay in Windows" : "Secrets stay in your keyring", `Passwords and passphrases go to ${secretStore} — never into the database, exports or logs.`)}
        {point(<ShieldCheck size={18} />, "Host keys are verified", "You confirm each server's fingerprint on first connect; changed keys are blocked until you decide.")}
        <div className="grid grid-cols-3 gap-2 pt-2">
          <Button
            onClick={() => {
              done();
              newLocalTerminal();
            }}
          >
            <Monitor size={14} /> Open terminal
          </Button>
          <Button
            onClick={() => {
              done();
              newSessionDialog();
            }}
          >
            <Server size={14} /> Add SSH session
          </Button>
          <Button
            onClick={async () => {
              done();
              try {
                const n = await api.importOpenSshConfig(null);
                await useSessions.getState().refresh();
                toast("success", `Imported ${n} host(s) from ~/.ssh/config`);
              } catch (e) {
                toast("info", "No ~/.ssh/config found to import.");
              }
            }}
          >
            <FileInput size={14} /> Import ~/.ssh/config
          </Button>
        </div>
        <div className="flex justify-end">
          <Button variant="primary" onClick={done}>
            Get started
          </Button>
        </div>
      </div>
    </Modal>
  );
}
