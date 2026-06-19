import { createTRPCRouter } from "../init";
import { authRouter as auth } from "./auth";
import { mtprotoRouter as mtproto } from "./mtproto";
import { tracksRouter as tracks } from "./tracks";

export const appRouter = createTRPCRouter({ auth, mtproto, tracks });
export type AppRouter = typeof appRouter;
