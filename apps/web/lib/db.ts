import Database from "better-sqlite3";
import fs from "node:fs";
import path from "node:path";

const DATA_DIR = path.join(process.cwd(), "data");
const DB_PATH = path.join(DATA_DIR, "soundgrammy.db");

function createDatabase(): Database.Database {
  fs.mkdirSync(DATA_DIR, { recursive: true });
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
      created_at TEXT NOT NULL DEFAULT (datetime('now'))
    )
  `);

  db.exec(`
    CREATE INDEX IF NOT EXISTS idx_tracks_tg_user_id ON tracks (tg_user_id)
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

export interface Track {
  id: number;
  tg_user_id: number;
  file_id: string;
  file_unique_id: string;
  title: string | null;
  performer: string | null;
  duration: number | null;
  created_at: string;
}

export function getTracksByUser(tgUserId: number): Track[] {
  const db = getDb();
  return db
    .prepare("SELECT * FROM tracks WHERE tg_user_id = ? ORDER BY created_at DESC")
    .all(tgUserId) as Track[];
}

export function getTrackByFileId(fileId: string, tgUserId: number): Track | undefined {
  const db = getDb();
  return db
    .prepare("SELECT * FROM tracks WHERE file_id = ? AND tg_user_id = ?")
    .get(fileId, tgUserId) as Track | undefined;
}

export function insertTrack(track: Omit<Track, "id" | "created_at">): Track {
  const db = getDb();
  const stmt = db.prepare(`
    INSERT INTO tracks (tg_user_id, file_id, file_unique_id, title, performer, duration)
    VALUES (@tg_user_id, @file_id, @file_unique_id, @title, @performer, @duration)
    ON CONFLICT (file_unique_id) DO UPDATE SET
      file_id = @file_id,
      title = @title,
      performer = @performer,
      duration = @duration
  `);
  const result = stmt.run(track);
  return db
    .prepare("SELECT * FROM tracks WHERE id = ?")
    .get(result.lastInsertRowid) as Track;
}
