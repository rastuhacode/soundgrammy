import { z } from "zod";
import {
  addTrackToPlaylist,
  createPlaylist,
  deletePlaylist,
  getPlaylistsBundle,
  removeTrackFromPlaylist,
  toggleLikedTrack,
  updatePlaylist,
  type PlaylistsBundle,
} from "@/lib/db";
import { createTRPCRouter, protectedProcedure, toTRPCError } from "../init";

const playlistThumbnailSchema = z.object({
  data: z.string().min(1).max(700_000),
  mime: z.enum(["image/jpeg", "image/png", "image/webp"]),
});

export const playlistsRouter = createTRPCRouter({
  list: protectedProcedure.query(
    ({ ctx }): PlaylistsBundle => getPlaylistsBundle(ctx.session.tgUserId),
  ),

  create: protectedProcedure
    .input(
      z.object({
        name: z.string().min(1).max(100),
        thumbnail: playlistThumbnailSchema.optional(),
      }),
    )
    .mutation(({ ctx, input }) => {
      try {
        return createPlaylist(
          ctx.session.tgUserId,
          input.name,
          input.thumbnail,
        );
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  update: protectedProcedure
    .input(
      z.object({
        id: z.number().int().positive(),
        name: z.string().min(1).max(100).optional(),
        thumbnail: playlistThumbnailSchema.nullable().optional(),
      }),
    )
    .mutation(({ ctx, input }) => {
      try {
        return updatePlaylist(input.id, ctx.session.tgUserId, {
          name: input.name,
          thumbnail: input.thumbnail,
        });
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
