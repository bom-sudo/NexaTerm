import { useEffect, useMemo, useRef, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Cable,
  Network,
  ChevronDown, ChevronRight, Columns2, Copy, Download, FileInput, Folder, FolderPlus, FolderSync, Monitor, Pencil, Play, Plus, Search, Server, Star, StarOff, Trash2, Upload,
} from "lucide-react";
import { api, toApiError } from "../../services/api";
import { useSessions } from "../../store/sessions";
import { openDialog, toast, useUi } from "../../store/ui";
import { useWorkspace } from "../../store/workspace";
import { Button, IconButton, Modal, confirmDialog, promptDialog, useContextMenu, type MenuItem } from "../../components/ui";
import { connectSession, editSessionDialog, newSessionDialog } from "../../app/actions";
import type { ImportReport, Session } from "../../types";
import { secretStoreShort } from "../../utils/platform";

type Sort = "name" | "recent" | "host";

/** Summarise an import: what came in and why entries were skipped. */
function reportImport(r: ImportReport, source: string) {
  const reasons = r.skipped.slice(0, 4).map(([n, why]) => `${n}: ${why}`).join("; ");
  toast(r.imported ? "success" : "warn", `Imported ${r.imported} session(s) from ${source}.${r.skipped.length ? ` Skipped ${r.skipped.length}${reasons ? ` (${reasons}${r.skipped.length > 4 ? "; ..." : ""})` : ""}.` : ""}`, r.skipped.length ? 0 : undefined);
}

function matches(s: Session, q: string) {
  if (!q) return true;
  const hay = `${s.name} ${s.host ?? ""} ${s.username ?? ""} ${s.tags.join(" ")}`.toLowerCase();
  return q.toLowerCase().split(/\s+/).every((w) => hay.includes(w));
}

function sortFn(sort: Sort) {
  return (a: Session, b: Session) =>
    sort === "recent" ? (b.lastConnectedAt ?? "").localeCompare(a.lastConnectedAt ?? "") : sort === "host" ? (a.host ?? "").localeCompare(b.host ?? "") : a.name.localeCompare(b.name);
}

