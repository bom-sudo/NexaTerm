import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowLeft, ArrowRight, ArrowUp, Download, File, FilePlus, Folder, FolderPlus, Link2, Loader2, Pencil, RefreshCw, Trash2, Upload, X, Ban, FileEdit, Home } from "lucide-react";
import { api, toApiError } from "../../services/api";
import { toast } from "../../store/ui";
import { useWorkspace } from "../../store/workspace";
import { Badge, Button, EmptyState, IconButton, confirmDialog, promptDialog, useContextMenu } from "../../components/ui";
import { formatBytes, formatUnixTime, uid } from "../../utils/commands";
import { withSshPrompts } from "../ssh/connect";
import type { RemoteEntry } from "../../types";

interface Transfer {
  id: string;
  kind: "upload" | "download";
  name: string;
  done: number;
  total: number;
  state: "queued" | "running" | "done" | "failed" | "cancelled";
  error?: string;
  run: () => Promise<string>;
}

type SortKey = "name" | "size" | "modified";

export function SftpBrowser({ tabId, terminalId, sessionId }: { tabId: string; terminalId?: string; sessionId?: string }) {
  const [sftpId, setSftpId] = useState<string | null>(null);
  const [connErr, setConnErr] = useState<string | null>(null);
  const [path, setPath] = useState("/");
  const [pathInput, setPathInput] = useState("/");
  const [entries, setEntries] = useState<RemoteEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [listErr, setListErr] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; asc: boolean }>({ key: "name", asc: true });
  const [back, setBack] = useState<string[]>([]);
  const [fwd, setFwd] = useState<string[]>([]);
  const [transfers, setTransfersState] = useState<Transfer[]>([]);
  // Ref-backed queue: the pump must see the latest list synchronously.
  const queue = useRef<Transfer[]>([]);
  const setTransfers = (fn: (ts: Transfer[]) => Transfer[]) => {
    queue.current = fn(queue.current);
    setTransfersState(queue.current);
  };
  const [dragOver, setDragOver] = useState(false);
  const running = useRef(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const menu = useContextMenu();
  const isActive = useWorkspace((s) => s.activeTab === tabId);

  // Connect (reuse the terminal's SSH connection when available).
  const connect = useCallback(async () => {
    setConnErr(null);
    try {
      const r = terminalId && !sessionId
        ? await api.sftpOpen({ terminalId })
        : await withSshPrompts(sessionId!, (s) => api.sftpOpen({ terminalId, sessionId, password: s.password, passphrase: s.passphrase }));
      if (!r) return setConnErr("Connection cancelled");
      setSftpId(r.sftpId);
      setPath(r.home);
      setPathInput(r.home);
    } catch (e) {
      setConnErr(toApiError(e).message);
    }
  }, [terminalId, sessionId]);

  useEffect(() => {
    connect();
  }, [connect]);
  useEffect(() => () => void (sftpId && api.sftpClose(sftpId)), [sftpId]);

  const load = useCallback(
    async (p: string) => {
      if (!sftpId) return;
      setLoading(true);
      setListErr(null);
      try {
        setEntries(await api.sftpList(sftpId, p));
        setPathInput(p);
      } catch (e) {
        setListErr(toApiError(e).message);
      } finally {
        setLoading(false);
      }
    },
    [sftpId],
  );
  useEffect(() => {
    load(path);
  }, [path, load]);

  const navigate = (p: string) => {
    if (p === path) return load(p);
    setBack((b) => [...b, path]);
    setFwd([]);
    setSelected(null);
    setPath(p);
  };
  const parent = path === "/" ? "/" : path.replace(/\/[^/]+\/?$/, "") || "/";

  const visible = useMemo(() => {
    const f = entries.filter((e) => !filter || e.name.toLowerCase().includes(filter.toLowerCase()));
    const dir = sort.asc ? 1 : -1;
    return f.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      if (sort.key === "size") return (a.size - b.size) * dir;
      if (sort.key === "modified") return ((a.modified ?? 0) - (b.modified ?? 0)) * dir;
      return a.name.localeCompare(b.name) * dir;
    });
  }, [entries, filter, sort]);

  // ---------------- transfer queue (sequential, cancellable)
  const pump = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    try {
      for (;;) {
        const next = queue.current.find((t) => t.state === "queued");
        if (!next) break;
        setTransfers((ts) => ts.map((t) => (t.id === next.id ? { ...t, state: "running" } : t)));
        const t = next;
        try {
          await t.run();
          setTransfers((ts) => ts.map((x) => (x.id === t.id ? { ...x, state: "done", done: x.total || x.done } : x)));
        } catch (e) {
          const err = toApiError(e);
          setTransfers((ts) => ts.map((x) => (x.id === t.id ? { ...x, state: err.kind === "cancelled" ? "cancelled" : "failed", error: err.message } : x)));
        }
      }
    } finally {
      running.current = false;
      load(path);
    }
  }, [load, path]);

  const progress = (id: string) => (done: number, total: number) => setTransfers((ts) => ts.map((t) => (t.id === id ? { ...t, done, total } : t)));

  const enqueueUploads = async (files: string[]) => {
    if (!sftpId) return;
    const dir = path;
    for (const f of files) {
      const name = f.split(/[\\/]/).pop()!;
      const exists = entries.some((e) => e.name === name);
      let overwrite = false;
      if (exists) {
        overwrite = await confirmDialog({ title: "File exists", message: <>“{name}” already exists in {dir}. Overwrite it?</>, confirmText: "Overwrite", danger: true });
        if (!overwrite) continue;
      }
      const id = uid();
      setTransfers((ts) => [...ts, { id, kind: "upload", name, done: 0, total: 0, state: "queued", run: () => api.sftpUpload(sftpId, f, dir, overwrite, id, progress(id)) }]);
    }
    setTimeout(pump, 0);
  };

  const enqueueDownload = async (e: RemoteEntry) => {
    if (!sftpId) return;
    if (e.isDir) return toast("info", "Folder download is not supported yet — download files individually.");
    const dir = await open({ title: "Download to folder", directory: true, defaultPath: await api.downloadsDir() });
    if (typeof dir !== "string") return;
    const sep = dir.endsWith("\\") ? "" : "\\";
    let overwrite = false;
    if (await api.localExists(dir + sep + e.name)) {
      overwrite = await confirmDialog({ title: "File exists", message: <>“{e.name}” already exists in {dir}. Overwrite it?</>, confirmText: "Overwrite", danger: true });
      if (!overwrite) return;
    }
    const id = uid();
    setTransfers((ts) => [...ts, { id, kind: "download", name: e.name, done: 0, total: e.size, state: "queued", run: () => api.sftpDownload(sftpId, e.path, dir, overwrite, id, progress(id)) }]);
    setTimeout(pump, 0);
  };

  // Drag & drop from Windows Explorer (Tauri delivers native file paths).
  useEffect(() => {
    if (!isActive || !sftpId) return;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((ev) => {
        const p = ev.payload;
        if (p.type === "over" || p.type === "enter") setDragOver(true);
        else if (p.type === "leave") setDragOver(false);
        else if (p.type === "drop") {
          setDragOver(false);
          if (p.paths.length) enqueueUploads(p.paths);
        }
      })
      .then((u) => (unlisten = u));
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isActive, sftpId, path, entries]);

  const run = async (fn: () => Promise<unknown>, ok?: string) => {
    try {
      await fn();
      if (ok) toast("success", ok);
      load(path);
    } catch (e) {
      toast("error", toApiError(e).message);
    }
  };

  const del = async (e: RemoteEntry) => {
    const ok = await confirmDialog({
      title: `Delete ${e.isDir ? "folder" : "file"}`,
      danger: true,
      confirmText: "Delete permanently",
      message: (
        <>
          Permanently delete <b className="font-mono">{e.path}</b>
          {e.isDir ? " and everything inside it" : ""} on the server? This cannot be undone.
        </>
      ),
    });
    if (ok) run(() => api.sftpDelete(sftpId!, e.path), "Deleted");
  };

  const entryMenu = (e: RemoteEntry) => [
    ...(e.isDir ? [{ label: "Open", icon: <Folder size={13} />, onClick: () => navigate(e.path) }] : [{ label: "Edit", icon: <FileEdit size={13} />, onClick: () => useWorkspace.getState().openEditor(sftpId!, e.path) }]),
    { label: "Download…", icon: <Download size={13} />, onClick: () => enqueueDownload(e), disabled: e.isDir },
    {
      label: "Rename…",
      icon: <Pencil size={13} />,
      onClick: async () => {
        const r = await promptDialog({ title: "Rename", label: "New name", initial: e.name });
        if (r?.value && r.value !== e.name) run(() => api.sftpRename(sftpId!, e.path, r.value));
      },
    },
    { label: "Delete…", icon: <Trash2 size={13} />, danger: true, onClick: () => del(e) },
  ];

  if (connErr) {
    return (
      <EmptyState icon={<X size={32} />} title="SFTP connection failed">
        <div className="mb-3 select-text">{connErr}</div>
        <Button variant="primary" onClick={connect}>
          Retry
        </Button>
      </EmptyState>
    );
  }
  if (!sftpId) return <EmptyState icon={<Loader2 size={28} className="animate-spin" />} title="Opening SFTP session…" />;

  const sortHeader = (key: SortKey, label: string, cls = "") => (
    <th className={`cursor-pointer px-2 py-1.5 font-medium select-none hover:text-fg ${cls}`} onClick={() => setSort((s) => ({ key, asc: s.key === key ? !s.asc : true }))}>
      {label} {sort.key === key ? (sort.asc ? "▲" : "▼") : ""}
    </th>
  );

  const activeTransfers = transfers.filter((t) => t.state === "running" || t.state === "queued").length;

  return (
    <div ref={rootRef} className={`flex h-full flex-col rounded-lg border bg-panel ${dragOver ? "border-accent" : "border-line"}`}>
      <div className="flex items-center gap-1 border-b border-line p-2">
        <IconButton title="Back" disabled={!back.length} onClick={() => { const b = back[back.length - 1]; setBack(back.slice(0, -1)); setFwd([path, ...fwd]); setPath(b); }}>
          <ArrowLeft size={15} />
        </IconButton>
        <IconButton title="Forward" disabled={!fwd.length} onClick={() => { const f = fwd[0]; setFwd(fwd.slice(1)); setBack([...back, path]); setPath(f); }}>
          <ArrowRight size={15} />
        </IconButton>
        <IconButton title="Parent folder" disabled={path === "/"} onClick={() => navigate(parent)}>
          <ArrowUp size={15} />
        </IconButton>
        <IconButton title="Home folder" onClick={async () => navigate(await api.sftpRealpath(sftpId, "."))}>
          <Home size={15} />
        </IconButton>
        <form className="flex-1" onSubmit={(e) => { e.preventDefault(); navigate(pathInput.trim() || "/"); }}>
          <input className="field font-mono !py-1" value={pathInput} onChange={(e) => setPathInput(e.target.value)} />
        </form>
        <IconButton title="Refresh" onClick={() => load(path)}>
          <RefreshCw size={15} className={loading ? "animate-spin" : ""} />
        </IconButton>
        <input className="field !w-40 !py-1" placeholder="Filter…" value={filter} onChange={(e) => setFilter(e.target.value)} />
        <div className="mx-1 h-5 w-px bg-line" />
        <IconButton title="Upload files…" onClick={async () => { const f = await open({ title: "Upload files", multiple: true }); if (Array.isArray(f) && f.length) enqueueUploads(f); else if (typeof f === "string") enqueueUploads([f]); }}>
          <Upload size={15} />
        </IconButton>
        <IconButton title="Download selected" disabled={!selected} onClick={() => { const e = entries.find((x) => x.path === selected); if (e) enqueueDownload(e); }}>
          <Download size={15} />
        </IconButton>
        <IconButton title="New folder" onClick={async () => { const r = await promptDialog({ title: "New folder", label: "Folder name" }); if (r?.value) run(() => api.sftpMkdir(sftpId, path, r.value)); }}>
          <FolderPlus size={15} />
        </IconButton>
        <IconButton title="New file" onClick={async () => { const r = await promptDialog({ title: "New file", label: "File name" }); if (r?.value) run(() => api.sftpCreateFile(sftpId, path, r.value)); }}>
          <FilePlus size={15} />
        </IconButton>
      </div>

      <div className="min-h-0 flex-1 overflow-auto" onContextMenu={(e) => menu.open(e, [
        { label: "Refresh", onClick: () => load(path) },
        { label: "New folder…", onClick: async () => { const r = await promptDialog({ title: "New folder", label: "Folder name" }); if (r?.value) run(() => api.sftpMkdir(sftpId, path, r.value)); } },
        { label: "New file…", onClick: async () => { const r = await promptDialog({ title: "New file", label: "File name" }); if (r?.value) run(() => api.sftpCreateFile(sftpId, path, r.value)); } },
      ])}>
        {listErr ? (
          <div className="p-6 text-center text-[12.5px] text-err select-text">{listErr}</div>
        ) : (
          <table className="w-full border-collapse text-[12.5px]">
            <thead className="sticky top-0 z-10 bg-panel-2 text-left text-[11.5px] text-dim">
              <tr>
                {sortHeader("name", "Name", "w-full")}
                {sortHeader("size", "Size", "text-right whitespace-nowrap")}
                {sortHeader("modified", "Modified", "whitespace-nowrap")}
                <th className="px-2 py-1.5 font-medium">Permissions</th>
                <th className="px-2 py-1.5 font-medium">Owner</th>
              </tr>
            </thead>
            <tbody>
              {path !== "/" && (
                <tr className="hover:bg-hover" onDoubleClick={() => navigate(parent)}>
                  <td className="px-2 py-1 text-dim" colSpan={5}>
                    <span className="flex items-center gap-2"><Folder size={14} className="text-accent" /> ..</span>
                  </td>
                </tr>
              )}
              {visible.map((e) => (
                <tr
                  key={e.path}
                  className={`cursor-default ${selected === e.path ? "bg-accent-soft" : "hover:bg-hover"}`}
                  onClick={() => setSelected(e.path)}
                  onDoubleClick={() => (e.isDir ? navigate(e.path) : useWorkspace.getState().openEditor(sftpId, e.path))}
                  onContextMenu={(ev) => { ev.stopPropagation(); setSelected(e.path); menu.open(ev, entryMenu(e)); }}
                >
                  <td className="max-w-0 px-2 py-1">
                    <span className="flex items-center gap-2 truncate">
                      {e.isDir ? <Folder size={14} className="shrink-0 text-accent" /> : <File size={14} className="shrink-0 text-dim" />}
                      <span className="truncate">{e.name}</span>
                      {e.isSymlink && <Link2 size={12} className="shrink-0 text-faint" />}
                    </span>
                  </td>
                  <td className="px-2 py-1 text-right whitespace-nowrap text-dim">{e.isDir ? "" : formatBytes(e.size)}</td>
                  <td className="px-2 py-1 whitespace-nowrap text-dim">{formatUnixTime(e.modified)}</td>
                  <td className="px-2 py-1 font-mono text-[11.5px] whitespace-nowrap text-dim">{e.permissions}</td>
                  <td className="px-2 py-1 whitespace-nowrap text-dim">{e.owner}{e.group ? `:${e.group}` : ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {!listErr && !loading && visible.length === 0 && <div className="p-6 text-center text-[12px] text-dim">{filter ? "No files match the filter." : "This folder is empty. Drop files here to upload."}</div>}
      </div>

      <div className="border-t border-line">
        <div className="flex items-center gap-2 px-3 py-1.5 text-[11.5px] text-dim">
          <span>{visible.length} item(s)</span>
          <span className="flex-1" />
          <span>Transfers: {activeTransfers} active · {transfers.length} total</span>
          {transfers.some((t) => !["queued", "running"].includes(t.state)) && (
            <button className="text-accent hover:underline" onClick={() => setTransfers((ts) => ts.filter((t) => t.state === "queued" || t.state === "running"))}>
              Clear finished
            </button>
          )}
        </div>
        {transfers.length > 0 && (
          <div className="max-h-36 overflow-auto px-2 pb-2">
            {transfers.slice().reverse().map((t) => {
              const pct = t.total ? Math.min(100, Math.round((t.done / t.total) * 100)) : t.state === "done" ? 100 : 0;
              return (
                <div key={t.id} className="flex items-center gap-2 rounded px-1 py-0.5 text-[11.5px]">
                  {t.kind === "upload" ? <Upload size={12} /> : <Download size={12} />}
                  <span className="w-48 truncate" title={t.name}>{t.name}</span>
                  <div className="h-1.5 flex-1 overflow-hidden rounded bg-panel-2">
                    <div className={`h-full ${t.state === "failed" ? "bg-err" : t.state === "done" ? "bg-ok" : "bg-accent"}`} style={{ width: `${pct}%` }} />
                  </div>
                  <span className="w-36 text-right text-dim">{formatBytes(t.done)}{t.total ? ` / ${formatBytes(t.total)}` : ""}</span>
                  <Badge tone={t.state === "done" ? "ok" : t.state === "failed" ? "err" : t.state === "running" ? "accent" : "neutral"}>{t.state}</Badge>
                  {(t.state === "running" || t.state === "queued") && (
                    <button title="Cancel transfer" className="text-dim hover:text-err" onClick={() => (t.state === "queued" ? setTransfers((ts) => ts.map((x) => (x.id === t.id ? { ...x, state: "cancelled" } : x))) : api.jobCancel(t.id))}>
                      <Ban size={13} />
                    </button>
                  )}
                  {t.error && <span className="max-w-60 truncate text-err" title={t.error}>{t.error}</span>}
                </div>
              );
            })}
          </div>
        )}
      </div>
      {menu.node}
    </div>
  );
}
