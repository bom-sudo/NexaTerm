// Live xterm instances by pane id, so toolbar/menu actions can reach the active terminal.
import type { Terminal } from "@xterm/xterm";
import type { SearchAddon } from "@xterm/addon-search";

export interface TermHandle {
  term: Terminal;
  search: SearchAddon;
  fit: () => void;
  terminalId: () => string | undefined;
}

export const terminals = new Map<string, TermHandle>();

/** Plain text of the whole buffer (scrollback included) for export. */
export function bufferText(term: Terminal): string {
  const b = term.buffer.active;
  const lines: string[] = [];
  for (let i = 0; i < b.length; i++) lines.push(b.getLine(i)?.translateToString(true) ?? "");
  while (lines.length && lines[lines.length - 1] === "") lines.pop();
  return lines.join("\r\n") + "\r\n";
}
