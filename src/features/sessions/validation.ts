import type { SessionInput } from "../../types";

export type FieldErrors = Partial<Record<"host" | "port" | "username" | "keyPath", string>>;
export const FIELD_ORDER = ["host", "port", "username", "keyPath"] as const;

/** Checks the fields the user must fill in, with messages that say what to type. */
export function validateFields(f: SessionInput): FieldErrors {
  const e: FieldErrors = {};
  if (f.protocol === "ssh" || f.protocol === "telnet") {
    const host = (f.host ?? "").trim();
    if (!host) e.host = "Enter the server's address, for example 192.168.1.10 or server.example.com.";
    else if (/\s/.test(host)) e.host = "The address cannot contain spaces.";
    else if (host.includes("@")) e.host = "Put only the address here; the user name goes in Username.";
    const port = Number(f.port ?? (f.protocol === "ssh" ? 22 : 23));
    if (!Number.isInteger(port) || port < 1 || port > 65535) e.port = "Use a port from 1 to 65535.";
  }
  if (f.protocol === "ssh") {
    if (!(f.username ?? "").trim()) e.username = "Enter the user name to log in with, for example root.";
    if (f.authMethod === "key" && !(f.keyPath ?? "").trim()) e.keyPath = "Choose your private key file with the folder button.";
  }
  return e;
}

/** Which field a validation message from the backend belongs to, if any. */
export function fieldForServerError(message: string): keyof FieldErrors | null {
  if (/^host|invalid host/i.test(message)) return "host";
  if (/^port/i.test(message)) return "port";
  if (/username/i.test(message)) return "username";
  if (/private key path/i.test(message)) return "keyPath";
  return null;
}
