use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::AppHandle;
use tokio::sync::{Mutex, Notify};
use tokio::time::timeout;

use crate::domain::{
    engine_setting_key, parse_inspector_placement, OcrEngineFieldInfo, OcrEngineInfo, OcrFieldKind,
    OcrResult, OcrSettingsBundle, OcrSettingsSave, DEFAULT_OCR_ACTIVE_ENGINE,
    DEFAULT_OCR_INSPECTOR_PLACEMENT, DEFAULT_OCR_TIMEOUT_MS, SETTING_OCR_ACTIVE_ENGINE,
    SETTING_OCR_INSPECTOR_PLACEMENT, SETTING_OCR_TIMEOUT_MS,
};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::services::settings_service::SettingsService;

use super::engines::baidu;
use super::registry::OcrRegistry;
use super::secrets::{read_engine_field, write_engine_field};
use super::temp;

/// 识别单飞 + 可取消。
struct RecognizeGate {
    lock: Mutex<()>,
    cancelled: AtomicBool,
    notify: Notify,
}

fn recognize_gate() -> &'static RecognizeGate {
    static GATE: OnceLock<RecognizeGate> = OnceLock::new();
    GATE.get_or_init(|| RecognizeGate {
        lock: Mutex::new(()),
        cancelled: AtomicBool::new(false),
        notify: Notify::new(),
    })
}

pub struct OcrService;

impl OcrService {
    fn active_engine_id(db: &Database) -> Result<String, AppError> {
        let raw = SettingsService::get(db, SETTING_OCR_ACTIVE_ENGINE)?
            .unwrap_or_else(|| DEFAULT_OCR_ACTIVE_ENGINE.to_string());
        let id = raw.trim();
        if id.is_empty() {
            return Ok(DEFAULT_OCR_ACTIVE_ENGINE.to_string());
        }
        if !OcrRegistry::global().is_known(id) {
            return Err(AppError::ValidationError {
                message: format!("unknown ocr.active_engine: {id}"),
            });
        }
        Ok(id.to_string())
    }

    /// 取消进行中的识别（无任务时为 no-op）。
    pub fn cancel_recognize() {
        let gate = recognize_gate();
        gate.cancelled.store(true, Ordering::SeqCst);
        gate.notify.notify_waiters();
    }

    pub async fn recognize(db: &Database, path: &str) -> Result<OcrResult, AppError> {
        let gate = recognize_gate();
        let _guard = match gate.lock.try_lock() {
            Ok(guard) => guard,
            Err(_) => {
                return Err(AppError::OcrBusy {
                    message: "OCR recognition already in progress".into(),
                });
            }
        };

        gate.cancelled.store(false, Ordering::SeqCst);

        let active_id = Self::active_engine_id(db)?;
        let registry = OcrRegistry::global();
        let cfg = registry.load_config(db, &active_id)?;
        registry.require_configured(&active_id, &cfg)?;

        let timeout_ms = SettingsService::ocr_timeout_ms(db)?;
        let bytes = temp::read_image_bytes(path)?;
        if bytes.is_empty() {
            return Err(AppError::OcrBadImage {
                message: "image file is empty".into(),
            });
        }

        let engine = registry.engine(&active_id)?;
        let fut = engine.recognize(&bytes, &cfg);
        let timed = timeout(Duration::from_millis(timeout_ms), fut);

        tokio::select! {
            _ = async {
                loop {
                    if gate.cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    gate.notify.notified().await;
                }
            } => {
                Err(AppError::OcrCancelled {
                    message: "OCR recognition cancelled".into(),
                })
            }
            result = timed => {
                match result {
                    Ok(inner) => inner,
                    Err(_) => Err(AppError::OcrTimeout {
                        message: format!("OCR timed out after {timeout_ms}ms"),
                    }),
                }
            }
        }
    }

