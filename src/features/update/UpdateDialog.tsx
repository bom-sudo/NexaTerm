import { useEffect, useState } from "react";
import { CheckCircle2, Download, ExternalLink, Loader2, RefreshCw, ShieldCheck } from "lucide-react";
import { api, toApiError, type UpdateInfo } from "../../services/api";
import { openDialog } from "../../store/ui";
import { useWorkspace } from "../../store/workspace";
import { Button, Modal } from "../../components/ui";

type Phase =
  | { kind: "checking" }
  | { kind: "error"; message: string }
  | { kind: "result"; info: UpdateInfo }
  | { kind: "downloading"; info: UpdateInfo; done: number; total: number }
  | { kind: "installing" };

const mb = (n: number) => `${(n / 1024 / 1024).toFixed(1)} MB`;

function UpdateBody({ close }: { close: () => void }) {
  const [phase, setPhase] = useState<Phase>({ kind: "checking" });

  const check = () => {
    setPhase({ kind: "checking" });
    api
      .updateCheck()
      .then((info) => setPhase({ kind: "result", info }))
      .catch((e) => setPhase({ kind: "error", message: toApiError(e).message }));
  };
  useEffect(check, []);

  const install = async (info: UpdateInfo) => {
    setPhase({ kind: "downloading", info, done: 0, total: info.installerSize ?? 0 });
    try {
      const path = await api.updateDownload((done, total) => setPhase({ kind: "downloading", info, done, total }));
      setPhase({ kind: "installing" });
      await api.updateInstall(path);
    } catch (e) {
      setPhase({ kind: "error", message: toApiError(e).message });
    }
  };

  const openSessions = useWorkspace.getState().allPanes().filter((x) => x.pane.status === "connected").length;

  let body: React.ReactNode;
  let footer: React.ReactNode = <Button onClick={close}>Close</Button>;
  switch (phase.kind) {
    case "checking":
      body = (
        <div className="flex items-center gap-2 text-dim">
          <Loader2 size={16} className="animate-spin" /> Checking GitHub for the latest version…
        </div>
      );
      break;
    case "error":
      body = <div className="text-err">{phase.message}</div>;
      footer = (
        <>
          <Button onClick={close}>Close</Button>
          <Button variant="primary" onClick={check}>
            <RefreshCw size={14} /> Try again
          </Button>
        </>
      );
      break;
    case "result": {
      const { info } = phase;
      const date = info.publishedAt ? new Date(info.publishedAt).toLocaleDateString() : null;
      if (!info.newer) {
        body = (
          <div className="flex items-start gap-3">
            <CheckCircle2 size={20} className="mt-0.5 shrink-0 text-ok" />
            <div>
              <div className="font-semibold text-fg">NexaTerm {info.current} is the latest version.</div>
              <div className="mt-1 text-dim">Checked the releases on GitHub{date ? `; the latest was published ${date}` : ""}.</div>
            </div>
          </div>
        );
        break;
      }
      body = (
        <div className="space-y-3">
          <div>
            <div className="text-[15px] font-semibold text-fg">NexaTerm {info.latest} is available</div>
            <div className="mt-1 text-dim">
              You have {info.current}
              {date ? ` · released ${date}` : ""}.{" "}
              <button type="button" className="text-accent hover:underline" onClick={() => void api.openReleasePage(info.pageUrl)}>
                What's new
              </button>
            </div>
          </div>
          {info.canInstall ? (
            <div className="flex items-start gap-2 rounded-lg border border-line bg-panel-2 p-3 text-[12px] text-dim">
              <ShieldCheck size={16} className="mt-0.5 shrink-0 text-ok" />
              <div>
                NexaTerm downloads the installer from GitHub{info.installerSize ? ` (${mb(info.installerSize)})` : ""}, checks it against the release's SHA-256 checksum,
                then closes and installs the update. It opens again by itself when done. Your sessions and settings are kept.
                {openSessions > 0 && <div className="mt-1 text-warn">{openSessions} open connection(s) will be closed.</div>}
              </div>
            </div>
          ) : (
            <div className="text-[12px] text-dim">Download the new version from the release page (portable and Linux builds are updated by replacing the files or with your package manager).</div>
          )}
        </div>
      );
      footer = (
        <>
          <Button onClick={close}>Later</Button>
          {info.canInstall ? (
            <Button variant="primary" onClick={() => void install(info)}>
              <Download size={14} /> Download and install
            </Button>
          ) : (
            <Button variant="primary" onClick={() => void api.openReleasePage(info.pageUrl)}>
              <ExternalLink size={14} /> Open download page
            </Button>
          )}
        </>
      );
      break;
    }
    case "downloading": {
      const pct = phase.total > 0 ? Math.min(100, (phase.done / phase.total) * 100) : 0;
      body = (
        <div className="space-y-2">
          <div className="text-dim">
            Downloading NexaTerm {phase.info.latest}… {phase.total > 0 && `${mb(phase.done)} / ${mb(phase.total)}`}
          </div>
          <div className="h-2 overflow-hidden rounded-full bg-panel-2">
            <div className="h-full rounded-full bg-accent transition-[width]" style={{ width: `${pct}%` }} />
          </div>
        </div>
      );
      footer = null;
      break;
    }
    case "installing":
      body = (
        <div className="flex items-center gap-2 text-dim">
          <Loader2 size={16} className="animate-spin" /> Checksum verified. Starting the installer; NexaTerm will close and reopen…
        </div>
      );
      footer = null;
      break;
  }

  return (
    <Modal title="Check for updates" onClose={phase.kind === "downloading" || phase.kind === "installing" ? () => {} : close} width={480} footer={footer}>
      <div className="text-[13px]">{body}</div>
    </Modal>
  );
}

export function openUpdateDialog() {
  return openDialog<void>((close) => <UpdateBody close={() => close()} />);
}
