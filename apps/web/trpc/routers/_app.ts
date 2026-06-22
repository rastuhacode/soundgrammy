import { createTRPCRouter } from "../init";
import { authRouter as auth } from "./auth";
import { mtprotoRouter as mtproto } from "./mtproto";
import { playlistsRouter as playlists } from "./playlists";
import { tracksRouter as tracks } from "./tracks";

export const appRouter = createTRPCRouter({ auth, mtproto, playlists, tracks });
export type AppRouter = typeof appRouter;
