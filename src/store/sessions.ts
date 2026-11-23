import { create } from "zustand";
import { api } from "../services/api";
import type { Session, SessionGroup, ShellProfile } from "../types";

interface SessionsStore {
  sessions: Session[];
  groups: SessionGroup[];
  shells: ShellProfile[];
  loaded: boolean;
  refresh: () => Promise<void>;
  loadShells: () => Promise<void>;
  byId: (id: string | null | undefined) => Session | undefined;
}

export const useSessions = create<SessionsStore>((set, get) => ({
  sessions: [],
  groups: [],
  shells: [],
  loaded: false,
  refresh: async () => {
    const [sessions, groups] = await Promise.all([api.sessionsList(), api.groupsList()]);
    set({ sessions, groups, loaded: true });
  },
  loadShells: async () => set({ shells: await api.shellsList() }),
  byId: (id) => (id ? get().sessions.find((s) => s.id === id) : undefined),
}));
