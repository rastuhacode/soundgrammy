import Database from "better-sqlite3";
import fs from "node:fs";
import path from "node:path";

const DATA_DIR = path.join(process.cwd(), "data");
const DB_PATH = path.join(DATA_DIR, "soundgrammy.db");

function ensureWritableDataDir() {
  fs.mkdirSync(DATA_DIR, { recursive: true });
  const probe = path.join(DATA_DIR, ".write-test");
  try {
    fs.writeFileSync(probe, "");
    fs.unlinkSync(probe);
  } catch {
    throw new Error(
      `Database directory is not writable: ${DATA_DIR}. If you previously ran the app with sudo, run: sudo chown -R $(whoami) ${DATA_DIR}`,
    );
  }
}

function createDatabase(): Database.Database {
  ensureWritableDataDir();
  const db = new Database(DB_PATH);

  db.pragma("journal_mode = WAL");
  db.pragma("foreign_keys = ON");

  db.exec(`
    CREATE TABLE IF NOT EXISTS tracks (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      tg_user_id INTEGER NOT NULL,
      file_id TEXT NOT NULL,
      file_unique_id TEXT NOT NULL UNIQUE,
      title TEXT,
      performer TEXT,
      duration INTEGER,
      source TEXT NOT NULL DEFAULT 'bot',
      mime_type TEXT,
      mtproto_document TEXT,
      created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
  `);

  db.exec(`
    CREATE INDEX IF NOT EXISTS idx_tracks_tg_user_id ON tracks (tg_user_id)
  `);

  db.exec(`
    CREATE TABLE IF NOT EXISTS mtproto_sessions (
      tg_user_id INTEGER PRIMARY KEY,
      session_data TEXT NOT NULL,
      phone_number TEXT,
      created_at TEXT NOT NULL DEFAULT (datetime('now')),
      updated_at TEXT NOT NULL DEFAULT (datetime('now')),
      last_sync_at TEXT,
      saved_music_hash TEXT
    )
  `);

  db.exec(`
    CREATE TABLE IF NOT EXISTS mtproto_auth_pending (
      auth_token TEXT PRIMARY KEY,
      tg_user_id INTEGER NOT NULL,
      phone_number TEXT NOT NULL,
      phone_code_hash TEXT NOT NULL,
      session_data TEXT NOT NULL,
      created_at TEXT NOT NULL DEFAULT (datetime('now')),
      expires_at TEXT NOT NULL
    )
  `);

  db.exec(`
    CREATE TABLE IF NOT EXISTS playlists (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      tg_user_id INTEGER NOT NULL,
      name TEXT NOT NULL,
      kind TEXT NOT NULL CHECK (kind IN ('liked', 'custom')),
      thumbnail_data TEXT,
      thumbnail_mime TEXT,
      created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
  `);

  db.exec(`
    CREATE UNIQUE INDEX IF NOT EXISTS idx_playlists_user_liked
    ON playlists (tg_user_id) WHERE kind = 'liked'
  `);

  db.exec(`
    CREATE TABLE IF NOT EXISTS playlist_tracks (
      playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
      track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
      position INTEGER NOT NULL DEFAULT 0,
      added_at TEXT NOT NULL DEFAULT (datetime('now')),
      PRIMARY KEY (playlist_id, track_id)
    )
  `);

  return db;
}

let dbInstance: Database.Database | null = null;

export function getDb(): Database.Database {
  if (!dbInstance) {
    dbInstance = createDatabase();
  }
  return dbInstance;
}

export interface BotFile {
  file_id: string;
  file_unique_id: string;
  file_size: number;
}
export interface BotThumbnail extends BotFile {
  width: number;
  height: number;
}

export interface BotAudioPayload extends BotFile {
  tg_user_id: number;
  duration: number;
  file_name: string;
  mime_type: string;
  title: string;
  performer: string;
  thumbnail?: BotThumbnail;
  thumb?: BotThumbnail;
}

export type Track = BotAudioPayload & {
  id: number;
  created_at: string;
  source: "mtproto";
  mtproto_document?: string | null;
};

export function getTracksByUser(tgUserId: number): Track[] {
  const db = getDb();
  return db
    .prepare(
      "SELECT * FROM tracks WHERE tg_user_id = ? ORDER BY created_at DESC",
    )
    .all(tgUserId) as Track[];
}

