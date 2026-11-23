// Development-only scene setup for README screenshots (NEXATERM_SHOWCASE=<scene>, debug builds).
// Loaded via dynamic import behind `import.meta.env.DEV`, so it is not part of release bundles.
// Every scene uses real features against the bundled mock SSH/SFTP server; nothing is simulated.
import { api } from "../services/api";
import { useSessions } from "../store/sessions";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";
import { useWorkspace, type PaneSpec } from "../store/workspace";
import { sessionSpec } from "./actions";

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

function ssh(name: string): PaneSpec {
  const s = useSessions.getState().sessions.find((x) => x.name === name);
  if (!s) throw new Error(`showcase: session ${name} missing (run seed_showcase)`);
  return sessionSpec(s)!;
}

function localShell(): PaneSpec {
  const sh = useSessions.getState().shells.find((s) => s.id === "powershell") ?? useSessions.getState().shells[0];
  const script =
    "if ($env:NEXATERM_REPO) { Set-Location $env:NEXATERM_REPO }; function prompt { 'PS NexaTerm> ' }; " +
    "Write-Host 'Local PowerShell via Windows ConPTY' -ForegroundColor Cyan; " +
    "Get-ChildItem .\\src-tauri\\src -Directory | Format-Table Name, @{n='Files';e={(Get-ChildItem $_.FullName -File).Count}} -AutoSize";
  return { kind: "local", program: sh.path, args: ["-NoLogo", "-NoExit", "-Command", script], cwd: null, label: "PowerShell", sessionId: null };
}

async function workspace() {
  const ws = useWorkspace.getState();
  ws.openTerminal(localShell());
  ws.splitActive(ssh("web-01"));
  const first = useWorkspace.getState().activeTab!;
  const web = useSessions.getState().sessions.find((s) => s.name === "web-01")!;
  ws.openSftp({ sessionId: web.id, title: "SFTP · web-01" });
  ws.openTool("tunnels", "SSH Tunnels");
  await wait(300);
  ws.activate(first);
}

export async function runShowcase(scene: string) {
  await wait(800);
  const ws = useWorkspace.getState();
  switch (scene) {
    case "workspace":
      return workspace();
    case "light":
      useSettings.setState((s) => ({ settings: { ...s.settings, theme: "light", terminalTheme: "daylight" } }));
      return workspace();
    case "sftp": {
      const web = useSessions.getState().sessions.find((s) => s.name === "web-01")!;
      ws.openTerminal(ssh("web-01"));
      ws.openSftp({ sessionId: web.id, title: "SFTP · web-01" });
      return;
    }
    case "multiexec": {
      ws.openTerminal(ssh("web-01"));
      ws.splitActive(ssh("web-02"));
      ws.splitActive(ssh("db-01"));
      ws.splitActive(localShell());
      await wait(5000);
      const ids = useWorkspace.getState().allPanes().filter((p) => p.pane.spec.kind === "ssh" && p.pane.status === "connected").map((p) => p.pane.id);
      useWorkspace.getState().setMultiExec({ enabled: true, selected: ids });
      return;
    }
    case "tunnels": {
      ws.openTool("tunnels", "SSH Tunnels");
      const t = (await api.tunnelsList()).find((x) => x.kind === "dynamic");
      if (t) await api.tunnelStart(t.id).catch(() => {});
      return;
    }
    case "keys":
      return ws.openTool("keys", "SSH Keys");
    case "snippets":
      useUi.getState().setSidebarView("snippets");
      ws.openTerminal(ssh("web-01"));
      return;
    case "hostkey":
      ws.openTerminal(ssh("new-server"));
      return;
    case "settings":
      return ws.openTool("settings", "Settings");
  }
}
