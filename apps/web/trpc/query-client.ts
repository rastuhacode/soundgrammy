import {
  defaultShouldDehydrateQuery,
  QueryClient,
} from "@tanstack/react-query";
import superjson from "superjson";

/**
 * Creates a QueryClient configured to match the tRPC transformer (superjson),
 * so data serialized on the server hydrates correctly on the client. A fresh
 * instance is created per server request; the browser reuses a singleton (see
 * `client.tsx`).
 */
export function makeQueryClient(): QueryClient {
  const STALE_TIME = 30 * 1000; // 30s

  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: STALE_TIME, // avoid immediate refetch on the client after SSR hydration
      },
      dehydrate: {
        serializeData: superjson.serialize,
        shouldDehydrateQuery: (query) =>
          defaultShouldDehydrateQuery(query) ||
          query.state.status === "pending",
      },
      hydrate: {
        deserializeData: superjson.deserialize,
      },
    },
  });
}
