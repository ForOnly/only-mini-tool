use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};

use tauri::AppHandle;

use crate::errors::AppError;
use crate::infrastructure::filesystem;

const DEFAULT_TAIL_BYTES: u64 = 256 * 1024;

pub struct LogService;

impl LogService {
    pub fn read_tail(app: &AppHandle, max_bytes: Option<u64>) -> Result<String, AppError> {
        let path = filesystem::log_file_path(app)?;
        if !path.exists() {
            return Ok(String::new());
        }
        let limit = max_bytes.unwrap_or(DEFAULT_TAIL_BYTES).max(1024);
        let mut file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|error| AppError::InternalError {
                message: format!("read log: {error}"),
            })?;
        let len = file
            .seek(SeekFrom::End(0))
            .map_err(|error| AppError::InternalError {
                message: format!("seek log: {error}"),
            })?;
        let start = len.saturating_sub(limit);
        file.seek(SeekFrom::Start(start))
            .map_err(|error| AppError::InternalError {
                message: format!("seek log start: {error}"),
            })?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(|error| AppError::InternalError {
                message: format!("read log body: {error}"),
            })?;
        if start > 0 {
            // 可能截断在多字节中间，跳到下一行
            if let Some(pos) = buf.find('\n') {
                buf = buf[pos + 1..].to_string();
            }
            buf.insert_str(0, "…\n");
        }
        Ok(buf)
    }

    pub fn clear(app: &AppHandle) -> Result<(), AppError> {
        let path = filesystem::log_file_path(app)?;
        if path.exists() {
            fs::write(&path, "").map_err(|error| AppError::InternalError {
                message: format!("clear log: {error}"),
            })?;
        }
        tracing::info!("app log cleared");
        Ok(())
    }

    pub fn logs_dir(app: &AppHandle) -> Result<String, AppError> {
        filesystem::ensure_dirs(app)?;
        Ok(filesystem::logs_dir(app)?.to_string_lossy().into_owned())
    }

    pub fn open_logs_dir(app: &AppHandle) -> Result<(), AppError> {
        let dir = filesystem::logs_dir(app)?;
        filesystem::ensure_dirs(app)?;
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(&dir)
                .spawn()
                .map_err(|error| AppError::InternalError {
                    message: format!("open logs dir: {error}"),
                })?;
            return Ok(());
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = dir;
            Err(AppError::InternalError {
                message: "open logs dir is only supported on Windows in v1".into(),
            })
        }
    }
}
