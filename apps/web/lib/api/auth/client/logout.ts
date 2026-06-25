"use client";

import { createTRPCClient, httpBatchLink } from "@trpc/client";
import superjson from "superjson";
import { useSessionStore } from "@/stores/session-store";
import type { AppRouter } from "@/trpc/routers/_app";

let logoutClient: ReturnType<typeof createTRPCClient<AppRouter>> | undefined;

function getLogoutClient() {
  logoutClient ??= createTRPCClient<AppRouter>({
    links: [
      httpBatchLink({
        url: "/api/trpc",
        transformer: superjson,
      }),
    ],
  });
  return logoutClient;
}

/** Clears the server session cookie and client session state. */
export async function performClientLogout(): Promise<void> {
  try {
    await getLogoutClient().auth.logout.mutate();
  } catch {
    // Session may already be invalid — still clear client state.
  }

  useSessionStore.getState().clearSession();
}
