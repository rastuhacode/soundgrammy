import bigInt, { type BigInteger } from "big-integer";
import type { TelegramClient } from "telegram";
import { Api } from "telegram";
import { toSignedLittleBuffer } from "telegram/Helpers";
import { tlobjects } from "telegram/tl/AllTLObjects";

/**
 * Hand-rolled TL (Type Language) bindings for Telegram's `users.savedMusic`
 * layer, which GramJS does not ship constructors for yet.
 *
 * The requests are plain factory functions rather than classes: GramJS'
 * `client.invoke` only needs a value that satisfies the {@link SavedMusicTlRequest}
 * contract (serialization via `getBytes` plus the read/resolve hooks), so there
 * is no behavior that benefits from a class here.
 */

// Telegram TL schema constructor IDs (users.savedMusic layer)
const SAVED_MUSIC_ID = 0x34a2f297; // users.savedMusic
const SAVED_MUSIC_NOT_MODIFIED_ID = 0xe3878aa4; // users.savedMusicNotModified
const GET_SAVED_MUSIC_ID = 0x788d7fe3; // users.getSavedMusic
const GET_SAVED_MUSIC_BY_ID = 0x7573a4e9; // users.getSavedMusicByID
const VECTOR_CONSTRUCTOR_ID = 0x1cb5c415; // vector constructor (TL built-in)

/** Telegram saved-music page size (max documents returned per request). */
export const SAVED_MUSIC_PAGE_SIZE = 100;

export type SavedMusicResult =
  | { className: "users.savedMusic"; count: number; documents: Api.Document[] }
  | { className: "users.savedMusicNotModified"; count: number };

/**
 * Minimal subset of the GramJS request contract that `client.invoke` relies on
 * to serialize a custom TL request and read its response.
 */
interface SavedMusicTlRequest {
  CONSTRUCTOR_ID: number;
  SUBCLASS_OF_ID: number;
  classType: "request";
  className: string;
  getBytes(): Buffer;
  readResult(reader: { tgReadObject: () => unknown }): unknown;
  resolve(): Promise<void>;
}

let registered = false;

/**
 * Registers the saved-music response constructors with GramJS so the wire
 * reader knows how to decode them. Idempotent and invoked automatically by the
 * request helpers below.
 */
function ensureSavedMusicTlRegistered(): void {
  if (registered) return;
  registered = true;

  class SavedMusicNotModified {
    static CONSTRUCTOR_ID = SAVED_MUSIC_NOT_MODIFIED_ID;
    static fromReader(reader: { readInt: () => number }) {
      const count = reader.readInt();
      return { className: "users.savedMusicNotModified", count };
    }
  }

  class SavedMusic {
    static CONSTRUCTOR_ID = SAVED_MUSIC_ID;
    static fromReader(reader: {
      readInt: () => number;
      tgReadObject: () => Api.Document;
    }) {
      const count = reader.readInt();
      reader.readInt();
      const len = reader.readInt();
      const documents: Api.Document[] = [];
      for (let i = 0; i < len; i++) {
        documents.push(reader.tgReadObject());
      }
      return { className: "users.savedMusic", count, documents };
    }
  }

  tlobjects[SAVED_MUSIC_ID] = SavedMusic;
  tlobjects[SAVED_MUSIC_NOT_MODIFIED_ID] = SavedMusicNotModified;
}

function serializeVector(items: Buffer[]): Buffer {
  const header = Buffer.alloc(8); // 4-byte constructor id + 4-byte element count
  header.writeUInt32LE(VECTOR_CONSTRUCTOR_ID, 0);
  header.writeInt32LE(items.length, 4); // vector length field offset
  return Buffer.concat([header, ...items]);
}

function constructorHeader(constructorId: number): Buffer {
  const header = Buffer.alloc(4); // 32-bit TL constructor id
  header.writeUInt32LE(constructorId, 0);
  return header;
}

function int32LE(value: number): Buffer {
  const buffer = Buffer.alloc(4);
  buffer.writeInt32LE(value, 0);
  return buffer;
}

/** Shared read/resolve hooks; saved-music requests just echo the decoded object. */
const baseRequest = {
  readResult(reader: { tgReadObject: () => unknown }) {
    return reader.tgReadObject();
  },
  async resolve() {
    // InputUserSelf needs no resolution.
  },
} satisfies Pick<SavedMusicTlRequest, "readResult" | "resolve">;

interface GetSavedMusicParams {
  offset: number;
  limit: number;
  /** "0" forces a fetch; a non-zero hash lets the server short-circuit when unchanged. */
  hash?: string | BigInteger;
}

/** Builds a `users.getSavedMusic` request for the current user. */
function createGetSavedMusicRequest({
  offset,
  limit,
  hash = "0",
}: GetSavedMusicParams): SavedMusicTlRequest {
  const hashInt = typeof hash === "string" ? bigInt(hash) : hash;
  return {
    ...baseRequest,
    CONSTRUCTOR_ID: GET_SAVED_MUSIC_ID,
    SUBCLASS_OF_ID: 0, // TL request base class id (none)
    classType: "request",
    className: "users.GetSavedMusic",
    getBytes() {
      return Buffer.concat([
        constructorHeader(GET_SAVED_MUSIC_ID),
        new Api.InputUserSelf().getBytes(),
        int32LE(offset),
        int32LE(limit),
        toSignedLittleBuffer(hashInt, 8), // 64-bit saved-music hash
      ]);
    },
  };
}

/** Builds a `users.getSavedMusicByID` request for the given documents. */
function createGetSavedMusicByIdRequest(
  documents: Api.InputDocument[],
): SavedMusicTlRequest {
  return {
    ...baseRequest,
    CONSTRUCTOR_ID: GET_SAVED_MUSIC_BY_ID,
    SUBCLASS_OF_ID: 0, // TL request base class id (none)
    classType: "request",
    className: "users.GetSavedMusicByID",
    getBytes() {
      return Buffer.concat([
        constructorHeader(GET_SAVED_MUSIC_BY_ID),
        new Api.InputUserSelf().getBytes(),
        serializeVector(documents.map((doc) => doc.getBytes())),
      ]);
    },
  };
}

/**
 * Invokes a saved-music request, registering the response constructors first
 * and centralizing the single unavoidable cast through GramJS' generic
 * `invoke` signature (it has no type for our custom TL request).
 */
async function invokeSavedMusic(
  client: TelegramClient,
  request: SavedMusicTlRequest,
): Promise<SavedMusicResult> {
  ensureSavedMusicTlRegistered();
  return (await client.invoke(request as never)) as SavedMusicResult;
}

/** Fetches a page of the current user's profile (saved) music. */
export function getSavedMusic(
  client: TelegramClient,
  params: GetSavedMusicParams,
): Promise<SavedMusicResult> {
  return invokeSavedMusic(client, createGetSavedMusicRequest(params));
}

/** Re-fetches specific saved-music documents by their input document refs. */
export function getSavedMusicByID(
  client: TelegramClient,
  documents: Api.InputDocument[],
): Promise<SavedMusicResult> {
  return invokeSavedMusic(client, createGetSavedMusicByIdRequest(documents));
}
