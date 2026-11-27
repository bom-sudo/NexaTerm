// Typed wrappers around Tauri IPC. The UI never talks to the network or disk directly.
import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AppErrorPayload,
  AppInfo,
  HistoryEntry,
  KeyInfo,
  KnownHost,
  RemoteEntry,
  Session,
  SessionGroup,
  SessionInput,
  ShellProfile,
  FolderTransfer,
  ImportReport,
  MacroProfile,
  MacroStep,
  PortInfo,
  XStatus,
  Snippet,
  SshKeyRecord,
  TermEvent,
  TunnelProfile,
  TunnelStatus,
} from "../types";

export class ApiError extends Error {
  kind: AppErrorPayload["kind"];
  details: AppErrorPayload["details"];
  constructor(p: AppErrorPayload) {
    super(p.message);
    this.kind = p.kind;
    this.details = p.details;
  }
}

export function toApiError(e: unknown): ApiError {
  if (e instanceof ApiError) return e;
  if (e && typeof e === "object" && "kind" in e && "message" in e) return new ApiError(e as AppErrorPayload);
  return new ApiError({ kind: "other", message: String(e), details: null });
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toApiError(e);
  }
}

export interface TermChannels {
  onData: (bytes: Uint8Array) => void;
  onEvent: (ev: TermEvent) => void;
}

function termChannels(h: TermChannels) {
  const data = new Channel<ArrayBuffer | number[]>();
  data.onmessage = (m) => h.onData(m instanceof ArrayBuffer ? new Uint8Array(m) : Uint8Array.from(m));
  const ev = new Channel<TermEvent>();
  ev.onmessage = h.onEvent;
  return { onData: data, onEvent: ev };
}

