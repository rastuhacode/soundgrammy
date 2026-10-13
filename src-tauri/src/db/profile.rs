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
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM profile", [])?;
        tx.execute(
            "INSERT INTO profile (tg_user_id, first_name, last_name, username, phone) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT (tg_user_id) DO UPDATE SET \
               first_name = excluded.first_name, last_name = excluded.last_name, \
               username = excluded.username, phone = excluded.phone",
            params![tg_user_id, first_name, last_name, username, phone],
        )?;
        tx.execute(
            "INSERT INTO app_settings (key, value) VALUES ('account_generation', '1') \
            ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + 1",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn account_generation(&self) -> AppResult<i64> {
        self.get_setting_i64("account_generation", 0)
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

    /// Refresh display fields without recreating a profile cleared by logout.
    #[cfg(test)]
    pub fn refresh_active_profile(
        &self,
        tg_user_id: i64,
        first_name: &str,
        last_name: Option<&str>,
        username: Option<&str>,
        phone: Option<&str>,
    ) -> AppResult<()> {
        self.refresh_profile_at_generation(
            tg_user_id,
            self.account_generation()?,
            first_name,
            last_name,
            username,
            phone,
        )
    }

    pub fn refresh_profile_at_generation(
        &self,
        tg_user_id: i64,
        generation: i64,
        first_name: &str,
        last_name: Option<&str>,
        username: Option<&str>,
        phone: Option<&str>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let updated = conn.execute(
            "UPDATE profile SET first_name = ?2, last_name = ?3, username = ?4, phone = ?5 \
             WHERE tg_user_id = ?1 AND tg_user_id = (SELECT tg_user_id FROM profile LIMIT 1) \
             AND ?6 = COALESCE((SELECT CAST(value AS INTEGER) FROM app_settings WHERE key = 'account_generation'), 0)",
            params![tg_user_id, first_name, last_name, username, phone, generation],
        )?;
        if updated == 0 {
            return Err(crate::error::AppError::NotAuthorized);
        }
        Ok(())
    }

    pub fn clear_active_profile(&self, tg_user_id: i64) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM profile WHERE tg_user_id = ?1",
            params![tg_user_id],
        )?;
        tx.execute(
            "INSERT INTO app_settings (key, value) VALUES ('account_generation', '1') \
            ON CONFLICT(key) DO UPDATE SET value = CAST(value AS INTEGER) + 1",
            [],
        )?;
        tx.commit()?;
        Ok(())
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
