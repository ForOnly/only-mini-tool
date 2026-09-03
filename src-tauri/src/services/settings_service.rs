use crate::domain::{
    UiTheme, DEFAULT_OCR_ACTIVE_ENGINE, DEFAULT_OCR_INSPECTOR_PLACEMENT, DEFAULT_OCR_TIMEOUT_MS,
    OCR_ENGINE_KEY_PREFIX, OCR_SETTINGS_VERSION, SETTING_DEBUG_ENABLED, SETTING_OCR_ACTIVE_ENGINE,
    SETTING_OCR_INSPECTOR_PLACEMENT, SETTING_OCR_SETTINGS_VERSION, SETTING_OCR_TIMEOUT_MS,
    SETTING_UI_THEME, parse_inspector_placement,
};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::repository::settings::SettingsRepo;
use crate::services::ocr::OcrRegistry;

pub struct SettingsService;

impl SettingsService {
    pub fn ensure_defaults(db: &Database) -> Result<(), AppError> {
        db.with_conn(|conn| {
            if SettingsRepo::get(conn, SETTING_UI_THEME)?.is_none() {
                SettingsRepo::set(conn, SETTING_UI_THEME, UiTheme::System.as_str())?;
            }
            if SettingsRepo::get(conn, SETTING_DEBUG_ENABLED)?.is_none() {
                SettingsRepo::set(conn, SETTING_DEBUG_ENABLED, "false")?;
            }

            let version_raw = SettingsRepo::get(conn, SETTING_OCR_SETTINGS_VERSION)?;
            let version_ok = version_raw
                .as_deref()
                .and_then(|v| v.parse::<u32>().ok())
                == Some(OCR_SETTINGS_VERSION);
            if !version_ok {
                SettingsRepo::delete_prefix(conn, "ocr.")?;
                SettingsRepo::set(
                    conn,
                    SETTING_OCR_SETTINGS_VERSION,
                    &OCR_SETTINGS_VERSION.to_string(),
                )?;
                SettingsRepo::set(conn, SETTING_OCR_ACTIVE_ENGINE, DEFAULT_OCR_ACTIVE_ENGINE)?;
                SettingsRepo::set(
                    conn,
                    SETTING_OCR_TIMEOUT_MS,
                    &DEFAULT_OCR_TIMEOUT_MS.to_string(),
                )?;
                SettingsRepo::set(
                    conn,
                    SETTING_OCR_INSPECTOR_PLACEMENT,
                    DEFAULT_OCR_INSPECTOR_PLACEMENT,
                )?;
            } else if SettingsRepo::get(conn, SETTING_OCR_INSPECTOR_PLACEMENT)?.is_none() {
                // 旧库缺键时补默认，不 wipe
                SettingsRepo::set(
                    conn,
                    SETTING_OCR_INSPECTOR_PLACEMENT,
                    DEFAULT_OCR_INSPECTOR_PLACEMENT,
                )?;
            }

            Ok(())
        })
    }

    pub fn get(db: &Database, key: &str) -> Result<Option<String>, AppError> {
        db.with_conn(|conn| SettingsRepo::get(conn, key))
    }

    /// 写入任意 settings 键（含 `ocr.engine.*`）；供 OCR bundle save 使用。
    pub fn set_raw(db: &Database, key: &str, value: &str) -> Result<(), AppError> {
        db.with_conn(|conn| SettingsRepo::set(conn, key, value))
    }

    pub fn set(db: &Database, key: &str, value: &str) -> Result<(), AppError> {
        if key == SETTING_OCR_SETTINGS_VERSION || key.starts_with(OCR_ENGINE_KEY_PREFIX) {
            return Err(AppError::ValidationError {
                message: format!("key must be written via OCR settings bundle: {key}"),
            });
        }
        if key == SETTING_UI_THEME {
            UiTheme::parse(value).map_err(|_| AppError::ValidationError {
                message: format!("invalid ui.theme: {value}"),
            })?;
        }
        if key == SETTING_DEBUG_ENABLED && value != "true" && value != "false" {
            return Err(AppError::ValidationError {
                message: format!("invalid app.debug_enabled: {value}"),
            });
        }
        if key == SETTING_OCR_TIMEOUT_MS {
            let ms: u64 = value.parse().map_err(|_| AppError::ValidationError {
                message: format!("invalid ocr.timeout_ms: {value}"),
            })?;
            if ms == 0 {
                return Err(AppError::ValidationError {
                    message: "ocr.timeout_ms must be > 0".into(),
                });
            }
        }
        if key == SETTING_OCR_ACTIVE_ENGINE && !OcrRegistry::global().is_known(value.trim()) {
            return Err(AppError::ValidationError {
                message: format!("invalid ocr.active_engine: {value}"),
            });
        }
        if key == SETTING_OCR_INSPECTOR_PLACEMENT {
            parse_inspector_placement(value).map_err(|_| AppError::ValidationError {
                message: format!("invalid ocr.inspector_placement: {value}"),
            })?;
        }
        Self::set_raw(db, key, value)
    }

    pub fn get_ui_theme(db: &Database) -> Result<UiTheme, AppError> {
        let raw = Self::get(db, SETTING_UI_THEME)?;
        Ok(raw
            .as_deref()
            .and_then(|v| UiTheme::parse(v).ok())
            .unwrap_or(UiTheme::System))
    }

    pub fn is_debug_enabled(db: &Database) -> Result<bool, AppError> {
        Ok(Self::get(db, SETTING_DEBUG_ENABLED)?.as_deref() == Some("true"))
    }

    pub fn ocr_timeout_ms(db: &Database) -> Result<u64, AppError> {
        let raw = Self::get(db, SETTING_OCR_TIMEOUT_MS)?;
        Ok(raw
            .as_deref()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_OCR_TIMEOUT_MS)
            .max(1))
    }
}
