import {
  defaultShouldDehydrateQuery,
  MutationCache,
  QueryCache,
  QueryClient,
  type QueryClientConfig,
} from "@tanstack/react-query";
import superjson from "superjson";

const STALE_TIME = 30 * 1000; // 30s

function getDefaultOptions(): QueryClientConfig["defaultOptions"] {
  return {
    queries: {
      staleTime: STALE_TIME, // avoid immediate refetch on the client after SSR hydration
    },
    dehydrate: {
      serializeData: superjson.serialize,
      shouldDehydrateQuery: (query) =>
        defaultShouldDehydrateQuery(query)
        || query.state.status === "pending",
    },
    hydrate: {
      deserializeData: superjson.deserialize,
    },
  };
}

/**
 * Creates a QueryClient configured to match the tRPC transformer (superjson),
 * so data serialized on the server hydrates correctly on the client. A fresh
 * instance is created per server request; the browser reuses a singleton (see
 * `client.tsx`).
 */
export function makeQueryClient(
  caches?: Pick<QueryClientConfig, "queryCache" | "mutationCache">,
): QueryClient {
  return new QueryClient({
    ...caches,
    defaultOptions: getDefaultOptions(),
  });
}

export function makeBrowserQueryClient(
  onError: (error: Error) => void,
): QueryClient {
  return makeQueryClient({
    queryCache: new QueryCache({ onError }),
    mutationCache: new MutationCache({ onError }),
  });
}
