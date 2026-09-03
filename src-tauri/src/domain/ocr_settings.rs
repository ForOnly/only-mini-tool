//! OCR 设置 Bundle DTO（ts-rs）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum OcrFieldKind {
    Text,
    Password,
    Url,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrEngineFieldInfo {
    pub name: String,
    pub setting_key: String,
    pub required: bool,
    pub kind: OcrFieldKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrEngineInfo {
    pub id: String,
    pub fields: Vec<OcrEngineFieldInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrSettingsBundle {
    pub active_engine: String,
    pub timeout_ms: String,
    pub inspector_placement: String,
    pub engines: Vec<OcrEngineInfo>,
    pub values: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrSettingsSave {
    pub active_engine: String,
    pub timeout_ms: String,
    pub inspector_placement: String,
    pub values: HashMap<String, String>,
}
