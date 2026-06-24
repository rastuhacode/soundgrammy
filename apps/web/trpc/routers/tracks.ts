import { z } from "zod";
import { deleteTrack, getTracksByUser, type Track } from "@/lib/db";
import { getTrackMetadata } from "@/lib/track-metadata";
import { createTRPCRouter, protectedProcedure, toTRPCError } from "../init";

export const tracksRouter = createTRPCRouter({
  /** Lists the signed-in user's tracks, newest first. */
  list: protectedProcedure.query(({ ctx }): Track[] =>
    getTracksByUser(ctx.session.tgUserId),
  ),

  /** Loads track and Telegram document metadata for the info dialog. */
  metadata: protectedProcedure
    .input(z.object({ trackId: z.number().int().positive() }))
    .query(async ({ ctx, input }) => {
      try {
        return await getTrackMetadata(input.trackId, ctx.session.tgUserId);
      } catch (error) {
        throw toTRPCError(error, "BAD_GATEWAY");
      }
    }),

  /** Deletes one of the signed-in user's tracks. */
  delete: protectedProcedure
    .input(z.object({ id: z.number().int().positive() }))
    .mutation(({ ctx, input }) => {
      deleteTrack(input.id, ctx.session.tgUserId);
      return { success: true as const };
    }),
});