    pub fn get_settings(db: &Database) -> Result<OcrSettingsBundle, AppError> {
        let registry = OcrRegistry::global();
        let active_engine = SettingsService::get(db, SETTING_OCR_ACTIVE_ENGINE)?
            .unwrap_or_else(|| DEFAULT_OCR_ACTIVE_ENGINE.to_string());
        let active_engine = {
            let trimmed = active_engine.trim();
            if registry.is_known(trimmed) {
                trimmed.to_string()
            } else {
                DEFAULT_OCR_ACTIVE_ENGINE.to_string()
            }
        };

        let timeout_ms = SettingsService::get(db, SETTING_OCR_TIMEOUT_MS)?
            .unwrap_or_else(|| DEFAULT_OCR_TIMEOUT_MS.to_string());

        let inspector_placement = SettingsService::get(db, SETTING_OCR_INSPECTOR_PLACEMENT)?
            .unwrap_or_else(|| DEFAULT_OCR_INSPECTOR_PLACEMENT.to_string());
        let inspector_placement = parse_inspector_placement(&inspector_placement)
            .unwrap_or(DEFAULT_OCR_INSPECTOR_PLACEMENT)
            .to_string();

        let mut engines = Vec::new();
        let mut values = HashMap::new();
        values.insert(SETTING_OCR_ACTIVE_ENGINE.to_string(), active_engine.clone());
        values.insert(SETTING_OCR_TIMEOUT_MS.to_string(), timeout_ms.clone());
        values.insert(
            SETTING_OCR_INSPECTOR_PLACEMENT.to_string(),
            inspector_placement.clone(),
        );

        for spec in registry.specs() {
            let mut fields = Vec::new();
            for field in spec.fields {
                let setting_key = engine_setting_key(spec.id, field.name);
                let val = read_engine_field(db, &setting_key, field.kind)?;
                values.insert(setting_key.clone(), val);
                fields.push(OcrEngineFieldInfo {
                    name: field.name.to_string(),
                    setting_key,
                    required: field.required,
                    kind: field.kind,
                });
            }
            engines.push(OcrEngineInfo {
                id: spec.id.to_string(),
                fields,
            });
        }

        Ok(OcrSettingsBundle {
            active_engine,
            timeout_ms,
            inspector_placement,
            engines,
            values,
        })
    }

    pub fn save_settings(db: &Database, payload: OcrSettingsSave) -> Result<(), AppError> {
        let registry = OcrRegistry::global();
        let active = payload.active_engine.trim();
        if !registry.is_known(active) {
            return Err(AppError::ValidationError {
                message: format!("invalid ocr.active_engine: {active}"),
            });
        }

        let timeout = payload.timeout_ms.trim();
        let ms: u64 = timeout.parse().map_err(|_| AppError::ValidationError {
            message: format!("invalid ocr.timeout_ms: {timeout}"),
        })?;
        if ms == 0 {
            return Err(AppError::ValidationError {
                message: "ocr.timeout_ms must be > 0".into(),
            });
        }

        let placement =
            parse_inspector_placement(&payload.inspector_placement).ok_or_else(|| {
                AppError::ValidationError {
                    message: format!(
                        "invalid ocr.inspector_placement: {}",
                        payload.inspector_placement
                    ),
                }
            })?;

        let engine = registry.engine(active)?;
        let spec = engine.spec();
        let allowed: HashMap<&str, OcrFieldKind> =
            spec.fields.iter().map(|f| (f.name, f.kind)).collect();

        for key in payload.values.keys() {
            let prefix = format!("ocr.engine.{active}.");
            let Some(field_name) = key.strip_prefix(&prefix) else {
                return Err(AppError::ValidationError {
                    message: format!("unexpected settings key for active engine: {key}"),
                });
            };
            if !allowed.contains_key(field_name) {
                return Err(AppError::ValidationError {
                    message: format!("unknown field for engine {active}: {field_name}"),
                });
            }
        }

        SettingsService::set(db, SETTING_OCR_ACTIVE_ENGINE, active)?;
        SettingsService::set(db, SETTING_OCR_TIMEOUT_MS, &ms.to_string())?;
        SettingsService::set(db, SETTING_OCR_INSPECTOR_PLACEMENT, placement)?;
        for field in spec.fields {
            let key = engine_setting_key(active, field.name);
            let val = payload
                .values
                .get(&key)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            write_engine_field(db, &key, field.kind, &val)?;
        }

        // 百度密钥变更后丢弃进程内 token 缓存
        if active == "baidu" {
            baidu::invalidate_token_cache();
        }
        Ok(())
    }

    pub fn stage_image_file(app: &AppHandle, source: &str) -> Result<String, AppError> {
        temp::stage_image_file(app, source)
    }

    pub fn stage_image_bytes(
        app: &AppHandle,
        bytes: Vec<u8>,
        ext: String,
    ) -> Result<String, AppError> {
        temp::stage_image_bytes(app, &bytes, &ext)
    }

    pub fn save_clipboard_image(app: &AppHandle) -> Result<String, AppError> {
        temp::save_clipboard_image(app)
    }

    pub fn rotate_image_orientation(
        app: &AppHandle,
        path: &str,
        degrees: i32,
    ) -> Result<String, AppError> {
        temp::rotate_image_orientation(app, path, degrees)
    }

    pub fn clear_ocr_temp(app: &AppHandle) -> Result<(), AppError> {
        temp::clear_ocr_temp(app)
    }
}
