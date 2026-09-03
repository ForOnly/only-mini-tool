//! OCR 工具全局 settings 键（ocr.*）；引擎字段键由 Registry 按 spec 拼装。

pub const SETTING_OCR_SETTINGS_VERSION: &str = "ocr.settings_version";
pub const SETTING_OCR_ACTIVE_ENGINE: &str = "ocr.active_engine";
pub const SETTING_OCR_TIMEOUT_MS: &str = "ocr.timeout_ms";
pub const SETTING_OCR_INSPECTOR_PLACEMENT: &str = "ocr.inspector_placement";

/// 引擎字段键前缀：`ocr.engine.<id>.<field>`。
pub const OCR_ENGINE_KEY_PREFIX: &str = "ocr.engine.";

pub const OCR_SETTINGS_VERSION: u32 = 2;
pub const DEFAULT_OCR_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_OCR_ACTIVE_ENGINE: &str = "baidu";
pub const DEFAULT_OCR_INSPECTOR_PLACEMENT: &str = "right";

pub fn engine_setting_key(engine_id: &str, field: &str) -> String {
    format!("{OCR_ENGINE_KEY_PREFIX}{engine_id}.{field}")
}

/// 校验结果面板停靠位置。
pub fn parse_inspector_placement(raw: &str) -> Result<&'static str, ()> {
    match raw.trim() {
        "left" => Ok("left"),
        "right" => Ok("right"),
        "bottom" => Ok("bottom"),
        _ => Err(()),
    }
}
