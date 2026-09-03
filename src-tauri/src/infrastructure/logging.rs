//! 应用日志：写入 `{app_data_dir}/logs/app.log`。

use std::fs::OpenOptions;
use std::sync::Mutex;

use tauri::AppHandle;
use tracing_subscriber::EnvFilter;

use crate::errors::AppError;
use crate::infrastructure::filesystem;

/// 初始化 tracing。`debug_enabled` 为 true 时输出 DEBUG，否则 INFO。
pub fn init(app: &AppHandle, debug_enabled: bool) -> Result<(), AppError> {
    filesystem::ensure_dirs(app)?;
    let log_file = filesystem::log_file_path(app)?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .map_err(|error| AppError::InternalError {
            message: format!("open log file: {error}"),
        })?;

    let filter = if debug_enabled {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_target(true)
        .try_init()
        .map_err(|error| AppError::InternalError {
            message: format!("init tracing: {error}"),
        })?;

    tracing::info!(
        debug_enabled,
        path = %log_file.display(),
        "logging initialized"
    );
    Ok(())
}
