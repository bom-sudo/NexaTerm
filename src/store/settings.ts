import { create } from "zustand";
import { api } from "../services/api";

export interface Settings {
  theme: "dark" | "light";
  terminalTheme: string;
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
  cursorStyle: "block" | "bar" | "underline";
  cursorBlink: boolean;
  scrollback: number;
  copyOnSelect: boolean;
  rightClickPaste: boolean;
  /** When true, Ctrl+W / Ctrl+T inside a terminal go to the shell (use Ctrl+Shift+W/T for tabs). */
  shellKeysPassThrough: boolean;
  defaultShellId: string;
  confirmCloseConnected: boolean;
  onboarded: boolean;
  /** Colour error/warning/success words and IP addresses in terminal output. */
  highlightOutput: boolean;
  /** Ask before pasting text that contains line breaks (it would run several commands). */
  confirmMultilinePaste: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  theme: "dark",
  terminalTheme: "midnight",
  fontFamily: '"Cascadia Mono", "Cascadia Code", Consolas, monospace',
  fontSize: 14,
  lineHeight: 1.15,
  cursorStyle: "block",
  cursorBlink: true,
  scrollback: 10000,
  copyOnSelect: false,
  rightClickPaste: true,
  shellKeysPassThrough: true,
  defaultShellId: "powershell",
  confirmCloseConnected: true,
  onboarded: false,
  highlightOutput: true,
  confirmMultilinePaste: true,
};

interface SettingsStore {
  settings: Settings;
  loaded: boolean;
  load: () => Promise<void>;
  set: <K extends keyof Settings>(key: K, value: Settings[K]) => Promise<void>;
}

export const useSettings = create<SettingsStore>((set, get) => ({
  settings: DEFAULT_SETTINGS,
  loaded: false,
  load: async () => {
    const stored = await api.settingsGet();
    const merged = { ...DEFAULT_SETTINGS } as Record<string, unknown>;
    for (const [k, v] of Object.entries(stored)) if (k in merged) merged[k] = v;
    set({ settings: merged as unknown as Settings, loaded: true });
  },
  set: async (key, value) => {
    set({ settings: { ...get().settings, [key]: value } });
    await api.settingsSet(key, value);
  },
}));

export interface TermPalette {
  name: string;
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
  black: string; red: string; green: string; yellow: string; blue: string; magenta: string; cyan: string; white: string;
  brightBlack: string; brightRed: string; brightGreen: string; brightYellow: string; brightBlue: string; brightMagenta: string; brightCyan: string; brightWhite: string;
}

export const TERMINAL_THEMES: Record<string, TermPalette> = {
  midnight: {
    name: "Midnight (default)",
    background: "#0b1222", foreground: "#d6e2f5", cursor: "#38bdf8", selectionBackground: "#1e3a5f",
    black: "#1b2438", red: "#f2777a", green: "#7fd88f", yellow: "#f2c66d", blue: "#6aa8ff", magenta: "#c594f5", cyan: "#4fd6e8", white: "#cfd8e8",
    brightBlack: "#5a6a8a", brightRed: "#ff9a9c", brightGreen: "#a3eeb0", brightYellow: "#ffd98a", brightBlue: "#93c0ff", brightMagenta: "#dbb4ff", brightCyan: "#82ecf7", brightWhite: "#ffffff",
  },
  daylight: {
    name: "Daylight",
    background: "#fbfcfe", foreground: "#1d2a44", cursor: "#0284c7", selectionBackground: "#cfe3f7",
    black: "#1d2a44", red: "#c43c3c", green: "#2f8a43", yellow: "#a06a00", blue: "#1f5fbf", magenta: "#8a3fb8", cyan: "#0e7a8a", white: "#c9d3e3",
    brightBlack: "#5f6e8c", brightRed: "#e05252", brightGreen: "#3aa856", brightYellow: "#c48a10", brightBlue: "#3b7de0", brightMagenta: "#a65ad6", brightCyan: "#1597aa", brightWhite: "#eef2f8",
  },
  solarizedDark: {
    name: "Solarized Dark",
    background: "#002b36", foreground: "#93a1a1", cursor: "#93a1a1", selectionBackground: "#073642",
    black: "#073642", red: "#dc322f", green: "#859900", yellow: "#b58900", blue: "#268bd2", magenta: "#d33682", cyan: "#2aa198", white: "#eee8d5",
    brightBlack: "#586e75", brightRed: "#cb4b16", brightGreen: "#586e75", brightYellow: "#657b83", brightBlue: "#839496", brightMagenta: "#6c71c4", brightCyan: "#93a1a1", brightWhite: "#fdf6e3",
  },
  highContrast: {
    name: "High Contrast",
    background: "#000000", foreground: "#ffffff", cursor: "#ffff00", selectionBackground: "#3a3a8a",
    black: "#000000", red: "#ff5555", green: "#55ff55", yellow: "#ffff55", blue: "#5599ff", magenta: "#ff55ff", cyan: "#55ffff", white: "#e6e6e6",
    brightBlack: "#808080", brightRed: "#ff8080", brightGreen: "#80ff80", brightYellow: "#ffff80", brightBlue: "#80b3ff", brightMagenta: "#ff80ff", brightCyan: "#80ffff", brightWhite: "#ffffff",
  },
};