export function SessionsPanel() {
  const { sessions, groups, refresh } = useSessions();
  const focusReq = useUi((s) => s.sessionSearchFocus);
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<Sort>("name");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const menu = useContextMenu();

  useEffect(() => {
    if (focusReq) searchRef.current?.focus();
  }, [focusReq]);

  const filtered = useMemo(() => sessions.filter((s) => matches(s, q)).sort(sortFn(sort)), [sessions, q, sort]);
  const favorites = filtered.filter((s) => s.favorite);
  const recent = useMemo(() => sessions.filter((s) => s.lastConnectedAt).sort(sortFn("recent")).slice(0, 5), [sessions]);

  const act = async (fn: () => Promise<unknown>, ok?: string) => {
    try {
      await fn();
      await refresh();
      if (ok) toast("success", ok);
    } catch (e) {
      toast("error", toApiError(e).message);
    }
  };

  const sessionMenu = (s: Session): MenuItem[] => [
    { label: "Connect", icon: <Play size={13} />, onClick: () => connectSession(s) },
    { label: "Connect in split pane", icon: <Columns2 size={13} />, onClick: () => connectSession(s, { split: true }) },
    ...(s.protocol === "ssh" ? [{ label: "Open SFTP browser", icon: <FolderSync size={13} />, onClick: () => useWorkspace.getState().openSftp({ sessionId: s.id, title: `SFTP · ${s.name}` }) }] : []),
    { separator: true, label: "", onClick: () => {} },
    { label: "Edit…", icon: <Pencil size={13} />, onClick: () => editSessionDialog(s) },
    { label: "Duplicate", icon: <Copy size={13} />, onClick: () => act(() => api.sessionDuplicate(s.id), "Session duplicated (secrets are not copied)") },
    { label: s.favorite ? "Remove from favorites" : "Add to favorites", icon: s.favorite ? <StarOff size={13} /> : <Star size={13} />, onClick: () => act(() => api.sessionSetFavorite(s.id, !s.favorite)) },
    { label: "Move to folder…", icon: <Folder size={13} />, onClick: () => moveDialog(s) },
    { separator: true, label: "", onClick: () => {} },
    {
      label: "Delete",
      icon: <Trash2 size={13} />,
      danger: true,
      onClick: async () => {
        if (await confirmDialog({ title: "Delete session", message: <>Delete <b>{s.name}</b>? Its saved password/passphrase is removed from {secretStoreShort} too.</>, confirmText: "Delete", danger: true }))
          act(() => api.sessionDelete(s.id), "Session deleted");
      },
    },
  ];

  const moveDialog = (s: Session) =>
    openDialog<null>((close) => (
      <Modal title={`Move "${s.name}"`} onClose={() => close(null)} width={380}>
        <div className="flex flex-col gap-1">
          {[{ id: null as string | null, name: "(no folder)" }, ...groups].map((g) => (
            <button
              key={g.id ?? "root"}
              className={`rounded-md px-3 py-2 text-left hover:bg-hover ${s.groupId === g.id ? "text-accent" : ""}`}
              onClick={() => {
                close(null);
                act(() => api.sessionMove(s.id, g.id));
              }}
            >
              {g.name}
            </button>
          ))}
        </div>
      </Modal>
    ));

  const groupMenu = (id: string, name: string): MenuItem[] => [
    { label: "New session here…", icon: <Plus size={13} />, onClick: () => newSessionDialog({ groupId: id }) },
    {
      label: "Rename…",
      icon: <Pencil size={13} />,
      onClick: async () => {
        const r = await promptDialog({ title: "Rename folder", label: "Folder name", initial: name });
        if (r?.value) act(() => api.groupRename(id, r.value));
      },
    },
    {
      label: "Delete folder",
      icon: <Trash2 size={13} />,
      danger: true,
      onClick: async () => {
        if (await confirmDialog({ title: "Delete folder", message: `Delete folder "${name}"? Sessions inside are kept and moved to the top level.`, confirmText: "Delete folder", danger: true }))
          act(() => api.groupDelete(id));
      },
    },
  ];

  const importMenu = (e: React.MouseEvent) =>
    menu.open(e, [
      {
        label: "Import OpenSSH config (~/.ssh/config)",
        icon: <FileInput size={13} />,
        onClick: () => act(async () => toast("success", `Imported ${await api.importOpenSshConfig(null)} host(s)`)),
      },
      {
        label: "Import OpenSSH config from file…",
        icon: <FileInput size={13} />,
        onClick: async () => {
          const p = await open({ title: "OpenSSH config file" });
          if (typeof p === "string") act(async () => toast("success", `Imported ${await api.importOpenSshConfig(p)} host(s)`));
        },
      },
      {
        label: "Import from MobaXterm (.mxtsessions / MobaXterm.ini)…",
        icon: <FileInput size={13} />,
        onClick: async () => {
          const p = await open({ title: "MobaXterm sessions file", filters: [{ name: "MobaXterm sessions", extensions: ["mxtsessions", "ini"] }] });
          if (typeof p === "string") act(async () => reportImport(await api.importMobaXterm(p), "MobaXterm"));
        },
      },
      {
        label: "Import from PuTTY (this PC)",
        icon: <FileInput size={13} />,
        onClick: () => act(async () => reportImport(await api.importPutty(null), "PuTTY")),
      },
      {
        label: "Import from PuTTY export (.reg)…",
        icon: <FileInput size={13} />,
        onClick: async () => {
          const p = await open({ title: "PuTTY registry export", filters: [{ name: "Registry file", extensions: ["reg"] }] });
          if (typeof p === "string") act(async () => reportImport(await api.importPutty(p), "PuTTY"));
        },
      },
      {
        label: "Import NexaTerm sessions (.json)…",
        icon: <Upload size={13} />,
        onClick: async () => {
          const p = await open({ title: "Import sessions", filters: [{ name: "NexaTerm sessions", extensions: ["json"] }] });
          if (typeof p === "string") act(async () => toast("success", `Imported ${await api.sessionsImport(p)} session(s)`));
        },
      },
      {
        label: "Export sessions (.json, no secrets)…",
        icon: <Download size={13} />,
        onClick: async () => {
          const p = await save({ title: "Export sessions", defaultPath: "nexaterm-sessions.json", filters: [{ name: "JSON", extensions: ["json"] }] });
          if (p) act(async () => toast("success", `Exported ${await api.sessionsExport(p)} session(s). Passwords are not included.`));
        },
      },
    ]);

  const row = (s: Session, indent = 0) => (
    <div
      key={s.id}
      draggable
      onDragStart={(e) => e.dataTransfer.setData("text/nexaterm-session", s.id)}
      onDoubleClick={() => connectSession(s)}
      onContextMenu={(e) => menu.open(e, sessionMenu(s))}
      className="group flex cursor-default items-center gap-2 rounded-md px-2 py-1 hover:bg-hover"
      style={{ paddingLeft: 8 + indent * 14 }}
      title={`${s.name}${s.host ? `\n${s.username}@${s.host}:${s.port}` : ""}${s.tags.length ? `\nTags: ${s.tags.join(", ")}` : ""}\nDouble-click to connect`}
    >
      {s.protocol === "ssh" ? (
        <Server size={14} className="shrink-0 text-accent" />
      ) : s.protocol === "telnet" ? (
        <Network size={14} className="shrink-0 text-warn" />
      ) : s.protocol === "serial" ? (
        <Cable size={14} className="shrink-0 text-warn" />
      ) : (
        <Monitor size={14} className="shrink-0 text-accent-2" />
      )}
      <div className="min-w-0 flex-1">
        <div className="truncate text-[12.5px]">{s.name}</div>
        {s.host && <div className="truncate text-[11px] text-faint">{s.username}@{s.host}{s.port !== 22 ? `:${s.port}` : ""}</div>}
      </div>
      {s.favorite && <Star size={11} className="shrink-0 fill-warn text-warn" />}
      <button className="hidden shrink-0 text-dim group-hover:block hover:text-accent" title="Connect" onClick={() => connectSession(s)}>
        <Play size={13} />
      </button>
    </div>
  );

  const section = (key: string, title: React.ReactNode, items: Session[], opts: { groupId?: string; menu?: MenuItem[]; indent?: number } = {}) => (
    <div
      key={key}
      onDragOver={(e) => {
        if (opts.groupId !== undefined && e.dataTransfer.types.includes("text/nexaterm-session")) {
          e.preventDefault();
          setDropTarget(key);
        }
      }}
      onDragLeave={() => setDropTarget(null)}
      onDrop={(e) => {
        setDropTarget(null);
        const id = e.dataTransfer.getData("text/nexaterm-session");
        if (id && opts.groupId !== undefined) act(() => api.sessionMove(id, opts.groupId || null));
      }}
      className={dropTarget === key ? "rounded-md bg-accent-soft" : ""}
    >
      <div
        className="flex cursor-default items-center gap-1 px-1 py-1 text-[11px] font-semibold tracking-wide text-dim uppercase hover:text-fg"
        onClick={() => setCollapsed((c) => ({ ...c, [key]: !c[key] }))}
        onContextMenu={opts.menu ? (e) => menu.open(e, opts.menu!) : undefined}
      >
        {collapsed[key] ? <ChevronRight size={13} /> : <ChevronDown size={13} />}
        {title}
        <span className="ml-auto font-normal text-faint">{items.length}</span>
      </div>
      {!collapsed[key] && items.map((s) => row(s, 1))}
    </div>
  );

  return (
    <div className="flex h-full flex-col">
      <div className="space-y-2 p-2">
        <div className="relative">
          <Search size={13} className="absolute top-2.5 left-2.5 text-faint" />
          <input ref={searchRef} className="field !pl-8" placeholder="Search sessions (Ctrl+Shift+F)" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <div className="flex items-center gap-1">
          <Button variant="primary" className="!px-2" onClick={() => newSessionDialog()}>
            <Plus size={14} /> New
          </Button>
          <IconButton
            title="New folder"
            onClick={async () => {
              const r = await promptDialog({ title: "New folder", label: "Folder name" });
              if (r?.value) act(() => api.groupCreate(r.value));
            }}
          >
            <FolderPlus size={15} />
          </IconButton>
          <IconButton title="Import / export" onClick={importMenu}>
            <FileInput size={15} />
          </IconButton>
          <select className="field ml-auto !w-auto !py-1 !text-[11.5px]" value={sort} onChange={(e) => setSort(e.target.value as Sort)} title="Sort sessions">
            <option value="name">Name</option>
            <option value="recent">Recent</option>
            <option value="host">Host</option>
          </select>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto px-1 pb-2">
        {sessions.length === 0 ? (
          <div className="p-4 text-center text-[12px] text-dim">
            No saved sessions yet.
            <br />
            Create one with <b>New</b>, or import your <code>~/.ssh/config</code>.
          </div>
        ) : (
          <>
            {!q && favorites.length > 0 && section("fav", <><Star size={11} /> Favorites</>, favorites)}
            {!q && recent.length > 0 && section("recent", "Recent", recent)}
            {groups.map((g) => section(`g:${g.id}`, <><Folder size={11} /> {g.name}</>, filtered.filter((s) => s.groupId === g.id), { groupId: g.id, menu: groupMenu(g.id, g.name) }))}
            {section("root", groups.length ? "Ungrouped" : "Sessions", filtered.filter((s) => !s.groupId || !groups.some((g) => g.id === s.groupId)), { groupId: "" })}
            {q && filtered.length === 0 && <div className="p-4 text-center text-[12px] text-dim">No sessions match “{q}”.</div>}
          </>
        )}
      </div>
      {menu.node}
    </div>
  );
}