export const api = {
  // app
  appInfo: () => call<AppInfo>("app_info"),
  settingsGet: () => call<Record<string, unknown>>("settings_get"),
  settingsSet: (key: string, value: unknown) => call<void>("settings_set", { key, value }),
  dbBackup: (path: string) => call<void>("db_backup", { path }),
  dbRestore: (path: string) => call<void>("db_restore", { path }),
  historyList: (limit = 50) => call<HistoryEntry[]>("history_list", { limit }),
  historyClear: () => call<void>("history_clear"),
  jobCancel: (jobId: string) => call<boolean>("job_cancel", { jobId }),

  // sessions
  sessionsList: () => call<Session[]>("sessions_list"),
  sessionCreate: (input: SessionInput) => call<Session>("session_create", { input }),
  sessionUpdate: (id: string, input: SessionInput) => call<Session>("session_update", { id, input }),
  sessionDelete: (id: string) => call<void>("session_delete", { id }),
  sessionDuplicate: (id: string) => call<Session>("session_duplicate", { id }),
  sessionSetFavorite: (id: string, favorite: boolean) => call<void>("session_set_favorite", { id, favorite }),
  sessionMove: (id: string, groupId: string | null) => call<void>("session_move", { id, groupId }),
  groupsList: () => call<SessionGroup[]>("groups_list"),
  groupCreate: (name: string, parentId: string | null = null) => call<SessionGroup>("group_create", { name, parentId }),
  groupRename: (id: string, name: string) => call<void>("group_rename", { id, name }),
  groupDelete: (id: string) => call<void>("group_delete", { id }),
  sessionsExport: (path: string) => call<number>("sessions_export", { path }),
  sessionsImport: (path: string) => call<number>("sessions_import", { path }),
  importOpenSshConfig: (path: string | null) => call<number>("import_openssh_config", { path }),
  importMobaXterm: (path: string) => call<ImportReport>("import_mobaxterm", { path }),
  importPutty: (path: string | null) => call<ImportReport>("import_putty", { path }),

  // terminals
  shellsList: () => call<ShellProfile[]>("shells_list"),
  openLocal: (
    spec: { program: string; args: string[]; cwd: string | null; label: string; sessionId: string | null; cols: number; rows: number },
    h: TermChannels,
  ) => call<string>("terminal_open_local", { spec, ...termChannels(h) }),
  openSsh: (
    req: { sessionId: string; password?: string; passphrase?: string; saveSecret?: boolean; cols: number; rows: number },
    h: TermChannels,
  ) => call<{ terminalId: string; host: string; username: string; label: string }>("terminal_open_ssh", { req, ...termChannels(h) }),
  openSession: (req: { sessionId: string; cols: number; rows: number }, h: TermChannels) =>
    call<string>("terminal_open_session", { ...req, ...termChannels(h) }),
  serialPorts: () => call<PortInfo[]>("serial_ports"),
  /** CPU / memory / disk / network counters of the server behind an SSH terminal. */
  sshMonitorSample: (terminalId: string) => call<import("../features/monitor/serverStats").RemoteStats>("ssh_monitor_sample", { terminalId }),
  sshPromptAnswer: (id: string, answers: string[] | null) => call<boolean>("ssh_prompt_answer", { id, answers }),
  write: (id: string, data: string) => call<void>("terminal_write", { id, data }),
  broadcast: (ids: string[], data: string) => call<[string, string | null][]>("terminal_broadcast", { ids, data }),
  resize: (id: string, cols: number, rows: number) => call<void>("terminal_resize", { id, cols, rows }),
  close: (id: string) => call<void>("terminal_close", { id }),
  logStart: (id: string, dir: string | null = null) => call<string>("terminal_log_start", { id, dir }),
  logStop: (id: string) => call<string | null>("terminal_log_stop", { id }),
  writeTextFile: (path: string, content: string) => call<void>("write_text_file", { path, content }),

  // known hosts
  knownHostsList: () => call<KnownHost[]>("known_hosts_list"),
  knownHostTrust: (host: string, port: number, publicKey: string) => call<string>("known_host_trust", { host, port, publicKey }),
  knownHostRemove: (host: string, port: number) => call<void>("known_host_remove", { host, port }),

  // sftp
  sftpOpen: (req: { terminalId?: string; sessionId?: string; password?: string; passphrase?: string }) =>
    call<{ sftpId: string; home: string; label: string }>("sftp_open", { req }),
  sftpClose: (sftpId: string) => call<void>("sftp_close", { sftpId }),
  sftpList: (sftpId: string, path: string) => call<RemoteEntry[]>("sftp_list", { sftpId, path }),
  sftpRealpath: (sftpId: string, path: string) => call<string>("sftp_realpath", { sftpId, path }),
  sftpExists: (sftpId: string, path: string) => call<boolean>("sftp_exists", { sftpId, path }),
  sftpMkdir: (sftpId: string, dir: string, name: string) => call<string>("sftp_mkdir", { sftpId, dir, name }),
  sftpCreateFile: (sftpId: string, dir: string, name: string) => call<string>("sftp_create_file", { sftpId, dir, name }),
  sftpRename: (sftpId: string, path: string, newName: string) => call<string>("sftp_rename", { sftpId, path, newName }),
  sftpDelete: (sftpId: string, path: string) => call<void>("sftp_delete", { sftpId, path }),
  sftpReadText: (sftpId: string, path: string) => call<{ content: string; mode: number | null }>("sftp_read_text", { sftpId, path }),
  sftpWriteText: (sftpId: string, path: string, content: string) => call<void>("sftp_write_text", { sftpId, path, content }),
  sftpUpload: (sftpId: string, localPath: string, remoteDir: string, overwrite: boolean, jobId: string, onProgress: (done: number, total: number) => void) => {
    const ch = new Channel<{ done: number; total: number }>();
    ch.onmessage = (p) => onProgress(p.done, p.total);
    return call<string>("sftp_upload", { sftpId, localPath, remoteDir, overwrite, jobId, onProgress: ch });
  },
  sftpDownload: (sftpId: string, remotePath: string, localDir: string, overwrite: boolean, jobId: string, onProgress: (done: number, total: number) => void) => {
    const ch = new Channel<{ done: number; total: number }>();
    ch.onmessage = (p) => onProgress(p.done, p.total);
    return call<string>("sftp_download", { sftpId, remotePath, localDir, overwrite, jobId, onProgress: ch });
  },
  sftpUploadDir: (sftpId: string, localPath: string, remoteDir: string, overwrite: boolean, jobId: string, onProgress: (done: number, total: number) => void) => {
    const ch = new Channel<{ done: number; total: number }>();
    ch.onmessage = (p) => onProgress(p.done, p.total);
    return call<FolderTransfer>("sftp_upload_dir", { sftpId, localPath, remoteDir, overwrite, jobId, onProgress: ch });
  },
  sftpDownloadDir: (sftpId: string, remotePath: string, localDir: string, overwrite: boolean, jobId: string, onProgress: (done: number, total: number) => void) => {
    const ch = new Channel<{ done: number; total: number }>();
    ch.onmessage = (p) => onProgress(p.done, p.total);
    return call<FolderTransfer>("sftp_download_dir", { sftpId, remotePath, localDir, overwrite, jobId, onProgress: ch });
  },
  sftpStat: (sftpId: string, path: string) => call<RemoteEntry>("sftp_stat", { sftpId, path }),
  sftpChmod: (sftpId: string, path: string, mode: number, recursive: boolean) => call<number>("sftp_chmod", { sftpId, path, mode, recursive }),
  sftpChown: (sftpId: string, path: string, uid: number | null, gid: number | null) => call<void>("sftp_chown", { sftpId, path, uid, gid }),
  localIsDir: (path: string) => call<boolean>("local_is_dir", { path }),
  localExists: (path: string) => call<boolean>("local_exists", { path }),
  downloadsDir: () => call<string>("downloads_dir"),

  // tunnels
  tunnelsList: () => call<TunnelProfile[]>("tunnels_list"),
  tunnelSave: (id: string | null, input: Omit<TunnelProfile, "id" | "autoStart">) => call<TunnelProfile>("tunnel_save", { id, input }),
  tunnelDelete: (id: string) => call<void>("tunnel_delete", { id }),
  tunnelStart: (id: string, password?: string, passphrase?: string) => call<void>("tunnel_start", { id, password, passphrase }),
  tunnelStop: (id: string) => call<void>("tunnel_stop", { id }),
  tunnelStatuses: () => call<TunnelStatus[]>("tunnel_statuses"),

  // network
  netRunTool: (tool: "ping" | "traceroute", host: string, count: number, jobId: string, onLine: (l: string) => void) => {
    const ch = new Channel<string>();
    ch.onmessage = onLine;
    return call<number>("net_run_tool", { tool, host, count, jobId, onLine: ch });
  },
  netDns: (host: string) => call<{ query: string; addresses: string[]; elapsedMs: number }>("net_dns", { host }),
  netReverse: (ip: string) => call<string>("net_reverse", { ip }),
  netTcpProbe: (host: string, port: number, timeoutMs?: number) =>
    call<{ host: string; port: number; open: boolean; address: string | null; elapsedMs: number; error: string | null }>("net_tcp_probe", { host, port, timeoutMs }),
  netInterfaces: () => call<{ name: string; address: string; isLoopback: boolean; isIpv6: boolean; netmask: string | null }[]>("net_interfaces"),
  netHostname: () => call<string>("net_hostname"),
  netPublicIp: () => call<string>("net_public_ip"),

  // keys
  keysList: () => call<SshKeyRecord[]>("keys_list"),
  keysDefaultDir: () => call<string>("keys_default_dir"),
  keyGenerate: (path: string, kind: "ed25519" | "rsa", bits: number | null, comment: string, passphrase: string | null) =>
    call<KeyInfo>("key_generate", { path, kind, bits, comment, passphrase }),
  keyImport: (path: string) => call<KeyInfo>("key_import", { path }),
  keyInspect: (path: string) => call<KeyInfo>("key_inspect", { path }),
  keyChangePassphrase: (path: string, old: string | null, next: string | null) => call<KeyInfo>("key_change_passphrase", { path, old, new: next }),
  keyForget: (id: string) => call<void>("key_forget", { id }),

  // X11
  x11Status: () => call<XStatus>("x11_status"),
  x11Start: (id: string) => call<void>("x11_start", { id }),

  // macros
  macrosList: () => call<MacroProfile[]>("macros_list"),
  macroSave: (id: string | null, name: string, steps: MacroStep[]) => call<string>("macro_save", { id, name, steps }),
  macroDelete: (id: string) => call<void>("macro_delete", { id }),

  // snippets
  snippetsList: () => call<Snippet[]>("snippets_list"),
  snippetSave: (id: string | null, input: { name: string; category: string; command: string; description: string | null }) => call<string>("snippet_save", { id, input }),
  snippetDelete: (id: string) => call<void>("snippet_delete", { id }),
};
