import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ChevronDown, ChevronUp, Loader2, PlugZap, RotateCw, X } from "lucide-react";
import { api } from "../../services/api";
import { useSettings, TERMINAL_THEMES } from "../../store/settings";
import { useWorkspace, type Pane } from "../../store/workspace";
import { describeError, withSshPrompts } from "../ssh/connect";
import { terminals } from "./registry";
import { Button } from "../../components/ui";
import { handleGlobalShortcut } from "../../app/shortcuts";
import { OutputHighlighter } from "./highlight";
import { useMacroRecorder } from "../../store/macros";
import { confirmDialog } from "../../components/ui";

interface Props {
  tabId: string;
  pane: Pane;
  active: boolean;
  visible: boolean;
}

export function TerminalPane({ tabId, pane, active, visible }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const idRef = useRef<string | undefined>(undefined);
  const hlRef = useRef(new OutputHighlighter({ enabled: useSettings.getState().settings.highlightOutput }));
  const settings = useSettings((s) => s.settings);
  const { updatePane, setActivePane, reconnectPane, closePane } = useWorkspace.getState();
  const multi = useWorkspace((s) => s.multiExec);
  const searchReq = useWorkspace((s) => s.terminalSearch);
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState("");

  // Create the xterm instance once per pane.
  useEffect(() => {
    const s = useSettings.getState().settings;
    const term = new Terminal({
      allowProposedApi: true,
      fontFamily: s.fontFamily,
      fontSize: s.fontSize,
      lineHeight: s.lineHeight,
      cursorStyle: s.cursorStyle,
      cursorBlink: s.cursorBlink,
      scrollback: s.scrollback,
      theme: TERMINAL_THEMES[s.terminalTheme] ?? TERMINAL_THEMES.midnight,
      macOptionIsMeta: true,
      rightClickSelectsWord: false,
    });
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    term.open(host.current!);
    termRef.current = term;
    fitRef.current = fit;
    searchRef.current = search;

    const safeFit = () => {
      if (!host.current || host.current.offsetWidth === 0) return;
      try {
        fit.fit();
      } catch {
        /* not visible yet */
      }
    };
    safeFit();
    terminals.set(pane.id, { term, search, fit: safeFit, terminalId: () => idRef.current });

    // Multi-line paste runs several commands at once: ask first (setting, on by default).
    const pasteText = async (text: string) => {
      if (!text) return;
      const lines = text.replace(/\r\n/g, "\n").replace(/\n+$/, "").split("\n");
      if (lines.length > 1 && useSettings.getState().settings.confirmMultilinePaste) {
        const ok = await confirmDialog({
          title: `Paste ${lines.length} lines?`,
          confirmText: "Paste",
          message: (
            <div className="space-y-2">
              <p>The text contains line breaks, so each line may run as a separate command.</p>
              <pre className="max-h-48 overflow-auto rounded-lg border border-line bg-elev p-2 font-mono text-[12px] whitespace-pre-wrap select-text">
                {lines.slice(0, 20).join("\n")}
                {lines.length > 20 ? `\n... (${lines.length - 20} more lines)` : ""}
              </pre>
            </div>
          ),
        });
        if (!ok) return term.focus();
      }
      term.paste(text);
      term.focus();
    };

    // OSC 7 (file://host/path) from the shell reports the working directory; the SFTP side
    // panel follows it. Shells that do not emit OSC 7 simply never update cwd.
    const osc7 = term.parser.registerOscHandler(7, (data) => {
      const m = /^file:\/\/[^/]*(\/.*)$/.exec(data);
      if (m) {
        let path = m[1];
        try {
          path = decodeURIComponent(path);
        } catch {
          /* keep raw */
        }
        useWorkspace.getState().updatePane(pane.id, { cwd: path });
      }
      return true;
    });

    // Keyboard: app shortcuts, clipboard. Returning false stops xterm from handling the key.
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      const passThrough = useSettings.getState().settings.shellKeysPassThrough;
      if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key === "w" || e.key === "t") && passThrough) return true; // shell gets Ctrl+W / Ctrl+T
      if (e.ctrlKey && e.shiftKey && e.code === "KeyC") {
        const sel = term.getSelection();
        if (sel) writeText(sel);
        return false;
      }
      if (e.ctrlKey && e.shiftKey && e.code === "KeyV") {
        e.preventDefault();
        readText().then((t) => void pasteText(t));
        return false;
      }
      if (e.ctrlKey && !e.shiftKey && e.key === "c" && term.hasSelection()) {
        writeText(term.getSelection());
        term.clearSelection();
        return false;
      }
      if (e.ctrlKey && !e.shiftKey && e.key === "v") {
        e.preventDefault();
        readText().then((t) => void pasteText(t));
        return false;
      }
      if (e.ctrlKey && !e.shiftKey && e.key === "f") {
        setSearchOpen(true);
        return false;
      }
      if (handleGlobalShortcut(e)) return false;
      return true;
    });

    term.onSelectionChange(() => {
      if (useSettings.getState().settings.copyOnSelect && term.hasSelection()) writeText(term.getSelection());
    });

    const dataSub = term.onData((d) => {
      useMacroRecorder.getState().record(pane.id, d);
      const tid = idRef.current;
      if (!tid) return;
      const me = useWorkspace.getState().multiExec;
      if (me.enabled && me.mirrorTyping && me.selected.includes(pane.id)) {
        const ids = useWorkspace
          .getState()
          .allPanes()
          .filter((x) => me.selected.includes(x.pane.id) && x.pane.terminalId && x.pane.status === "connected")
          .map((x) => x.pane.terminalId!);
        api.broadcast(ids.includes(tid) ? ids : [...ids, tid], d);
      } else {
        api.write(tid, d).catch(() => {});
      }
    });
    const resizeSub = term.onResize(({ cols, rows }) => {
      if (idRef.current) api.resize(idRef.current, cols, rows).catch(() => {});
    });

    let raf = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(safeFit);
    });
    ro.observe(host.current!);

    const onContext = (ev: MouseEvent) => {
      if (!useSettings.getState().settings.rightClickPaste) return;
      ev.preventDefault();
      if (term.hasSelection()) {
        writeText(term.getSelection());
        term.clearSelection();
      } else readText().then((t) => void pasteText(t));
    };
    host.current!.addEventListener("contextmenu", onContext);
    const hostEl = host.current!;

    return () => {
      ro.disconnect();
      hostEl.removeEventListener("contextmenu", onContext);
      dataSub.dispose();
      osc7.dispose();
      resizeSub.dispose();
      terminals.delete(pane.id);
      if (idRef.current) api.close(idRef.current).catch(() => {});
      idRef.current = undefined;
      term.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pane.id]);

  // Launch / relaunch the backend (initial start and every Reconnect).
  useEffect(() => {
    const term = termRef.current!;
    let disposed = false;
    hlRef.current = new OutputHighlighter({ enabled: useSettings.getState().settings.highlightOutput });
    const prev = idRef.current;
    if (prev) api.close(prev).catch(() => {});
    idRef.current = undefined;
    if (pane.generation > 0) term.write("\r\n\x1b[90m── reconnecting ──\x1b[0m\r\n");

    const handlers = {
      onData: (b: Uint8Array) => !disposed && term.write(hlRef.current.process(b)),
      onEvent: (ev: { type: string; code?: number | null; reason?: string | null; state?: string; message?: string | null }) => {
        if (disposed) return;
        if (ev.type === "exit") {
          const lost = !!ev.reason;
          term.write(`\r\n\x1b[${lost ? "31" : "90"}m[${lost ? ev.reason : `process exited${ev.code != null ? ` with code ${ev.code}` : ""}`}]\x1b[0m\r\n`);
          updatePane(pane.id, { status: lost ? "lost" : "exited" });
        } else if (ev.type === "status" && ev.state === "lost") {
          updatePane(pane.id, { status: "lost", error: ev.message ?? "Connection lost" });
        }
      },
    };

    (async () => {
      fitRef.current && terminals.get(pane.id)?.fit();
      const { cols, rows } = term;
      let started = performance.now();
      try {
        if (pane.spec.kind === "session") {
          const spec = pane.spec;
          term.write(`\x1b[90mOpening ${spec.protocol} session ${spec.label}...\x1b[0m\r\n`);
          const id = await api.openSession({ sessionId: spec.sessionId, cols: term.cols, rows: term.rows }, handlers);
          if (disposed) return void api.close(id);
          idRef.current = id;
          updatePane(pane.id, { terminalId: id, status: "connected", error: undefined });
        } else if (pane.spec.kind === "local") {
          const spec = pane.spec;
          const id = await api.openLocal({ program: spec.program, args: spec.args, cwd: spec.cwd, label: spec.label, sessionId: spec.sessionId, cols, rows }, handlers);
          if (disposed) return void api.close(id);
          idRef.current = id;
          updatePane(pane.id, { terminalId: id, status: "connected", error: undefined });
        } else {
          const sessionId = pane.spec.sessionId;
          term.write(`\x1b[90mConnecting to ${pane.spec.label}…\x1b[0m\r\n`);
          const r = await withSshPrompts(sessionId, (s) => {
            started = performance.now(); // time only the final attempt, not dialogs
            return api.openSsh({ sessionId, ...s, cols: term.cols, rows: term.rows }, handlers);
          });
          if (disposed) {
            if (r) api.close(r.terminalId);
            return;
          }
          if (!r) {
            updatePane(pane.id, { status: "error", error: "Connection cancelled" });
            term.write("\x1b[33mConnection cancelled.\x1b[0m\r\n");
            return;
          }
          idRef.current = r.terminalId;
          updatePane(pane.id, { terminalId: r.terminalId, status: "connected", host: r.host, username: r.username, connectMs: Math.round(performance.now() - started), error: undefined });
          // Ensure the server has our real size even if the window changed while connecting.
          api.resize(r.terminalId, term.cols, term.rows).catch(() => {});
        }
        if (active) term.focus();
      } catch (e) {
        if (disposed) return;
        const msg = describeError(e);
        term.write(`\r\n\x1b[31m${msg}\x1b[0m\r\n`);
        updatePane(pane.id, { status: "error", error: msg });
      }
    })();
    return () => {
      disposed = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pane.id, pane.generation]);

  // Apply settings changes live.
  useEffect(() => {
    const t = termRef.current;
    if (!t) return;
    t.options.fontFamily = settings.fontFamily;
    t.options.fontSize = settings.fontSize;
    t.options.lineHeight = settings.lineHeight;
    t.options.cursorStyle = settings.cursorStyle;
    t.options.cursorBlink = settings.cursorBlink;
    t.options.scrollback = settings.scrollback;
    t.options.theme = TERMINAL_THEMES[settings.terminalTheme] ?? TERMINAL_THEMES.midnight;
    hlRef.current.setEnabled(settings.highlightOutput);
    terminals.get(pane.id)?.fit();
  }, [settings, pane.id]);

  useEffect(() => {
    if (visible && active) {
      terminals.get(pane.id)?.fit();
      termRef.current?.focus();
    }
  }, [visible, active, pane.id]);

  useEffect(() => {
    if (searchReq > 0 && active && visible) setSearchOpen(true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchReq]);

  const find = (dir: 1 | -1) => {
    const s = searchRef.current;
    if (!s || !query) return;
    const opts = { caseSensitive: false, decorations: { matchOverviewRuler: "#888", activeMatchColorOverviewRuler: "#38bdf8", matchBackground: "#1e3a5f", activeMatchBackground: "#0e7490" } };
    if (dir === 1) s.findNext(query, opts);
    else s.findPrevious(query, opts);
  };

  const selected = multi.enabled && multi.selected.includes(pane.id);
  const bg = (TERMINAL_THEMES[settings.terminalTheme] ?? TERMINAL_THEMES.midnight).background;

  return (
    <div
      className={`relative flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden rounded-lg border ${
        selected ? "border-warn" : active ? "border-accent/60" : "border-line"
      }`}
      style={{ background: bg }}
      onMouseDown={() => setActivePane(tabId, pane.id)}
    >
      {selected && <div className="absolute top-1 right-2 z-10 rounded bg-warn px-1.5 text-[10px] font-bold text-black">MULTI-EXEC</div>}
      {searchOpen && (
        <div className="absolute top-2 right-3 z-20 flex items-center gap-1 rounded-lg border border-line-strong bg-panel p-1 shadow-xl">
          <input
            autoFocus
            className="field !w-56 !py-1"
            placeholder="Search terminal…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") find(e.shiftKey ? -1 : 1);
              if (e.key === "Escape") {
                setSearchOpen(false);
                searchRef.current?.clearDecorations();
                termRef.current?.focus();
              }
            }}
          />
          <button className="p-1 text-dim hover:text-fg" title="Previous (Shift+Enter)" onClick={() => find(-1)}>
            <ChevronUp size={15} />
          </button>
          <button className="p-1 text-dim hover:text-fg" title="Next (Enter)" onClick={() => find(1)}>
            <ChevronDown size={15} />
          </button>
          <button
            className="p-1 text-dim hover:text-fg"
            title="Close (Esc)"
            onClick={() => {
              setSearchOpen(false);
              searchRef.current?.clearDecorations();
            }}
          >
            <X size={15} />
          </button>
        </div>
      )}
      <div ref={host} className="min-h-0 flex-1" />
      {pane.status === "connecting" && pane.spec.kind === "ssh" && (
        <div className="pointer-events-none absolute bottom-3 left-3 flex items-center gap-2 rounded-md bg-panel/90 px-2 py-1 text-[12px] text-dim">
          <Loader2 size={14} className="animate-spin" /> Connecting…
        </div>
      )}
      {(pane.status === "error" || pane.status === "lost" || pane.status === "exited") && (
        <div className="absolute right-3 bottom-3 flex items-center gap-2 rounded-lg border border-line-strong bg-panel px-3 py-2 shadow-xl">
          <PlugZap size={15} className={pane.status === "exited" ? "text-dim" : "text-err"} />
          <span className="max-w-[320px] truncate text-[12px] text-dim" title={pane.error}>
            {pane.status === "exited" ? "Session ended" : pane.status === "lost" ? "Connection lost" : pane.error}
          </span>
          <Button variant="primary" className="!px-2 !py-1" onClick={() => reconnectPane(pane.id)}>
            <RotateCw size={13} /> {pane.spec.kind === "ssh" ? "Reconnect" : "Restart"}
          </Button>
          <Button variant="ghost" className="!px-2 !py-1" onClick={() => closePane(tabId, pane.id)}>
            Close
          </Button>
        </div>
      )}
    </div>
  );
}
