use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::AppError;

pub struct SettingsRepo;

impl SettingsRepo {
    pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| AppError::DbError {
            message: error.to_string(),
        })
    }

    pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
        conn.execute(
            "INSERT INTO settings (key, value, created_at, updated_at)
             VALUES (?1, ?2, datetime('now'), datetime('now'))
             ON CONFLICT(key) DO UPDATE SET
               value = excluded.value,
               updated_at = datetime('now')",
            params![key, value],
        )
        .map_err(|error| AppError::DbError {
            message: error.to_string(),
        })?;
        Ok(())
    }

    /// 删除 key 以 prefix 开头的全部记录（SQL LIKE，prefix 内 `%`/`_` 按字面需自行转义）。
    pub fn delete_prefix(conn: &Connection, prefix: &str) -> Result<(), AppError> {
        let pattern = format!("{prefix}%");
        conn.execute("DELETE FROM settings WHERE key LIKE ?1 ESCAPE '\\'", params![pattern])
            .map_err(|error| AppError::DbError {
                message: error.to_string(),
            })?;
        Ok(())
    }
}
