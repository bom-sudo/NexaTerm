import { useEffect, useState } from "react";
import { Sidebar } from "../layouts/Sidebar";
import { Toolbar } from "../layouts/Toolbar";
import { TabBar } from "../layouts/TabBar";
import { Workspace } from "../layouts/Workspace";
import { StatusBar } from "../layouts/StatusBar";
import { MultiExecBar } from "../features/multiexec/MultiExecBar";
import { DialogHost, Toasts } from "../components/ui";
import { useSettings } from "../store/settings";
import { useSessions } from "../store/sessions";
import { useWorkspace } from "../store/workspace";
import { api, toApiError } from "../services/api";
import { toast } from "../store/ui";
import { installShortcuts, registerShortcutHandlers } from "./shortcuts";
import { newLocalTerminal, newSessionDialog } from "./actions";
import { Onboarding } from "./Onboarding";

export function App() {
  const theme = useSettings((s) => s.settings.theme);
  const loaded = useSettings((s) => s.loaded);
  const onboarded = useSettings((s) => s.settings.onboarded);
  const multi = useWorkspace((s) => s.multiExec.enabled);
  const [fatal, setFatal] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([useSettings.getState().load(), useSessions.getState().refresh(), useSessions.getState().loadShells()]).catch((e) => setFatal(toApiError(e).message));
    // Tell the user when the portable data folder was not writable and a fallback is in use.
    api.appInfo().then((i) => i.dataNotice && toast("warn", i.dataNotice, 0)).catch(() => {});
    registerShortcutHandlers({ newSession: () => newSessionDialog(), newTerminal: () => newLocalTerminal() });
    const off = installShortcuts();
    const noCtx = (e: MouseEvent) => {
      // Native browser context menu is never useful in the app; inputs keep it for copy/paste.
      const t = e.target as HTMLElement;
      if (!t.closest("input, textarea")) e.preventDefault();
    };
    window.addEventListener("contextmenu", noCtx);
    return () => {
      off();
      window.removeEventListener("contextmenu", noCtx);
    };
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  if (fatal) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="max-w-lg rounded-xl border border-err/40 bg-panel p-6">
          <div className="mb-2 text-[15px] font-semibold text-err">NexaTerm could not start</div>
          <div className="text-[13px] text-dim select-text">{fatal}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <Toolbar />
          {multi && <MultiExecBar />}
          <TabBar />
          <div className="min-h-0 flex-1 bg-bg">
            <Workspace />
          </div>
        </div>
      </div>
      <StatusBar />
      <DialogHost />
      <Toasts />
      {loaded && !onboarded && <Onboarding />}
    </div>
  );
}
