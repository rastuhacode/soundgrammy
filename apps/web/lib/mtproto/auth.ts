import { Api } from "telegram";
import { computeCheck } from "telegram/Password";
import { createMtprotoClient, saveClientSession } from "./client";
import { decryptSession } from "./crypto";
import { getMtprotoCredentials } from "./config";
import type { CodeDelivery } from "./phone";
import { getCodeDelivery } from "./phone";

export async function sendAuthCode(phoneNumber: string, forceSms = false) {
  const { apiId, apiHash } = getMtprotoCredentials();
  const client = await createMtprotoClient("");
  try {
    const result = await client.sendCode(
      { apiId, apiHash },
      phoneNumber,
      forceSms,
    );

    return {
      phoneCodeHash: result.phoneCodeHash,
      sessionData: saveClientSession(client),
      codeDelivery: getCodeDelivery(result.isCodeViaApp),
    };
  } finally {
    await client.disconnect();
  }
}

export async function resendAuthCode(
  phoneNumber: string,
  phoneCodeHash: string,
  encryptedSession: string,
): Promise<{ phoneCodeHash: string; sessionData: string; codeDelivery: CodeDelivery }> {
  const sessionString = decryptSession(encryptedSession);
  const client = await createMtprotoClient(sessionString);

  try {
    const result = await client.invoke(
      new Api.auth.ResendCode({
        phoneNumber,
        phoneCodeHash,
      }),
    );

    if (result instanceof Api.auth.SentCodeSuccess) {
      throw new Error("Already logged in");
    }

    return {
      phoneCodeHash: result.phoneCodeHash,
      sessionData: saveClientSession(client),
      codeDelivery: getCodeDelivery(
        result.type instanceof Api.auth.SentCodeTypeApp,
      ),
    };
  } finally {
    await client.disconnect();
  }
}

export async function signInWithCode(
  phoneNumber: string,
  phoneCodeHash: string,
  code: string,
  encryptedSession: string,
) {
  const sessionString = decryptSession(encryptedSession);
  const client = await createMtprotoClient(sessionString);

  try {
    try {
      await client.invoke(
        new Api.auth.SignIn({
          phoneNumber,
          phoneCodeHash,
          phoneCode: code,
        }),
      );
    } catch (error) {
      if (
        error instanceof Error &&
        "errorMessage" in error &&
        (error as { errorMessage?: string }).errorMessage ===
          "SESSION_PASSWORD_NEEDED"
      ) {
        return {
          needsPassword: true as const,
          sessionData: saveClientSession(client),
        };
      }
      throw error;
    }

    return {
      needsPassword: false as const,
      sessionData: saveClientSession(client),
    };
  } finally {
    await client.disconnect();
  }
}

export async function signInWithPassword(
  password: string,
  encryptedSession: string,
) {
  const sessionString = decryptSession(encryptedSession);
  const client = await createMtprotoClient(sessionString);

  try {
    const passwordInfo = await client.invoke(new Api.account.GetPassword());
    const passwordCheck = await computeCheck(passwordInfo, password);
    await client.invoke(
      new Api.auth.CheckPassword({ password: passwordCheck }),
    );

    return {
      sessionData: saveClientSession(client),
    };
  } finally {
    await client.disconnect();
  }
}
