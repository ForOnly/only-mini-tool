use rusqlite::{Connection, OptionalExtension};

use crate::errors::AppError;

struct Migration {
    version: i32,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../../migrations/001_settings.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../../migrations/002_settings_pk_audit.sql"),
    },
];

fn current_version(conn: &Connection) -> Result<i32, AppError> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_version'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| AppError::DbError {
            message: error.to_string(),
        })?
        .is_some();

    if !exists {
        return Ok(0);
    }

    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
    .map_err(|error| AppError::DbError {
        message: error.to_string(),
    })
}

pub fn run_migrations(conn: &Connection) -> Result<(), AppError> {
    let mut version = current_version(conn)?;
    for migration in MIGRATIONS {
        if migration.version <= version {
            continue;
        }
        conn.execute_batch(migration.sql)
            .map_err(|error| AppError::DbError {
                message: format!("migration {} failed: {error}", migration.version),
            })?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, datetime('now'))",
            [migration.version],
        )
        .map_err(|error| AppError::DbError {
            message: format!("record migration {}: {error}", migration.version),
        })?;
        version = migration.version;
    }
    Ok(())
}