export function getTrackById(id: number, tgUserId: number): Track | undefined {
  const db = getDb();
  return db
    .prepare("SELECT * FROM tracks WHERE id = ? AND tg_user_id = ?")
    .get(id, tgUserId) as Track | undefined;
}

export function getTrackByFileId(
  fileId: string,
  tgUserId: number,
): Track | undefined {
  const db = getDb();
  return db
    .prepare("SELECT * FROM tracks WHERE file_id = ? AND tg_user_id = ?")
    .get(fileId, tgUserId) as Track | undefined;
}

export interface InsertTrackInput {
  tg_user_id: number;
  file_id: string;
  file_unique_id: string;
  title?: string | null;
  performer?: string | null;
  duration?: number | null;
  source?: "mtproto";
  mime_type?: string | null;
  mtproto_document?: string | null;
  file_name?: string;
  file_size?: number;
}

export function insertTrack(track: InsertTrackInput) {
  const db = getDb();
  const stmt = db.prepare(`
    INSERT INTO tracks (
      tg_user_id, file_id, file_unique_id, title, performer, duration,
      source, mime_type, mtproto_document
    )
    VALUES (
      @tg_user_id, @file_id, @file_unique_id, @title, @performer, @duration,
      @source, @mime_type, @mtproto_document
    )
    ON CONFLICT (file_unique_id) DO UPDATE SET
      file_id = @file_id,
      title = @title,
      performer = @performer,
      duration = @duration,
      mime_type = @mime_type,
      mtproto_document = @mtproto_document
  `);
  const result = stmt.run({
    ...track,
    source: track.source ?? "bot",
    mime_type: track.mime_type ?? null,
    mtproto_document: track.mtproto_document ?? null,
  });
  return db
    .prepare<[number | bigint], Track>("SELECT * FROM tracks WHERE id = ?")
    .get(result.lastInsertRowid)!;
}

export function deleteTrack(id: number, tgUserId?: number) {
  const db = getDb();
  if (tgUserId !== undefined) {
    db.prepare("DELETE FROM tracks WHERE id = ? AND tg_user_id = ?").run(
      id,
      tgUserId,
    );
    return;
  }
  db.prepare("DELETE FROM tracks WHERE id = ?").run(id);
}

export interface MtprotoSession {
  tg_user_id: number;
  session_data: string;
  phone_number: string | null;
  created_at: string;
  updated_at: string;
  last_sync_at: string | null;
  saved_music_hash: string | null;
}

export function getMtprotoSession(
  tgUserId: number,
): MtprotoSession | undefined {
  const db = getDb();
  return db
    .prepare("SELECT * FROM mtproto_sessions WHERE tg_user_id = ?")
    .get(tgUserId) as MtprotoSession | undefined;
}

export function saveMtprotoSession(
  tgUserId: number,
  sessionData: string,
  phoneNumber: string,
) {
  const db = getDb();
  db.prepare(
    `
    INSERT INTO mtproto_sessions (tg_user_id, session_data, phone_number, updated_at)
    VALUES (?, ?, ?, datetime('now'))
    ON CONFLICT (tg_user_id) DO UPDATE SET
      session_data = excluded.session_data,
      phone_number = excluded.phone_number,
      updated_at = datetime('now')
  `,
  ).run(tgUserId, sessionData, phoneNumber);
}

export function updateMtprotoLastSync(tgUserId: number) {
  const db = getDb();
  db.prepare(
    "UPDATE mtproto_sessions SET last_sync_at = datetime('now') WHERE tg_user_id = ?",
  ).run(tgUserId);
}

export function updateMtprotoSavedMusicHash(
  tgUserId: number,
  savedMusicHash: string,
) {
  const db = getDb();
  db.prepare(
    "UPDATE mtproto_sessions SET saved_music_hash = ? WHERE tg_user_id = ?",
  ).run(savedMusicHash, tgUserId);
}

export function updateTrackMtprotoDocument(
  trackId: number,
  tgUserId: number,
  mtprotoDocument: string,
) {
  const db = getDb();
  db.prepare(
    "UPDATE tracks SET mtproto_document = ? WHERE id = ? AND tg_user_id = ?",
  ).run(mtprotoDocument, trackId, tgUserId);
}

