import { create } from "zustand";
import type { SessionPayload } from "@/lib/auth";

interface SessionState {
  session: SessionPayload | null;
  setSession: (session: SessionPayload) => void;
  clearSession: () => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  session: null,
  setSession: (session) => set({ session }),
  clearSession: () => set({ session: null }),
}));

export function formatDisplayName(session: SessionPayload): string {
  return [session.firstName, session.lastName].filter(Boolean).join(" ");
}

export function formatInitials(session: SessionPayload): string {
  const first = session.firstName.at(0) ?? "";
  const last = session.lastName?.at(0) ?? "";
  return (first + last).toUpperCase() || "?";
}
