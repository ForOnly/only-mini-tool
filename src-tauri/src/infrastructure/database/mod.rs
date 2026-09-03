mod migrations;

use std::sync::Mutex;

use rusqlite::Connection;
use tauri::AppHandle;

use crate::errors::AppError;
use crate::infrastructure::filesystem;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(app: &AppHandle) -> Result<Self, AppError> {
        filesystem::ensure_dirs(app)?;
        let db_path = filesystem::app_data_dir(app)?.join("data.db");
        let conn = Connection::open(db_path).map_err(|error| AppError::DbError {
            message: error.to_string(),
        })?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|error| AppError::DbError {
                message: error.to_string(),
            })?;
        migrations::run_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn with_conn<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>,
    {
        let conn = self.conn.lock().map_err(|_| AppError::InternalError {
            message: "database lock poisoned".into(),
        })?;
        f(&conn)
    }

    #[allow(dead_code)]
    pub fn with_tx<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&Connection) -> Result<T, AppError>,
    {
        let conn = self.conn.lock().map_err(|_| AppError::InternalError {
            message: "database lock poisoned".into(),
        })?;
        let tx = conn
            .unchecked_transaction()
            .map_err(|error| AppError::DbError {
                message: format!("transaction begin failed: {error}"),
            })?;
        match f(&tx) {
            Ok(value) => {
                tx.commit().map_err(|error| AppError::DbError {
                    message: format!("transaction commit failed: {error}"),
                })?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback();
                Err(error)
            }
        }
    }
}
