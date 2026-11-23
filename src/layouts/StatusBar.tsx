import { useEffect, useState } from "react";
import { useWorkspace } from "../store/workspace";
import { api } from "../services/api";
import type { AppInfo } from "../types";

export function StatusBar() {
  const ws = useWorkspace();
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => {
    api.appInfo().then(setInfo).catch(() => {});
  }, []);
  const pane = ws.activePane();
  const all = ws.allPanes();
  const connected = all.filter((x) => x.pane.status === "connected");
  const sshCount = connected.filter((x) => x.pane.spec.kind === "ssh").length;

  const stateColor = !pane ? "bg-faint" : pane.status === "connected" ? "bg-ok" : pane.status === "connecting" ? "bg-warn" : pane.status === "exited" ? "bg-faint" : "bg-err";
  const stateText = !pane ? "No active terminal" : { connected: "Connected", connecting: "Connecting…", exited: "Exited", lost: "Connection lost", error: "Error" }[pane.status];

  const item = (label: string, value: React.ReactNode, title?: string) => (
    <div className="flex items-center gap-1 border-l border-line px-2.5" title={title}>
      <span className="text-faint">{label}</span>
      <span className="text-dim">{value}</span>
    </div>
  );

  return (
    <div className="flex h-6 shrink-0 items-center border-t border-line bg-elev text-[11px]">
      <div className="flex items-center gap-1.5 px-2.5">
        <span className={`h-2 w-2 rounded-full ${stateColor}`} />
        <span className="text-dim">{stateText}</span>
      </div>
      {pane && pane.spec.kind === "ssh" && (
        <>
          {item("Host", pane.host ?? "—")}
          {item("User", pane.username ?? "—")}
          {pane.connectMs != null && item("Connect", `${pane.connectMs} ms`, "Time to establish and authenticate the SSH connection")}
          {pane.status === "connected" && item("SSH", "host key verified")}
        </>
      )}
      {pane && pane.spec.kind === "local" && item("Shell", pane.spec.label)}
      {pane?.logging && item("Logging", "on", pane.logging)}
      {ws.multiExec.enabled && <div className="border-l border-line bg-warn/20 px-2.5 font-semibold text-warn">MULTI-EXEC ({ws.multiExec.selected.length})</div>}
      <div className="flex-1" />
      {item("Sessions", `${connected.length} active · ${sshCount} SSH`)}
      {item("Encoding", "UTF-8")}
      {info && item("", `NexaTerm ${info.version}${info.portable ? " (portable)" : ""}`)}
    </div>
  );
}