export function deleteMtprotoTracksNotIn(
  tgUserId: number,
  fileUniqueIds: string[],
): number {
  const db = getDb();
  if (fileUniqueIds.length === 0) {
    const result = db
      .prepare("DELETE FROM tracks WHERE tg_user_id = ? AND source = 'mtproto'")
      .run(tgUserId);
    return result.changes;
  }

  const placeholders = fileUniqueIds.map(() => "?").join(", ");
  const result = db
    .prepare(
      `DELETE FROM tracks WHERE tg_user_id = ? AND source = 'mtproto' AND file_unique_id NOT IN (${placeholders})`,
    )
    .run(tgUserId, ...fileUniqueIds);
  return result.changes;
}

export function deleteMtprotoSession(tgUserId: number) {
  const db = getDb();
  db.prepare("DELETE FROM mtproto_sessions WHERE tg_user_id = ?").run(tgUserId);
}

export interface MtprotoAuthPending {
  auth_token: string;
  tg_user_id: number;
  phone_number: string;
  phone_code_hash: string;
  session_data: string;
  created_at: string;
  expires_at: string;
}

export function createMtprotoAuthPending(
  authToken: string,
  phoneNumber: string,
  phoneCodeHash: string,
  sessionData: string,
) {
  const db = getDb();
  db.prepare(
    `
    INSERT INTO mtproto_auth_pending (
      auth_token, tg_user_id, phone_number, phone_code_hash, session_data, expires_at
    )
    VALUES (?, 0, ?, ?, ?, datetime('now', '+15 minutes')) -- tg_user_id filled after sign-in; 15m auth window
  `,
  ).run(authToken, phoneNumber, phoneCodeHash, sessionData);
}

export function getMtprotoAuthPending(
  authToken: string,
): MtprotoAuthPending | undefined {
  const db = getDb();
  db.prepare(
    "DELETE FROM mtproto_auth_pending WHERE expires_at < datetime('now')",
  ).run();
  return db
    .prepare("SELECT * FROM mtproto_auth_pending WHERE auth_token = ?")
    .get(authToken) as MtprotoAuthPending | undefined;
}

export function updateMtprotoAuthPendingSession(
  authToken: string,
  sessionData: string,
) {
  const db = getDb();
  db.prepare(
    "UPDATE mtproto_auth_pending SET session_data = ? WHERE auth_token = ?",
  ).run(sessionData, authToken);
}

export function updateMtprotoAuthPending(
  authToken: string,
  phoneCodeHash: string,
  sessionData: string,
) {
  const db = getDb();
  db.prepare(
    `
    UPDATE mtproto_auth_pending
    SET phone_code_hash = ?, session_data = ?, expires_at = datetime('now', '+15 minutes') -- extend 15m auth window on resend
    WHERE auth_token = ?
  `,
  ).run(phoneCodeHash, sessionData, authToken);
}

export function deleteMtprotoAuthPending(authToken: string) {
  const db = getDb();
  db.prepare("DELETE FROM mtproto_auth_pending WHERE auth_token = ?").run(
    authToken,
  );
}

export type PlaylistKind = "liked" | "custom";

export type PlaylistThumbnailMime = "image/jpeg" | "image/png" | "image/webp";

export interface PlaylistThumbnailInput {
  data: string;
  mime: PlaylistThumbnailMime;
}

export interface Playlist {
  id: number;
  tg_user_id: number;
  name: string;
  kind: PlaylistKind;
  thumbnail_data: string | null;
  thumbnail_mime: string | null;
  created_at: string;
}

export interface CustomPlaylistSummary {
  id: number;
  name: string;
  trackIds: number[];
  hasThumbnail: boolean;
}

export interface PlaylistsBundle {
  liked: { id: number; trackIds: number[] };
  custom: CustomPlaylistSummary[];
}

const MAX_PLAYLIST_THUMBNAIL_BYTES = 512 * 1024;

function assertValidPlaylistThumbnail(thumbnail: PlaylistThumbnailInput): void {
  const allowed: PlaylistThumbnailMime[] = [
    "image/jpeg",
    "image/png",
    "image/webp",
  ];
  if (!allowed.includes(thumbnail.mime)) {
    throw new Error("Thumbnail must be JPEG, PNG, or WebP");
  }

  const bytes = Buffer.from(thumbnail.data, "base64");
  if (bytes.length === 0) {
    throw new Error("Thumbnail data is invalid");
  }
  if (bytes.length > MAX_PLAYLIST_THUMBNAIL_BYTES) {
    throw new Error("Thumbnail must be smaller than 512KB");
  }
}

