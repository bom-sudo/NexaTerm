// Snippet variables use {{name}} or {{name:default}}. Names must start with a letter so that
// Go-template style text such as {{.Names}} (docker --format) is left untouched.
const VAR_RE = /\{\{\s*([A-Za-z][\w-]*)\s*(?::([^}]*))?\}\}/g;

export interface SnippetVar {
  name: string;
  defaultValue: string;
}

export function parseVariables(command: string): SnippetVar[] {
  const seen = new Map<string, SnippetVar>();
  for (const m of command.matchAll(VAR_RE)) {
    if (!seen.has(m[1])) seen.set(m[1], { name: m[1], defaultValue: (m[2] ?? "").trim() });
  }
  return [...seen.values()];
}

export function applyVariables(command: string, values: Record<string, string>): string {
  return command.replace(VAR_RE, (_all, name: string, def?: string) => values[name] ?? (def ?? "").trim());
}

// Patterns that can destroy data or take systems down. Used to require an explicit confirmation
// before broadcasting to several servers or running a macro/snippet remotely.
const DANGEROUS: { re: RegExp; why: string }[] = [
  { re: /\brm\s+(-[a-z]*[rf][a-z]*\s+)+/i, why: "recursive/forced delete (rm -rf)" },
  { re: /\bmkfs(\.\w+)?\b/i, why: "formats a filesystem (mkfs)" },
  { re: /\bdd\s+.*\bof=\/dev\//i, why: "raw write to a block device (dd)" },
  // Only at command position (start, after ; & | or sudo), so "echo reboot-notes" is fine.
  { re: /(^|[;&|]\s*|\bsudo\s+)(shutdown|poweroff|halt|reboot|init\s+[06])(\s|$)/im, why: "shuts down or reboots the host" },
  { re: /:\(\)\s*\{\s*:\|:&\s*\}\s*;\s*:/, why: "fork bomb" },
  { re: /\bchmod\s+(-R\s+)?[0-7]*777\s+\/(\s|$)/i, why: "world-writable root" },
  { re: /\bchown\s+-R\b.*\s\/(\s|$)/i, why: "recursive chown on /" },
  { re: />\s*\/dev\/sd[a-z]/i, why: "overwrites a disk device" },
  { re: /\b(drop\s+(database|table|schema)|truncate\s+table)\b/i, why: "destructive SQL" },
  { re: /\bsystemctl\s+(stop|disable|mask)\b/i, why: "stops/disables services" },
  { re: /\b(iptables|nft)\s+(-F|flush)\b/i, why: "flushes firewall rules" },
  { re: /\bkill(all)?\s+-9\b/i, why: "force-kills processes" },
  { re: /\b(Remove-Item|rd|rmdir|del)\b.*\s-(Recurse|Force|s)\b/i, why: "recursive delete (Windows)" },
  { re: /\bFormat-Volume\b|\bformat\s+[a-z]:/i, why: "formats a volume (Windows)" },
  { re: /\bStop-Computer\b|\bRestart-Computer\b/i, why: "shuts down or restarts Windows" },
  { re: /\bgit\s+push\s+.*--force\b/i, why: "force push" },
];

export function dangerReasons(command: string): string[] {
  return DANGEROUS.filter((d) => d.re.test(command)).map((d) => d.why);
}

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${i === 0 ? v : v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function formatUnixTime(secs: number | null): string {
  if (!secs) return "—";
  const d = new Date(secs * 1000);
  const pad = (x: number) => String(x).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function uid(): string {
  return crypto.randomUUID();
}
