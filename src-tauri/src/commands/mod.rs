use tauri::{Manager, State};

use crate::appearance::{self, AppearanceHost};
use crate::domain::{
    AppearanceDto, OcrResult, OcrSettingsBundle, OcrSettingsSave, SETTING_DEBUG_ENABLED,
    SETTING_UI_THEME,
};
use crate::errors::AppError;
use crate::services::log_service::LogService;
use crate::services::ocr::OcrService;
use crate::services::settings_service::SettingsService;
use crate::state::AppState;

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, AppError> {
    SettingsService::get(&state.db, &key)
}

#[tauri::command]
pub fn set_setting(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    SettingsService::set(&state.db, &key, &value)?;
    if key == SETTING_UI_THEME {
        appearance::sync_from_settings(&app);
    }
    if key == SETTING_DEBUG_ENABLED {
        tracing::info!(enabled = value, "debug setting changed (restart for full log level)");
    }
    Ok(())
}

#[tauri::command]
pub fn get_appearance(host: State<'_, AppearanceHost>) -> Result<AppearanceDto, AppError> {
    Ok(host.snapshot())
}

#[tauri::command]
pub fn read_app_log(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    max_bytes: Option<u64>,
) -> Result<String, AppError> {
    require_debug(&state)?;
    LogService::read_tail(&app, max_bytes)
}

#[tauri::command]
pub fn clear_app_log(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    require_debug(&state)?;
    LogService::clear(&app)
}

#[tauri::command]
pub fn get_logs_dir(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, AppError> {
    require_debug(&state)?;
    LogService::logs_dir(&app)
}

#[tauri::command]
pub fn open_logs_dir(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    require_debug(&state)?;
    LogService::open_logs_dir(&app)
}

#[tauri::command]
pub fn open_devtools(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    require_debug(&state)?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::InternalError {
            message: "main window not found".into(),
        })?;
    window.open_devtools();
    tracing::info!("devtools opened");
    Ok(())
}

#[tauri::command]
pub async fn recognize_image(
    state: State<'_, AppState>,
    path: String,
) -> Result<OcrResult, AppError> {
    OcrService::recognize(&state.db, &path).await
}

#[tauri::command]
pub fn cancel_recognize() -> Result<(), AppError> {
    OcrService::cancel_recognize();
    Ok(())
}

#[tauri::command]
pub fn get_ocr_settings(state: State<'_, AppState>) -> Result<OcrSettingsBundle, AppError> {
    OcrService::get_settings(&state.db)
}

#[tauri::command]
pub fn save_ocr_settings(
    state: State<'_, AppState>,
    payload: OcrSettingsSave,
) -> Result<(), AppError> {
    OcrService::save_settings(&state.db, payload)
}

#[tauri::command]
pub fn stage_image_file(app: tauri::AppHandle, path: String) -> Result<String, AppError> {
    OcrService::stage_image_file(&app, &path)
}

#[tauri::command]
pub fn stage_image_bytes(
    app: tauri::AppHandle,
    bytes: Vec<u8>,
    ext: String,
) -> Result<String, AppError> {
    OcrService::stage_image_bytes(&app, bytes, ext)
}

#[tauri::command]
pub fn save_clipboard_image(app: tauri::AppHandle) -> Result<String, AppError> {
    OcrService::save_clipboard_image(&app)
}

#[tauri::command]
pub fn rotate_image_orientation(
    app: tauri::AppHandle,
    path: String,
    degrees: i32,
) -> Result<String, AppError> {
    OcrService::rotate_image_orientation(&app, &path, degrees)
}

#[tauri::command]
pub fn clear_ocr_temp(app: tauri::AppHandle) -> Result<(), AppError> {
    OcrService::clear_ocr_temp(&app)
}

fn require_debug(state: &State<'_, AppState>) -> Result<(), AppError> {
    if SettingsService::is_debug_enabled(&state.db)? {
        Ok(())
    } else {
        Err(AppError::ValidationError {
            message: "debug mode is disabled".into(),
        })
    }
}
