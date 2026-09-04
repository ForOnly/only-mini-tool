//! OCR 引擎全局注册表（OnceLock）。

use std::sync::OnceLock;

use crate::domain::engine_setting_key;
use crate::errors::AppError;
use crate::infrastructure::database::Database;

use super::config::{EngineConfig, EngineSpec, OcrEngine};
use super::engines;
use super::secrets::read_engine_field;

static REGISTRY: OnceLock<OcrRegistry> = OnceLock::new();

pub struct OcrRegistry {
    engines: Vec<Box<dyn OcrEngine>>,
}

impl OcrRegistry {
    pub fn global() -> &'static Self {
        REGISTRY.get_or_init(|| Self {
            engines: engines::register_all(),
        })
    }

    pub fn is_known(&self, id: &str) -> bool {
        self.engines.iter().any(|e| e.spec().id == id)
    }

    pub fn specs(&self) -> Vec<&'static EngineSpec> {
        self.engines.iter().map(|e| e.spec()).collect()
    }

    pub fn engine(&self, id: &str) -> Result<&dyn OcrEngine, AppError> {
        self.engines
            .iter()
            .find(|e| e.spec().id == id)
            .map(|e| e.as_ref())
            .ok_or_else(|| AppError::InternalError {
                message: format!("OCR engine not found: {id}"),
            })
    }

    pub fn load_config(&self, db: &Database, engine_id: &str) -> Result<EngineConfig, AppError> {
        let engine = self.engine(engine_id)?;
        let spec = engine.spec();
        let mut values = std::collections::HashMap::new();
        for field in spec.fields {
            let key = engine_setting_key(spec.id, field.name);
            let val = read_engine_field(db, &key, field.kind)?;
            values.insert(field.name.to_string(), val);
        }
        Ok(EngineConfig { values })
    }

    /// 识别前校验必填字段非空。
    pub fn require_configured(&self, engine_id: &str, cfg: &EngineConfig) -> Result<(), AppError> {
        let engine = self.engine(engine_id)?;
        for field in engine.spec().fields {
            if field.required {
                let _ = cfg.require(field.name)?;
            }
        }
        Ok(())
    }
}
