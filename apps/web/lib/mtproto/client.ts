/**
 * Barrel for the MTProto client surface. The implementation is split across
 * focused modules; this file preserves the historical `lib/mtproto/client`
 * import path and gives callers a single entry point for session management
 * and document downloads.
 */

export {
  createMtprotoClient,
  saveClientSession,
  withMtprotoClient,
} from "./session";

export {
  createMtprotoDocumentStream,
  downloadMtprotoDocumentThumbnail,
  type MtprotoDocumentStream,
} from "./download";

export { parseStoredDocument, type StoredDocument } from "./document";
