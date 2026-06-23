import { saveClientSession, withMtprotoClient } from "./client";
import { saveMtprotoSession } from "../db";

export interface MtprotoUserInfo {
  tgUserId: number;
  firstName: string;
  lastName?: string;
  username?: string;
}

export async function finalizeMtprotoLogin(
  encryptedSession: string,
  phoneNumber: string,
): Promise<MtprotoUserInfo> {
  return withMtprotoClient(encryptedSession, async (client) => {
    const me = await client.getMe();
    const sessionData = saveClientSession(client);
    const tgUserId = Number(me.id.toString());

    saveMtprotoSession(tgUserId, sessionData, phoneNumber);

    return {
      tgUserId,
      firstName: me.firstName ?? "User",
      lastName: me.lastName ?? undefined,
      username: me.username ?? undefined,
    };
  });
}
