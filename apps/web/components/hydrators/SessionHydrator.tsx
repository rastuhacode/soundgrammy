"use client";

import { useEffect } from "react";
import type { SessionPayload } from "@/lib/auth";
import { useSessionStore } from "@/stores/session-store";

interface SessionHydratorProps {
  session: SessionPayload;
  children: React.ReactNode;
}

export function SessionHydrator({ session, children }: SessionHydratorProps) {
  const setSession = useSessionStore((state) => state.setSession);

  useEffect(() => {
    setSession(session);
  }, [session, setSession]);

  return children;
}