function mapCustomPlaylistSummary(playlist: Playlist): CustomPlaylistSummary {
  return {
    id: playlist.id,
    name: playlist.name,
    trackIds: getPlaylistTrackIdsForPlaylist(playlist.id),
    hasThumbnail: Boolean(playlist.thumbnail_data && playlist.thumbnail_mime),
  };
}

function getPlaylistTrackIdsForPlaylist(playlistId: number): number[] {
  const db = getDb();
  const rows = db
    .prepare(
      `SELECT track_id FROM playlist_tracks
       WHERE playlist_id = ?
       ORDER BY position ASC, added_at ASC`,
    )
    .all(playlistId) as { track_id: number }[];
  return rows.map((row) => row.track_id);
}

function getPlaylistById(
  playlistId: number,
  tgUserId: number,
): Playlist | undefined {
  const db = getDb();
  return db
    .prepare("SELECT * FROM playlists WHERE id = ? AND tg_user_id = ?")
    .get(playlistId, tgUserId) as Playlist | undefined;
}

export function ensureLikedPlaylist(tgUserId: number): Playlist {
  const db = getDb();
  const existing = db
    .prepare(
      "SELECT * FROM playlists WHERE tg_user_id = ? AND kind = 'liked'",
    )
    .get(tgUserId) as Playlist | undefined;

  if (existing) {
    return existing;
  }

  const result = db
    .prepare(
      `INSERT INTO playlists (tg_user_id, name, kind)
       VALUES (?, 'Liked', 'liked')`,
    )
    .run(tgUserId);

  return db
    .prepare<[number | bigint], Playlist>("SELECT * FROM playlists WHERE id = ?")
    .get(result.lastInsertRowid)!;
}

export function getPlaylistsBundle(tgUserId: number): PlaylistsBundle {
  const liked = ensureLikedPlaylist(tgUserId);
  const db = getDb();

  const customPlaylists = db
    .prepare(
      `SELECT * FROM playlists
       WHERE tg_user_id = ? AND kind = 'custom'
       ORDER BY created_at ASC`,
    )
    .all(tgUserId) as Playlist[];

  return {
    liked: {
      id: liked.id,
      trackIds: getPlaylistTrackIdsForPlaylist(liked.id),
    },
    custom: customPlaylists.map(mapCustomPlaylistSummary),
  };
}

export function getPlaylistThumbnail(
  playlistId: number,
  tgUserId: number,
): { data: Buffer; mime: PlaylistThumbnailMime } | null {
  const playlist = getPlaylistById(playlistId, tgUserId);
  if (!playlist?.thumbnail_data || !playlist.thumbnail_mime) {
    return null;
  }

  const mime = playlist.thumbnail_mime as PlaylistThumbnailMime;
  if (
    mime !== "image/jpeg"
    && mime !== "image/png"
    && mime !== "image/webp"
  ) {
    return null;
  }

  return {
    data: Buffer.from(playlist.thumbnail_data, "base64"),
    mime,
  };
}

export function createPlaylist(
  tgUserId: number,
  name: string,
  thumbnail?: PlaylistThumbnailInput,
): CustomPlaylistSummary {
  const db = getDb();
  const trimmedName = name.trim();
  if (!trimmedName) {
    throw new Error("Playlist name is required");
  }

  if (thumbnail) {
    assertValidPlaylistThumbnail(thumbnail);
  }

  const result = db
    .prepare(
      `INSERT INTO playlists (tg_user_id, name, kind, thumbnail_data, thumbnail_mime)
       VALUES (?, ?, 'custom', ?, ?)`,
    )
    .run(
      tgUserId,
      trimmedName,
      thumbnail?.data ?? null,
      thumbnail?.mime ?? null,
    );

  const id = Number(result.lastInsertRowid);
  return {
    id,
    name: trimmedName,
    trackIds: [],
    hasThumbnail: Boolean(thumbnail),
  };
}

