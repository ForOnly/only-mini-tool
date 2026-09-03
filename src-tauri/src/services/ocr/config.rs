//! OCR 引擎规格与运行时配置。

use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::{OcrFieldKind, OcrResult};
use crate::errors::AppError;

/// 引擎运行时配置（仅后端从 settings 组装）。
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    pub values: HashMap<String, String>,
}

impl EngineConfig {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn require(&self, key: &str) -> Result<&str, AppError> {
        self.get(key)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| AppError::OcrNotConfigured {
                message: format!("{key} is empty"),
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EngineFieldSpec {
    pub name: &'static str,
    pub required: bool,
    pub kind: OcrFieldKind,
}

impl EngineFieldSpec {
    pub const fn text(name: &'static str, required: bool) -> Self {
        Self {
            name,
            required,
            kind: OcrFieldKind::Text,
        }
    }

    pub const fn password(name: &'static str, required: bool) -> Self {
        Self {
            name,
            required,
            kind: OcrFieldKind::Password,
        }
    }

    #[allow(dead_code)]
    pub const fn url(name: &'static str, required: bool) -> Self {
        Self {
            name,
            required,
            kind: OcrFieldKind::Url,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EngineSpec {
    pub id: &'static str,
    pub fields: &'static [EngineFieldSpec],
}

#[async_trait]
pub trait OcrEngine: Send + Sync {
    fn spec(&self) -> &'static EngineSpec;

    async fn recognize(&self, image: &[u8], cfg: &EngineConfig) -> Result<OcrResult, AppError>;
}
