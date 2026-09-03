use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::errors::AppError;

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map_err(|error| AppError::InternalError {
            message: format!("failed to resolve app data dir: {error}"),
        })
}

pub fn app_cache_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_cache_dir()
        .map_err(|error| AppError::InternalError {
            message: format!("failed to resolve app cache dir: {error}"),
        })
}

pub fn ocr_temp_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(app_cache_dir(app)?.join("ocr"))
}

pub fn logs_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(app_data_dir(app)?.join("logs"))
}

pub fn log_file_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    Ok(logs_dir(app)?.join("app.log"))
}

pub fn ensure_dirs(app: &AppHandle) -> Result<(), AppError> {
    let data = app_data_dir(app)?;
    let logs = logs_dir(app)?;
    let ocr = ocr_temp_dir(app)?;
    std::fs::create_dir_all(&data).map_err(|error| AppError::InternalError {
        message: format!("failed to create data dir: {error}"),
    })?;
    std::fs::create_dir_all(&logs).map_err(|error| AppError::InternalError {
        message: format!("failed to create logs dir: {error}"),
    })?;
    std::fs::create_dir_all(&ocr).map_err(|error| AppError::InternalError {
        message: format!("failed to create ocr temp dir: {error}"),
    })?;
    Ok(())
}
