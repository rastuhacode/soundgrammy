use rusqlite::{params, OptionalExtension};

use super::{Db, Profile};
use crate::error::AppResult;

impl Db {
    pub fn save_profile(
        &self,
        tg_user_id: i64,
        first_name: &str,
        last_name: Option<&str>,
        username: Option<&str>,
        phone: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO profile (tg_user_id, first_name, last_name, username, phone) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (tg_user_id) DO UPDATE SET \
               first_name = excluded.first_name, last_name = excluded.last_name, \
               username = excluded.username, phone = excluded.phone",
            params![tg_user_id, first_name, last_name, username, phone],
        )?;
        Ok(())
    }

    pub fn load_profile(&self) -> AppResult<Option<Profile>> {
        let conn = self.conn.lock().unwrap();
        let profile = conn
            .query_row(
                "SELECT tg_user_id, first_name, last_name, username, phone FROM profile LIMIT 1",
                [],
                |row| {
                    Ok(Profile {
                        tg_user_id: row.get(0)?,
                        first_name: row.get(1)?,
                        last_name: row.get(2)?,
                        username: row.get(3)?,
                        phone: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(profile)
    }

    pub fn clear_active_profile(&self, tg_user_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM profile WHERE tg_user_id = ?1",
            params![tg_user_id],
        )?;
        Ok(())
    }

    pub fn saved_music_hash(&self, tg_user_id: i64) -> AppResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let hash = conn
            .query_row(
                "SELECT saved_music_hash FROM app_meta WHERE tg_user_id = ?1",
                params![tg_user_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(hash)
    }

    pub fn last_sync_at(&self, tg_user_id: i64) -> AppResult<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let value = conn
            .query_row(
                "SELECT last_sync_at FROM app_meta WHERE tg_user_id = ?1",
                params![tg_user_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(value)
    }

    pub fn set_saved_music_hash(&self, tg_user_id: i64, hash: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_meta (tg_user_id, saved_music_hash) VALUES (?1, ?2) \
             ON CONFLICT (tg_user_id) DO UPDATE SET saved_music_hash = excluded.saved_music_hash",
            params![tg_user_id, hash],
        )?;
        Ok(())
    }

    pub fn mark_synced(&self, tg_user_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO app_meta (tg_user_id, last_sync_at) VALUES (?1, datetime('now')) \
             ON CONFLICT (tg_user_id) DO UPDATE SET last_sync_at = datetime('now')",
            params![tg_user_id],
        )?;
        Ok(())
    }
}
