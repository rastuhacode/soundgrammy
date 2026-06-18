import { randomUUID } from "node:crypto";
import QRCode from "qrcode";
import { z } from "zod";
import { TRPCError } from "@trpc/server";
import { createSession, serializeSessionCookie } from "lib/auth";
import {
  createMtprotoAuthPending,
  deleteMtprotoAuthPending,
  getMtprotoAuthPending,
  getMtprotoSession,
  updateMtprotoAuthPending,
  updateMtprotoAuthPendingSession,
  updateMtprotoLastSync,
  updateMtprotoSavedMusicHash,
} from "lib/db";
import {
  resendAuthCode,
  sendAuthCode,
  signInWithCode,
  signInWithPassword,
} from "lib/mtproto/auth";
import { withMtprotoClient } from "lib/mtproto/client";
import { finalizeMtprotoLogin, type MtprotoUserInfo } from "lib/mtproto/login";
import { normalizePhoneNumber } from "lib/mtproto/phone";
import {
  consumeQrSuccess,
  getQrAuthStatus,
  startQrAuth,
  submitQrPassword,
} from "lib/mtproto/qr-auth";
import { syncProfileMusic } from "lib/mtproto/sync";
import {
  createTRPCRouter,
  protectedProcedure,
  publicProcedure,
  toTRPCError,
} from "../init";

/** Loads a pending auth row or fails with the canonical "expired" message. */
function requirePending(authToken: string) {
  const pending = getMtprotoAuthPending(authToken);
  if (!pending) {
    throw new TRPCError({
      code: "NOT_FOUND",
      message: "Auth session expired. Request a new code.",
    });
  }
  return pending;
}

/** Issues the app session cookie for a freshly authenticated MTProto user. */
async function establishSession(
  setCookie: (value: string) => void,
  user: MtprotoUserInfo,
): Promise<void> {
  const token = await createSession({
    tgUserId: user.tgUserId,
    firstName: user.firstName,
    lastName: user.lastName,
    username: user.username,
  });
  setCookie(serializeSessionCookie(token));
}

const qrRouter = createTRPCRouter({
  /** Starts a QR login session and returns the QR payload to render. */
  start: publicProcedure.mutation(async () => {
    try {
      const authToken = randomUUID();
      const { qrUrl, expires } = await startQrAuth(authToken);
      const qrDataUrl = await QRCode.toDataURL(qrUrl, {
        width: 280,
        margin: 1,
        color: { dark: "#000000", light: "#ffffff" },
      });
      return { authToken, qrUrl, qrDataUrl, expires };
    } catch (error) {
      throw toTRPCError(error);
    }
  }),

  /**
   * Polls the QR login status. On success, establishes the session cookie and
   * consumes the QR session (mirrors the previous `/qr/status` endpoint).
   */
  status: publicProcedure
    .input(z.object({ authToken: z.string().min(1) }))
    .query(async ({ ctx, input }) => {
      const status = getQrAuthStatus(input.authToken);

      if (status.status === "success" && status.user) {
        const user = consumeQrSuccess(input.authToken) ?? status.user;
        await establishSession(ctx.setCookie, user);
        return { status: "success" as const };
      }

      return status;
    }),

  /** Submits the 2FA password for a QR login awaiting it. */
  password: publicProcedure
    .input(z.object({ authToken: z.string().min(1), password: z.string().min(1) }))
    .mutation(({ input }) => {
      try {
        submitQrPassword(input.authToken, input.password);
        return { ok: true as const };
      } catch (error) {
        throw toTRPCError(error);
      }
    }),
});

