import { z } from "zod";
import {
  addTrackToPlaylist,
  createPlaylist,
  deletePlaylist,
  getPlaylistsBundle,
  removeTrackFromPlaylist,
  toggleLikedTrack,
  type PlaylistsBundle,
} from "lib/db";
import { createTRPCRouter, protectedProcedure, toTRPCError } from "../init";

export const playlistsRouter = createTRPCRouter({
  list: protectedProcedure.query(({ ctx }): PlaylistsBundle =>
    getPlaylistsBundle(ctx.session.tgUserId),
  ),

  create: protectedProcedure
    .input(z.object({ name: z.string().min(1).max(100) }))
    .mutation(({ ctx, input }) => {
      try {
        return createPlaylist(ctx.session.tgUserId, input.name);
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  delete: protectedProcedure
    .input(z.object({ id: z.number().int().positive() }))
    .mutation(({ ctx, input }) => {
      try {
        deletePlaylist(input.id, ctx.session.tgUserId);
        return { success: true as const };
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  addTrack: protectedProcedure
    .input(
      z.object({
        playlistId: z.number().int().positive(),
        trackId: z.number().int().positive(),
      }),
    )
    .mutation(({ ctx, input }) => {
      try {
        addTrackToPlaylist(
          input.playlistId,
          input.trackId,
          ctx.session.tgUserId,
        );
        return { success: true as const };
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  removeTrack: protectedProcedure
    .input(
      z.object({
        playlistId: z.number().int().positive(),
        trackId: z.number().int().positive(),
      }),
    )
    .mutation(({ ctx, input }) => {
      try {
        removeTrackFromPlaylist(
          input.playlistId,
          input.trackId,
          ctx.session.tgUserId,
        );
        return { success: true as const };
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  toggleLike: protectedProcedure
    .input(z.object({ trackId: z.number().int().positive() }))
    .mutation(({ ctx, input }) => {
      try {
        return toggleLikedTrack(input.trackId, ctx.session.tgUserId);
      } catch (error) {
        throw toTRPCError(error);
      }
    }),
});
