"use client";

import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { createTRPCClient, httpBatchLink } from "@trpc/client";
import { createTRPCContext } from "@trpc/tanstack-react-query";
import { useState, type ReactNode } from "react";
import superjson from "superjson";
import { makeQueryClient } from "./query-client";
import type { AppRouter } from "./routers/_app";

/**
 * Client-side tRPC hooks. Components call `useTRPC()` to build typed query and
 * mutation options that are passed to TanStack Query's `useQuery`/`useMutation`.
 */
export const { TRPCProvider, useTRPC } = createTRPCContext<AppRouter>();

let browserQueryClient: QueryClient | undefined;

function getQueryClient(): QueryClient {
  // Server: always a fresh client. Browser: reuse one so cache persists across renders.
  if (typeof window === "undefined") {
    return makeQueryClient();
  }
  browserQueryClient ??= makeQueryClient();
  return browserQueryClient;
}

function getBaseUrl(): string {
  if (typeof window !== "undefined") {
    return "";
  }
  return `http://localhost:${process.env.PORT ?? 3000}`;
}

/**
 * Wraps the app with the TanStack Query and tRPC providers.
 *
 * Uses `httpBatchLink` (not the streaming variant) so `Set-Cookie` headers from
 * auth mutations propagate back to the browser.
 */
export function TRPCReactProvider({ children }: { children: ReactNode }) {
  const queryClient = getQueryClient();
  const [trpcClient] = useState(() =>
    createTRPCClient<AppRouter>({
      links: [
        httpBatchLink({
          url: `${getBaseUrl()}/api/trpc`,
          transformer: superjson,
        }),
      ],
    }),
  );

  return (
    <QueryClientProvider client={queryClient}>
      <TRPCProvider trpcClient={trpcClient} queryClient={queryClient}>
        {children}
      </TRPCProvider>
    </QueryClientProvider>
  );
}
