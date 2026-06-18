import { createTRPCRouter } from "../init";
import { authRouter } from "./auth";
import { mtprotoRouter } from "./mtproto";
import { tracksRouter } from "./tracks";

export const appRouter = createTRPCRouter({
  auth: authRouter,
  mtproto: mtprotoRouter,
  tracks: tracksRouter,
});

export type AppRouter = typeof appRouter;
