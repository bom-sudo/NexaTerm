export type Protocol = "ssh" | "local" | "telnet" | "rdp" | "vnc" | "serial";
export type AuthMethod = "password" | "key" | "agent" | "keyboardInteractive";

export interface SessionOptions {
  connectTimeoutSecs?: number | null;
  keepaliveSecs?: number | null;
  termType?: string | null;
  startupDir?: string | null;
  startupCommand?: string | null;
  retries?: number | null;
  shellPath?: string | null;
  shellArgs?: string[] | null;
  cwd?: string | null;
  encoding?: string | null;
  compression?: boolean | null;
  agentForward?: boolean | null;
  x11Forward?: boolean | null;
  x11Display?: string | null;
  followTerminalFolder?: boolean | null;
  serial?: SerialSettings | null;
}

export interface SerialSettings {
  port: string;
  baud: number;
  dataBits: number;
  parity: "none" | "odd" | "even" | "mark" | "space";
  stopBits: "one" | "onePointFive" | "two";
  flow: "none" | "xonXoff" | "rtsCts";
}

export interface PortInfo {
  name: string;
  description: string | null;
}

export interface KbdPrompt {
  id: string;
  host: string;
  username: string;
  name: string;
  instructions: string;
  prompts: { text: string; echo: boolean }[];
}

export interface FolderTransfer {
  path: string;
  files: number;
  skipped: string[];
}

export interface ImportReport {
  imported: number;
  skipped: [string, string][];
}

export type MacroStep = { kind: "text"; text: string } | { kind: "wait"; ms: number };

export interface MacroProfile {
  id: string;
  name: string;
  steps: MacroStep[];
  createdAt: string;
}

export interface XStatus {
  running: boolean;
  installed: { id: string; name: string; path: string }[];
}

export interface Session {
  id: string;
  name: string;
  groupId: string | null;
  protocol: Protocol;
  host: string | null;
  port: number | null;
  username: string | null;
  authMethod: AuthMethod | null;
  keyPath: string | null;
  hasPassword: boolean;
  hasPassphrase: boolean;
  jumpSessionId: string | null;
  options: SessionOptions;
  favorite: boolean;
  sortOrder: number;
  notes: string | null;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  lastConnectedAt: string | null;
}

export interface SessionInput {
  name: string;
  groupId: string | null;
  protocol: Protocol;
  host: string | null;
  port: number | null;
  username: string | null;
  authMethod: AuthMethod | null;
  keyPath: string | null;
  jumpSessionId: string | null;
  options: SessionOptions;
  favorite: boolean;
  notes: string | null;
  tags: string[];
  /** undefined = unchanged, "" = delete, value = store in Credential Manager */
  password?: string;
  passphrase?: string;
}

export interface SessionGroup {
  id: string;
  name: string;
  parentId: string | null;
  sortOrder: number;
}

export interface ShellProfile {
  id: string;
  name: string;
  path: string;
  args: string[];
  kind: string;
}

export interface HostKeyInfo {
  host: string;
  port: number;
  keyType: string;
  fingerprint: string;
  publicKey: string;
  previousFingerprint: string | null;
}

export interface AppErrorPayload {
  kind:
    | "validation"
    | "notFound"
    | "database"
    | "credential"
    | "io"
    | "terminal"
    | "connection"
    | "timeout"
    | "authFailed"
    | "hostKeyUnknown"
    | "hostKeyChanged"
    | "secretRequired"
    | "sftp"
    | "cancelled"
    | "other";
  message: string;
  details: HostKeyInfo | null;
}

export type TermEvent =
  | { type: "exit"; code: number | null; reason: string | null }
  | { type: "status"; state: string; message: string | null };

export interface KnownHost {
  host: string;
  port: number;
  keyType: string;
  publicKey: string;
  fingerprint: string;
  addedAt: string;
}

export interface RemoteEntry {
  name: string;
  path: string;
  isDir: boolean;
  isSymlink: boolean;
  size: number;
  modified: number | null;
  permissions: string;
  mode: number | null;
  owner: string | null;
  group: string | null;
}

export interface Snippet {
  id: string;
  name: string;
  category: string;
  command: string;
  description: string | null;
  updatedAt: string;
}

export interface TunnelProfile {
  id: string;
  name: string;
  sessionId: string;
  kind: "local" | "remote" | "dynamic";
  bindAddress: string;
  bindPort: number;
  destHost: string | null;
  destPort: number | null;
  autoStart: boolean;
}

export interface TunnelStatus {
  id: string;
  state: string;
  activeConnections: number;
  totalConnections: number;
  error: string | null;
}

export interface SshKeyRecord {
  id: string;
  name: string;
  privatePath: string;
  keyType: string;
  fingerprint: string;
  comment: string | null;
  encrypted: boolean;
  createdAt: string;
}

export interface KeyInfo {
  path: string;
  keyType: string;
  fingerprint: string;
  comment: string;
  encrypted: boolean;
  publicKey: string;
}

export interface HistoryEntry {
  id: number;
  sessionId: string | null;
  label: string;
  protocol: string;
  host: string | null;
  connectedAt: string;
  outcome: string;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  portable: boolean;
  dataNotice: string | null;
  showcase: string | null;
  schemaVersion: number;
  os: string;
}
