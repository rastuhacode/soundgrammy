import { fetchRequestHandler } from "@trpc/server/adapters/fetch";
import { createFetchContext } from "trpc/init";
import { appRouter } from "trpc/routers/_app";

function handler(req: Request) {
  return fetchRequestHandler({
    endpoint: "/api/trpc",
    req,
    router: appRouter,
    createContext: createFetchContext,
  });
}

export { handler as GET, handler as POST };
