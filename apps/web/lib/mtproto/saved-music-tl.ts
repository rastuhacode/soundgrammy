import bigInt, { type BigInteger } from "big-integer";
import { Api } from "telegram";
import { toSignedLittleBuffer } from "telegram/Helpers";
import { tlobjects } from "telegram/tl/AllTLObjects";

const SAVED_MUSIC_ID = 0x34a2f297;
const SAVED_MUSIC_NOT_MODIFIED_ID = 0xe3878aa4;
const GET_SAVED_MUSIC_ID = 0x788d7fe3;
const GET_SAVED_MUSIC_BY_ID = 0x7573a4e9;
const VECTOR_CONSTRUCTOR_ID = 0x1cb5c415;

let registered = false;

export function registerSavedMusicTl() {
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
  const header = Buffer.alloc(8);
  header.writeUInt32LE(VECTOR_CONSTRUCTOR_ID, 0);
  header.writeInt32LE(items.length, 4);
  return Buffer.concat([header, ...items]);
}

export class GetSavedMusicRequest {
  CONSTRUCTOR_ID = GET_SAVED_MUSIC_ID;
  SUBCLASS_OF_ID = 0;
  classType = "request" as const;
  className = "users.GetSavedMusic";
  id = new Api.InputUserSelf();
  offset: number;
  limit: number;
  hash: BigInteger;

  constructor({
    offset,
    limit,
    hash = "0",
  }: {
    offset: number;
    limit: number;
    hash?: string | BigInteger;
  }) {
    this.offset = offset;
    this.limit = limit;
    this.hash = typeof hash === "string" ? bigInt(hash) : hash;
  }

  getBytes() {
    const header = Buffer.alloc(4);
    header.writeUInt32LE(this.CONSTRUCTOR_ID, 0);
    const offsetBuf = Buffer.alloc(4);
    offsetBuf.writeInt32LE(this.offset, 0);
    const limitBuf = Buffer.alloc(4);
    limitBuf.writeInt32LE(this.limit, 0);
    return Buffer.concat([
      header,
      new Api.InputUserSelf().getBytes(),
      offsetBuf,
      limitBuf,
      toSignedLittleBuffer(this.hash, 8),
    ]);
  }

  readResult(reader: { tgReadObject: () => unknown }) {
    return reader.tgReadObject();
  }

  async resolve() {
    // InputUserSelf needs no resolution
  }
}

export class GetSavedMusicByIDRequest {
  CONSTRUCTOR_ID = GET_SAVED_MUSIC_BY_ID;
  SUBCLASS_OF_ID = 0;
  classType = "request" as const;
  className = "users.GetSavedMusicByID";
  id = new Api.InputUserSelf();
  documents: Api.InputDocument[];

  constructor(documents: Api.InputDocument[]) {
    this.documents = documents;
  }

  getBytes() {
    const header = Buffer.alloc(4);
    header.writeUInt32LE(this.CONSTRUCTOR_ID, 0);
    return Buffer.concat([
      header,
      new Api.InputUserSelf().getBytes(),
      serializeVector(this.documents.map((doc) => doc.getBytes())),
    ]);
  }

  readResult(reader: { tgReadObject: () => unknown }) {
    return reader.tgReadObject();
  }

  async resolve() {
    // InputUserSelf needs no resolution
  }
}

export type SavedMusicResult =
  | { className: "users.savedMusic"; count: number; documents: Api.Document[] }
  | { className: "users.savedMusicNotModified"; count: number };
