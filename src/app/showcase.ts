// Development-only scene setup for README screenshots (NEXATERM_SHOWCASE=<scene>, debug builds).
// Loaded via dynamic import behind `import.meta.env.DEV`, so it is not part of release bundles.
// Every scene uses real features against the bundled mock SSH/SFTP server; nothing is simulated.
import { api } from "../services/api";
import { useSessions } from "../store/sessions";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";
import { useWorkspace, type PaneSpec } from "../store/workspace";
import { newSessionDialog, sessionSpec } from "./actions";

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
    case "highlight": {
      const sh = useSessions.getState().shells.find((s) => s.id === "powershell") ?? useSessions.getState().shells[0];
      const lines = [
        "2026-11-27 09:12:01 INFO  web-01 health check ok (10.20.0.11:443)",
        "2026-11-27 09:12:02 WARN  db-01 replication lag 4.2 s, timeout in 30 s",
        "2026-11-27 09:12:03 ERROR deploy failed: connection refused by 10.20.0.15:22",
        "2026-11-27 09:12:04 INFO  cache cluster status active, 3 nodes ok",
      ];
      const script = "function prompt { 'PS NexaTerm> ' }; " + lines.map((l) => `Write-Output '${l}'`).join("; ");
      ws.openTerminal({ kind: "local", program: sh.path, args: ["-NoLogo", "-NoExit", "-Command", script], cwd: null, label: "Highlighting", sessionId: null });
      return;
    }
    case "sftpdock": {
      ws.openTerminal(ssh("web-01"));
      await wait(2500);
      const tab = useWorkspace.getState().activeTab!;
      useWorkspace.getState().toggleSftpDock(tab);
      return;
    }
    case "twofa":
      ws.openTerminal(ssh("otp-login"));
      return;
    case "serial":
      void newSessionDialog({ protocol: "serial", name: "Core switch console" });
      return;
    case "macros":
      useUi.getState().setSidebarView("macros");
      ws.openTerminal(ssh("web-01"));
      return;
    case "update": {
      const { openUpdateDialog } = await import("../features/update/UpdateDialog");
      void openUpdateDialog();
      await wait(4000);
      return;
    }
    case "monitor": {
      ws.openTerminal(ssh("web-01"));
      ws.splitActive(localShell());
      // Focus the SSH pane so the status bar shows its server's resources.
      const tab = useWorkspace.getState().tabs.find((t) => t.id === useWorkspace.getState().activeTab);
      if (tab?.kind === "terminal") ws.setActivePane(tab.id, tab.panes[0].id);
      await wait(9000);
      return;
    }
    case "settings":
      return ws.openTool("settings", "Settings");
  }
}
