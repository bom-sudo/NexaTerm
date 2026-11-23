import { create } from "zustand";
import type { ReactNode } from "react";
import { uid } from "../utils/commands";

export interface Toast {
  id: string;
  kind: "info" | "success" | "error" | "warn";
  text: string;
}

/** A modal rendered by <DialogHost/>. */
export interface DialogEntry {
  id: string;
  node: () => ReactNode;
}

interface UiStore {
  toasts: Toast[];
  dialogs: DialogEntry[];
  sidebarView: "sessions" | "snippets" | "history";
  sessionSearchFocus: number;
  /** durationMs = 0 keeps the toast until the user dismisses it. */
  toast: (kind: Toast["kind"], text: string, durationMs?: number) => void;
  dismiss: (id: string) => void;
  setSidebarView: (v: UiStore["sidebarView"]) => void;
  focusSessionSearch: () => void;
  open: <T>(render: (close: (value: T) => void) => ReactNode) => Promise<T>;
  closeDialog: (id: string) => void;
}

export const useUi = create<UiStore>((set, get) => ({
  toasts: [],
  dialogs: [],
  sidebarView: "sessions",
  sessionSearchFocus: 0,
  toast: (kind, text, durationMs) => {
    const id = uid();
    set({ toasts: [...get().toasts, { id, kind, text }].slice(-5) });
    const ms = durationMs ?? (kind === "error" ? 8000 : 4000);
    if (ms > 0) setTimeout(() => get().dismiss(id), ms);
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
  setSidebarView: (v) => set({ sidebarView: v }),
  focusSessionSearch: () => set({ sidebarView: "sessions", sessionSearchFocus: get().sessionSearchFocus + 1 }),
  open: <T,>(render: (close: (value: T) => void) => ReactNode) =>
    new Promise<T>((resolve) => {
      const id = uid();
      const close = (value: T) => {
        get().closeDialog(id);
        resolve(value);
      };
      set({ dialogs: [...get().dialogs, { id, node: () => render(close) }] });
    }),
  closeDialog: (id) => set({ dialogs: get().dialogs.filter((d) => d.id !== id) }),
}));

export const toast = (kind: Toast["kind"], text: string, durationMs?: number) => useUi.getState().toast(kind, text, durationMs);
export const openDialog = <T,>(render: (close: (value: T) => void) => ReactNode) => useUi.getState().open<T>(render);
