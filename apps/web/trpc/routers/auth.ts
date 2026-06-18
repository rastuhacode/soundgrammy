import { getSession } from "lib/auth";
import { serializeClearSessionCookie } from "lib/auth";
import { deleteMtprotoSession } from "lib/db";
import { createTRPCRouter, protectedProcedure, publicProcedure } from "../init";

export const authRouter = createTRPCRouter({
  /** Returns the current session payload (decoded from the session cookie). */
  me: protectedProcedure.query(({ ctx }) => ctx.session),

  /** Clears the session cookie and forgets the user's MTProto session. */
  logout: publicProcedure.mutation(async ({ ctx }) => {
    const session = await getSession();
    ctx.setCookie(serializeClearSessionCookie());
    if (session) {
      deleteMtprotoSession(session.tgUserId);
    }
    return { ok: true as const };
  }),
});
