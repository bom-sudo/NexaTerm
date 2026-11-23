import { ArrowDownToLine, Columns2, Eraser, FileDown, FolderSync, Grid2x2, Minus, Plus, Radio, Rows2, Search, Settings, SquarePlus, SquareTerminal, CircleDot, Zap, ChevronDown, RotateCcw, TextSelect } from "lucide-react";
import { useWorkspace } from "../store/workspace";
import { useSessions } from "../store/sessions";
import { IconButton, useContextMenu, type MenuItem } from "../components/ui";
import { newLocalTerminal, newSessionDialog, openSftpForActivePane, quickConnectDialog, terminalActions } from "../app/actions";

function Sep() {
  return <div className="mx-1 h-5 w-px bg-line" />;
}

export function Toolbar() {
  const ws = useWorkspace();
  const shells = useSessions((s) => s.shells);
  const active = ws.tabs.find((t) => t.id === ws.activeTab);
  const term = active?.kind === "terminal" ? active : undefined;
  const pane = ws.activePane();
  const menu = useContextMenu();

  return (
    <div className="flex h-10 shrink-0 items-center gap-0.5 border-b border-line bg-panel px-2">
      <IconButton title="New session (Ctrl+Shift+N)" onClick={() => newSessionDialog()}>
        <SquarePlus size={16} />
      </IconButton>
      <IconButton title="Quick connect" onClick={() => quickConnectDialog()}>
        <Zap size={16} />
      </IconButton>
      <div className="flex items-center">
        <IconButton title="New local terminal (Ctrl+T)" onClick={() => newLocalTerminal()}>
          <SquareTerminal size={16} />
        </IconButton>
        <button
          className="h-7 rounded-md px-0.5 text-dim hover:bg-hover hover:text-fg"
          title="Choose shell"
          onClick={(e) =>
            menu.open(e, [
              ...shells.map((s): MenuItem => ({ label: s.name, onClick: () => newLocalTerminal({ shell: s }) })),
              ...(term
                ? [{ label: "", separator: true, onClick: () => {} } as MenuItem, ...shells.map((s): MenuItem => ({ label: `Split: ${s.name}`, onClick: () => newLocalTerminal({ shell: s, split: true }) }))]
                : []),
            ])
          }
        >
          <ChevronDown size={12} />
        </button>
      </div>
      <Sep />
      <IconButton title="Split: add a local terminal pane to this tab" onClick={() => newLocalTerminal({ split: true })}>
        <Columns2 size={16} />
      </IconButton>
      <IconButton title="Layout: side by side" disabled={!term || term.panes.length !== 2} active={term?.panes.length === 2 && term.layout === "columns"} onClick={() => term && ws.setLayout(term.id, "columns")}>
        <Columns2 size={16} className="opacity-70" />
      </IconButton>
      <IconButton title="Layout: stacked" disabled={!term || term.panes.length !== 2} active={term?.panes.length === 2 && term.layout === "rows"} onClick={() => term && ws.setLayout(term.id, "rows")}>
        <Rows2 size={16} />
      </IconButton>
      <IconButton title="Four-pane grid: add panes until the tab has 4" disabled={!term || term.panes.length >= 4} onClick={() => {
        if (!term) return;
        for (let i = term.panes.length; i < 4; i++) newLocalTerminal({ split: true });
      }}>
        <Grid2x2 size={16} />
      </IconButton>
      <Sep />
      <IconButton title="Multi-execution: send input to several terminals" active={ws.multiExec.enabled} onClick={() => ws.setMultiExec({ enabled: !ws.multiExec.enabled })}>
        <Radio size={16} />
      </IconButton>
      <IconButton title="Open SFTP for the active SSH terminal" disabled={pane?.spec.kind !== "ssh"} onClick={openSftpForActivePane}>
        <FolderSync size={16} />
      </IconButton>
      <Sep />
      <IconButton title="Search terminal (Ctrl+F)" disabled={!pane} onClick={() => ws.requestTerminalSearch()}>
        <Search size={16} />
      </IconButton>
      <IconButton title="Clear terminal" disabled={!pane} onClick={terminalActions.clear}>
        <Eraser size={16} />
      </IconButton>
      <IconButton title="Reset terminal" disabled={!pane} onClick={terminalActions.reset}>
        <RotateCcw size={16} />
      </IconButton>
      <IconButton title="Select all" disabled={!pane} onClick={terminalActions.selectAll}>
        <TextSelect size={16} />
      </IconButton>
      <IconButton title="Scroll to bottom" disabled={!pane} onClick={terminalActions.scrollBottom}>
        <ArrowDownToLine size={16} />
      </IconButton>
      <IconButton title="Export terminal output to a file" disabled={!pane} onClick={terminalActions.exportOutput}>
        <FileDown size={16} />
      </IconButton>
      <IconButton title={pane?.logging ? `Stop logging (${pane.logging})` : "Start session logging"} disabled={!pane?.terminalId} active={!!pane?.logging} onClick={terminalActions.toggleLogging}>
        <CircleDot size={16} className={pane?.logging ? "text-err" : ""} />
      </IconButton>
      <Sep />
      <IconButton title="Decrease font size (Ctrl+-)" onClick={() => terminalActions.fontSize(-1)}>
        <Minus size={16} />
      </IconButton>
      <IconButton title="Increase font size (Ctrl++)" onClick={() => terminalActions.fontSize(1)}>
        <Plus size={16} />
      </IconButton>
      <div className="flex-1" />
      <IconButton title="Settings" onClick={() => ws.openTool("settings", "Settings")}>
        <Settings size={16} />
      </IconButton>
      {menu.node}
    </div>
  );
}
