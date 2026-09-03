use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UiTheme {
    System,
    Light,
    Dark,
}

impl UiTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ()> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(()),
        }
    }

    pub fn resolve(self, os: ColorScheme) -> ColorScheme {
        match self {
            Self::Light => ColorScheme::Light,
            Self::Dark => ColorScheme::Dark,
            Self::System => os,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct AppearanceDto {
    pub preference: UiTheme,
    pub resolved: ColorScheme,
}

/// 图片像素坐标矩形（百度 location 归一）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrRect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrWord {
    pub text: String,
    pub rect: OcrRect,
    #[ts(optional)]
    pub confidence: Option<f64>,
    #[ts(optional)]
    pub line: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct OcrResult {
    pub engine: String,
    pub words: Vec<OcrWord>,
    pub text: String,
}

pub mod ocr_settings;
pub mod settings;

pub use ocr_settings::{
    OcrEngineFieldInfo, OcrEngineInfo, OcrFieldKind, OcrSettingsBundle, OcrSettingsSave,
};
pub use settings::{
    engine_setting_key, parse_inspector_placement, DEFAULT_OCR_ACTIVE_ENGINE,
    DEFAULT_OCR_INSPECTOR_PLACEMENT, DEFAULT_OCR_TIMEOUT_MS, OCR_ENGINE_KEY_PREFIX,
    OCR_SETTINGS_VERSION, SETTING_DEBUG_ENABLED, SETTING_OCR_ACTIVE_ENGINE,
    SETTING_OCR_INSPECTOR_PLACEMENT, SETTING_OCR_SETTINGS_VERSION, SETTING_OCR_TIMEOUT_MS,
    SETTING_UI_THEME,
};

pub fn export_all_ts(out_dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(out_dir);
    UiTheme::export_all().expect("export UiTheme");
    ColorScheme::export_all().expect("export ColorScheme");
    AppearanceDto::export_all().expect("export AppearanceDto");
    OcrRect::export_all().expect("export OcrRect");
    OcrWord::export_all().expect("export OcrWord");
    OcrResult::export_all().expect("export OcrResult");
    OcrFieldKind::export_all().expect("export OcrFieldKind");
    OcrEngineFieldInfo::export_all().expect("export OcrEngineFieldInfo");
    OcrEngineInfo::export_all().expect("export OcrEngineInfo");
    OcrSettingsBundle::export_all().expect("export OcrSettingsBundle");
    OcrSettingsSave::export_all().expect("export OcrSettingsSave");

    let bindings = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings");
    let alt_bindings = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bindings");
    let _ = std::fs::create_dir_all(out_dir);
    for name in [
        "UiTheme.ts",
        "ColorScheme.ts",
        "AppearanceDto.ts",
        "OcrRect.ts",
        "OcrWord.ts",
        "OcrResult.ts",
        "OcrFieldKind.ts",
        "OcrEngineFieldInfo.ts",
        "OcrEngineInfo.ts",
        "OcrSettingsBundle.ts",
        "OcrSettingsSave.ts",
    ] {
        let src = if bindings.join(name).exists() {
            bindings.join(name)
        } else {
            alt_bindings.join(name)
        };
        let dst = out_dir.join(name);
        if src.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
    }
    let index = r#"export type { UiTheme } from "./UiTheme";
export type { ColorScheme } from "./ColorScheme";
export type { AppearanceDto } from "./AppearanceDto";
export type { OcrRect } from "./OcrRect";
export type { OcrWord } from "./OcrWord";
export type { OcrResult } from "./OcrResult";
export type { OcrFieldKind } from "./OcrFieldKind";
export type { OcrEngineFieldInfo } from "./OcrEngineFieldInfo";
export type { OcrEngineInfo } from "./OcrEngineInfo";
export type { OcrSettingsBundle } from "./OcrSettingsBundle";
export type { OcrSettingsSave } from "./OcrSettingsSave";
"#;
    std::fs::write(out_dir.join("index.ts"), index).expect("write index.ts");
}
