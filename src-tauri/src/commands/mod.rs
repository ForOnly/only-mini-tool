use tauri::{AppHandle, Manager, State};

use crate::appearance::{self, AppearanceHost};
use crate::domain::{
    AppearanceDto, OcrResult, OcrSettingsBundle, OcrSettingsSave, ScriptCreate, ScriptDto,
    ScriptRunRequest, ScriptRunResult, ScriptSummary, ScriptUpdate, ScriptVenvSummary,
    ScriptsSettingsBundle, ScriptsSettingsSave, SETTING_DEBUG_ENABLED, SETTING_UI_THEME,
};
use crate::errors::AppError;
use crate::services::log_service::LogService;
use crate::services::ocr::OcrService;
use crate::services::scripts::ScriptsService;
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
        tracing::info!(
            enabled = value,
            "debug setting changed (restart for full log level)"
        );
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

#[tauri::command]
pub fn list_scripts(state: State<'_, AppState>) -> Result<Vec<ScriptSummary>, AppError> {
    ScriptsService::list(&state.db)
}

#[tauri::command]
pub fn get_script(state: State<'_, AppState>, id: i64) -> Result<ScriptDto, AppError> {
    ScriptsService::get(&state.db, id)
}

#[tauri::command]
pub fn create_script(
    state: State<'_, AppState>,
    payload: ScriptCreate,
) -> Result<ScriptDto, AppError> {
    ScriptsService::create(&state.db, payload)
}

#[tauri::command]
pub async fn update_script(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    payload: ScriptUpdate,
) -> Result<ScriptDto, AppError> {
    ScriptsService::update(&app, &state.db, id, payload).await
}

#[tauri::command]
pub fn delete_script(state: State<'_, AppState>, id: i64) -> Result<(), AppError> {
    ScriptsService::delete(&state.db, id)
}

#[tauri::command]
pub fn rename_script(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<ScriptDto, AppError> {
    ScriptsService::rename(&state.db, id, name)
}

#[tauri::command]
pub fn get_scripts_settings(state: State<'_, AppState>) -> Result<ScriptsSettingsBundle, AppError> {
    ScriptsService::get_settings(&state.db)
}

#[tauri::command]
pub async fn save_scripts_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: ScriptsSettingsSave,
) -> Result<(), AppError> {
    ScriptsService::save_settings(&app, &state.db, payload).await
}

#[tauri::command]
pub async fn run_script(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    payload: ScriptRunRequest,
) -> Result<ScriptRunResult, AppError> {
    ScriptsService::run(&app, &state.db, payload).await
}

#[tauri::command]
pub fn cancel_script_run() -> Result<(), AppError> {
    ScriptsService::cancel_run();
    Ok(())
}

#[tauri::command]
pub async fn create_scripts_venv(state: State<'_, AppState>) -> Result<(), AppError> {
    ScriptsService::create_venv(&state.db).await
}

#[tauri::command]
pub async fn create_script_venv(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    ScriptsService::create_named_venv(&app, &state.db, &name).await
}

#[tauri::command]
pub async fn list_script_venvs(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<ScriptVenvSummary>, AppError> {
    ScriptsService::list_venvs(&app, &state.db).await
}

#[tauri::command]
pub async fn delete_script_venv(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    ScriptsService::delete_venv(&app, &state.db, &name).await
}

#[tauri::command]
pub async fn install_script_venv_packages(
    app: AppHandle,
    name: String,
    packages: Vec<String>,
    requirements: Option<String>,
) -> Result<ScriptRunResult, AppError> {
    ScriptsService::install_venv_packages(&app, &name, packages, requirements).await
}

#[tauri::command]
pub async fn open_script_venv_terminal(app: AppHandle, name: String) -> Result<(), AppError> {
    ScriptsService::open_venv_terminal(&app, &name).await
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
