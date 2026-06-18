import { z } from "zod";
import { deleteTrack, getTracksByUser, type Track } from "lib/db";
import { createTRPCRouter, protectedProcedure } from "../init";

export const tracksRouter = createTRPCRouter({
  /** Lists the signed-in user's tracks, newest first. */
  list: protectedProcedure.query(({ ctx }): Track[] =>
    getTracksByUser(ctx.session.tgUserId),
  ),

  /** Deletes one of the signed-in user's tracks. */
  delete: protectedProcedure
    .input(z.object({ id: z.number().int().positive() }))
    .mutation(({ ctx, input }) => {
      deleteTrack(input.id, ctx.session.tgUserId);
      return { success: true as const };
    }),
});