export const mtprotoRouter = createTRPCRouter({
  qr: qrRouter,

  /** Reports whether the user has a connected MTProto session. */
  status: protectedProcedure.query(({ ctx }) => {
    const mtprotoSession = getMtprotoSession(ctx.session.tgUserId);
    return {
      connected: Boolean(mtprotoSession),
      phoneNumber: mtprotoSession?.phone_number ?? null,
      lastSyncAt: mtprotoSession?.last_sync_at ?? null,
    };
  }),

  /** Syncs the user's Telegram profile music into the local library. */
  sync: protectedProcedure.mutation(async ({ ctx }) => {
    const mtprotoSession = getMtprotoSession(ctx.session.tgUserId);
    if (!mtprotoSession) {
      throw new TRPCError({
        code: "BAD_REQUEST",
        message: "Connect your Telegram account first",
      });
    }

    try {
      const result = await withMtprotoClient(
        mtprotoSession.session_data,
        (client) =>
          syncProfileMusic(client, ctx.session.tgUserId, {
            storedHash: mtprotoSession.saved_music_hash,
          }),
      );

      updateMtprotoLastSync(ctx.session.tgUserId);
      if (!result.notModified) {
        updateMtprotoSavedMusicHash(ctx.session.tgUserId, result.hash);
      }
      return result;
    } catch (error) {
      throw toTRPCError(error, "INTERNAL_SERVER_ERROR");
    }
  }),

  /** Sends a login code to the given phone number and opens a pending auth. */
  sendCode: publicProcedure
    .input(
      z.object({
        phoneNumber: z.string().min(1),
        forceSms: z.boolean().optional(),
      }),
    )
    .mutation(async ({ input }) => {
      let phoneNumber: string;
      try {
        phoneNumber = normalizePhoneNumber(input.phoneNumber.trim());
      } catch (error) {
        throw toTRPCError(error);
      }

      try {
        const { phoneCodeHash, sessionData, codeDelivery } = await sendAuthCode(
          phoneNumber,
          input.forceSms ?? false,
        );
        const authToken = randomUUID();
        createMtprotoAuthPending(
          authToken,
          phoneNumber,
          phoneCodeHash,
          sessionData,
        );
        return { authToken, codeDelivery };
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  /** Resends the login code for an existing pending auth (e.g. switch to SMS). */
  resendCode: publicProcedure
    .input(z.object({ authToken: z.string().min(1) }))
    .mutation(async ({ input }) => {
      const pending = requirePending(input.authToken);
      try {
        const result = await resendAuthCode(
          pending.phone_number,
          pending.phone_code_hash,
          pending.session_data,
        );
        updateMtprotoAuthPending(
          input.authToken,
          result.phoneCodeHash,
          result.sessionData,
        );
        return { codeDelivery: result.codeDelivery };
      } catch (error) {
        throw toTRPCError(error);
      }
    }),

  /** Verifies the login code; may require a 2FA password as a next step. */
  signIn: publicProcedure
    .input(z.object({ authToken: z.string().min(1), code: z.string().min(1) }))
    .mutation(async ({ ctx, input }) => {
      const pending = requirePending(input.authToken);
      try {
        const result = await signInWithCode(
          pending.phone_number,
          pending.phone_code_hash,
          input.code.trim(),
          pending.session_data,
        );

        if (result.needsPassword) {
          updateMtprotoAuthPendingSession(input.authToken, result.sessionData);
          return { needsPassword: true as const };
        }

        const user = await finalizeMtprotoLogin(
          result.sessionData,
          pending.phone_number,
        );
        deleteMtprotoAuthPending(input.authToken);
        await establishSession(ctx.setCookie, user);
        return { needsPassword: false as const };
      } catch (error) {
        throw toTRPCError(error, "UNAUTHORIZED");
      }
    }),

  /** Completes sign-in for accounts protected by a 2FA password. */
  password: publicProcedure
    .input(
      z.object({ authToken: z.string().min(1), password: z.string().min(1) }),
    )
    .mutation(async ({ ctx, input }) => {
      const pending = requirePending(input.authToken);
      try {
        const result = await signInWithPassword(
          input.password,
          pending.session_data,
        );
        const user = await finalizeMtprotoLogin(
          result.sessionData,
          pending.phone_number,
        );
        deleteMtprotoAuthPending(input.authToken);
        await establishSession(ctx.setCookie, user);
        return { ok: true as const };
      } catch (error) {
        throw toTRPCError(error, "UNAUTHORIZED");
      }
    }),
});
