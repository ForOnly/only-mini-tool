//! OCR 引擎密钥读写：密码字段走 OS keyring，并迁移旧 SQLite 明文。

use crate::domain::{engine_setting_key, OcrFieldKind};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::infrastructure::secret_store;
use crate::services::settings_service::SettingsService;

use super::engines::baidu;
use super::registry::OcrRegistry;

pub fn read_engine_field(
    db: &Database,
    setting_key: &str,
    kind: OcrFieldKind,
) -> Result<String, AppError> {
    if kind != OcrFieldKind::Password {
        return Ok(SettingsService::get(db, setting_key)?.unwrap_or_default());
    }
    if let Some(secret) = secret_store::get(setting_key)? {
        return Ok(secret);
    }
    let legacy = SettingsService::get(db, setting_key)?.unwrap_or_default();
    if !legacy.is_empty() {
        secret_store::set(setting_key, &legacy)?;
        SettingsService::set_raw(db, setting_key, "")?;
        return Ok(legacy);
    }
    Ok(String::new())
}

pub fn write_engine_field(
    db: &Database,
    setting_key: &str,
    kind: OcrFieldKind,
    value: &str,
) -> Result<(), AppError> {
    if kind == OcrFieldKind::Password {
        secret_store::set(setting_key, value)?;
        SettingsService::set_raw(db, setting_key, "")?;
        return Ok(());
    }
    SettingsService::set_raw(db, setting_key, value)
}

/// schema wipe 时清除全部引擎密码凭据，并失效百度 token 缓存。
pub fn wipe_engine_secrets_on_schema_reset() -> Result<(), AppError> {
    let registry = OcrRegistry::global();
    for spec in registry.specs() {
        for field in spec.fields {
            if field.kind == OcrFieldKind::Password {
                let key = engine_setting_key(spec.id, field.name);
                secret_store::delete(&key)?;
            }
        }
    }
    baidu::invalidate_token_cache();
    Ok(())
}