export function updatePlaylist(
  playlistId: number,
  tgUserId: number,
  input: {
    name?: string;
    thumbnail?: PlaylistThumbnailInput | null;
  },
): CustomPlaylistSummary {
  const playlist = getPlaylistById(playlistId, tgUserId);
  if (!playlist) {
    throw new Error("Playlist not found");
  }
  if (playlist.kind !== "custom") {
    throw new Error("Only custom playlists can be edited");
  }

  const nextName = input.name !== undefined ? input.name.trim() : playlist.name;
  if (!nextName) {
    throw new Error("Playlist name is required");
  }

  let nextThumbnailData = playlist.thumbnail_data;
  let nextThumbnailMime = playlist.thumbnail_mime;

  if (input.thumbnail === null) {
    nextThumbnailData = null;
    nextThumbnailMime = null;
  } else if (input.thumbnail) {
    assertValidPlaylistThumbnail(input.thumbnail);
    nextThumbnailData = input.thumbnail.data;
    nextThumbnailMime = input.thumbnail.mime;
  }

  const db = getDb();
  db.prepare(
    `UPDATE playlists
     SET name = ?, thumbnail_data = ?, thumbnail_mime = ?
     WHERE id = ? AND tg_user_id = ?`,
  ).run(
    nextName,
    nextThumbnailData,
    nextThumbnailMime,
    playlistId,
    tgUserId,
  );

  const updated = getPlaylistById(playlistId, tgUserId);
  if (!updated) {
    throw new Error("Playlist not found");
  }

  return mapCustomPlaylistSummary(updated);
}

export function deletePlaylist(playlistId: number, tgUserId: number): void {
  const playlist = getPlaylistById(playlistId, tgUserId);
  if (!playlist) {
    throw new Error("Playlist not found");
  }
  if (playlist.kind === "liked") {
    throw new Error("Cannot delete the Liked playlist");
  }

  const db = getDb();
  db.prepare("DELETE FROM playlists WHERE id = ? AND tg_user_id = ?").run(
    playlistId,
    tgUserId,
  );
}

function getNextPlaylistTrackPosition(playlistId: number): number {
  const db = getDb();
  const row = db
    .prepare(
      "SELECT COALESCE(MAX(position), -1) + 1 AS next_position FROM playlist_tracks WHERE playlist_id = ?",
    )
    .get(playlistId) as { next_position: number };
  return row.next_position;
}

export function addTrackToPlaylist(
  playlistId: number,
  trackId: number,
  tgUserId: number,
): void {
  const playlist = getPlaylistById(playlistId, tgUserId);
  if (!playlist) {
    throw new Error("Playlist not found");
  }
  if (playlist.kind === "liked") {
    throw new Error("Use toggleLike for the Liked playlist");
  }

  const track = getTrackById(trackId, tgUserId);
  if (!track) {
    throw new Error("Track not found");
  }

  const db = getDb();
  const position = getNextPlaylistTrackPosition(playlistId);
  db.prepare(
    `INSERT INTO playlist_tracks (playlist_id, track_id, position)
     VALUES (?, ?, ?)
     ON CONFLICT (playlist_id, track_id) DO NOTHING`,
  ).run(playlistId, trackId, position);
}

export function removeTrackFromPlaylist(
  playlistId: number,
  trackId: number,
  tgUserId: number,
): void {
  const playlist = getPlaylistById(playlistId, tgUserId);
  if (!playlist) {
    throw new Error("Playlist not found");
  }

  const db = getDb();
  db.prepare(
    "DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
  ).run(playlistId, trackId);
}

export function isTrackInPlaylist(
  playlistId: number,
  trackId: number,
): boolean {
  const db = getDb();
  const row = db
    .prepare(
      "SELECT 1 FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
    )
    .get(playlistId, trackId);
  return Boolean(row);
}

export function toggleLikedTrack(
  trackId: number,
  tgUserId: number,
): { liked: boolean; trackIds: number[] } {
  const likedPlaylist = ensureLikedPlaylist(tgUserId);
  const track = getTrackById(trackId, tgUserId);
  if (!track) {
    throw new Error("Track not found");
  }

  const db = getDb();
  const alreadyLiked = isTrackInPlaylist(likedPlaylist.id, trackId);

  if (alreadyLiked) {
    db.prepare(
      "DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
    ).run(likedPlaylist.id, trackId);
  } else {
    const position = getNextPlaylistTrackPosition(likedPlaylist.id);
    db.prepare(
      `INSERT INTO playlist_tracks (playlist_id, track_id, position)
       VALUES (?, ?, ?)`,
    ).run(likedPlaylist.id, trackId, position);
  }

  return {
    liked: !alreadyLiked,
    trackIds: getPlaylistTrackIdsForPlaylist(likedPlaylist.id),
  };
}
