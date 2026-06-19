import "server-only";

import { initTRPC, TRPCError } from "@trpc/server";
import type { FetchCreateContextFnOptions } from "@trpc/server/adapters/fetch";
import { cache } from "react";
import superjson from "superjson";
import { getSession } from "lib/auth";

/**
 * tRPC backend initialization: context, transformer, and the procedure
 * builders used across routers. Routers stay thin and delegate to the
 * framework-agnostic helpers in `lib/`.
 */

export interface TRPCContext {
  /** Response headers for the current request, or `null` in RSC where there is no HTTP response to mutate. */
  resHeaders: Headers | null;
  /** Appends a `Set-Cookie` header to the response (no-op in RSC). */
  setCookie: (value: string) => void;
}

/**
 * Context for React Server Component calls (via `createTRPCOptionsProxy`).
 * Wrapped in React's `cache` so it is created once per request. There is no
 * HTTP response to attach cookies to here, so cookie writes are no-ops.
 */
export const createTRPCContext = cache(async (): Promise<TRPCContext> => {
  return { resHeaders: null, setCookie: () => undefined };
});

/** Context for HTTP requests handled by the fetch adapter, where cookies can be set. */
export function createFetchContext({
  resHeaders,
}: FetchCreateContextFnOptions): TRPCContext {
  return {
    resHeaders,
    setCookie: (value) => resHeaders.append("Set-Cookie", value),
  };
}

const t = initTRPC.context<TRPCContext>().create({ transformer: superjson });

export const createTRPCRouter = t.router;
export const createCallerFactory = t.createCallerFactory;

/** Procedure available to anyone (e.g. login flows before a session exists). */
export const publicProcedure = t.procedure;

/**
 * Procedure that requires a valid session cookie. Resolves the session once
 * and injects it into `ctx` so handlers can rely on `ctx.session`.
 */
export const protectedProcedure = t.procedure.use(async ({ ctx, next }) => {
  const session = await getSession();
  if (!session) {
    throw new TRPCError({ code: "UNAUTHORIZED", message: "Not authenticated" });
  }
  return next({ ctx: { ...ctx, session } });
});

/**
 * Normalizes a thrown value into a {@link TRPCError} with a client-visible
 * message, passing existing {@link TRPCError}s through unchanged.
 */
export function toTRPCError(
  error: unknown,
  code: TRPCError["code"] = "BAD_REQUEST",
): TRPCError {
  if (error instanceof TRPCError) {
    return error;
  }
  return new TRPCError({
    code,
    message: error instanceof Error ? error.message : "Unexpected error",
  });
}
