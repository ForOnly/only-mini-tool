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

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
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
pub mod script;
pub mod settings;
pub mod terminal;

pub use ocr_settings::{
    OcrEngineFieldInfo, OcrEngineInfo, OcrFieldKind, OcrSettingsBundle, OcrSettingsSave,
};
pub use script::{
    ScriptArgsTemplate, ScriptCreate, ScriptDto, ScriptParamDef, ScriptParamPassAs,
    ScriptParamType, ScriptPathMode, ScriptRunRequest, ScriptRunResult, ScriptSummary,
    ScriptUpdate, ScriptVenvSummary, ScriptsSettingsBundle, ScriptsSettingsSave,
};
pub use terminal::{
    ScriptTerminalConfig, TerminalCreatePayload, TerminalEvent, TerminalEventKind, TerminalInfo,
};
pub use settings::{
    engine_setting_key, parse_inspector_placement, DEFAULT_OCR_ACTIVE_ENGINE,
    DEFAULT_OCR_INSPECTOR_PLACEMENT, DEFAULT_OCR_TIMEOUT_MS, DEFAULT_SCRIPTS_ENV_JSON,
    DEFAULT_SCRIPTS_ENV_PREFIX, DEFAULT_SCRIPTS_PYTHON_PATH, OCR_ENGINE_KEY_PREFIX,
    OCR_SETTINGS_VERSION, SETTING_DEBUG_ENABLED, SETTING_OCR_ACTIVE_ENGINE,
    SETTING_OCR_INSPECTOR_PLACEMENT, SETTING_OCR_SETTINGS_VERSION, SETTING_OCR_TIMEOUT_MS,
    SETTING_SCRIPTS_DEFAULT_WORKSPACE, SETTING_SCRIPTS_ENV_JSON, SETTING_SCRIPTS_ENV_PREFIX,
    SETTING_SCRIPTS_PYTHON_PATH, SETTING_SCRIPTS_VENV, SETTING_UI_THEME,
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
    ScriptParamType::export_all().expect("export ScriptParamType");
    ScriptParamPassAs::export_all().expect("export ScriptParamPassAs");
    ScriptPathMode::export_all().expect("export ScriptPathMode");
    ScriptParamDef::export_all().expect("export ScriptParamDef");
    ScriptArgsTemplate::export_all().expect("export ScriptArgsTemplate");
    ScriptSummary::export_all().expect("export ScriptSummary");
    ScriptDto::export_all().expect("export ScriptDto");
    ScriptCreate::export_all().expect("export ScriptCreate");
    ScriptUpdate::export_all().expect("export ScriptUpdate");
    ScriptsSettingsBundle::export_all().expect("export ScriptsSettingsBundle");
    ScriptsSettingsSave::export_all().expect("export ScriptsSettingsSave");
    ScriptVenvSummary::export_all().expect("export ScriptVenvSummary");
    ScriptRunRequest::export_all().expect("export ScriptRunRequest");
    ScriptRunResult::export_all().expect("export ScriptRunResult");
    TerminalCreatePayload::export_all().expect("export TerminalCreatePayload");
    TerminalInfo::export_all().expect("export TerminalInfo");
    TerminalEventKind::export_all().expect("export TerminalEventKind");
    TerminalEvent::export_all().expect("export TerminalEvent");
    ScriptTerminalConfig::export_all().expect("export ScriptTerminalConfig");

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
        "ScriptParamType.ts",
        "ScriptParamPassAs.ts",
        "ScriptPathMode.ts",
        "ScriptParamDef.ts",
        "ScriptArgsTemplate.ts",
        "ScriptSummary.ts",
        "ScriptDto.ts",
        "ScriptCreate.ts",
        "ScriptUpdate.ts",
        "ScriptsSettingsBundle.ts",
        "ScriptsSettingsSave.ts",
        "ScriptVenvSummary.ts",
        "ScriptRunRequest.ts",
        "ScriptRunResult.ts",
        "TerminalCreatePayload.ts",
        "TerminalInfo.ts",
        "TerminalEventKind.ts",
        "TerminalEvent.ts",
        "ScriptTerminalConfig.ts",
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
export type { ScriptParamType } from "./ScriptParamType";
export type { ScriptParamPassAs } from "./ScriptParamPassAs";
export type { ScriptPathMode } from "./ScriptPathMode";
export type { ScriptParamDef } from "./ScriptParamDef";
export type { ScriptArgsTemplate } from "./ScriptArgsTemplate";
export type { ScriptSummary } from "./ScriptSummary";
export type { ScriptDto } from "./ScriptDto";
export type { ScriptCreate } from "./ScriptCreate";
export type { ScriptUpdate } from "./ScriptUpdate";
export type { ScriptsSettingsBundle } from "./ScriptsSettingsBundle";
export type { ScriptsSettingsSave } from "./ScriptsSettingsSave";
export type { ScriptVenvSummary } from "./ScriptVenvSummary";
export type { ScriptRunRequest } from "./ScriptRunRequest";
export type { ScriptRunResult } from "./ScriptRunResult";
export type { TerminalCreatePayload } from "./TerminalCreatePayload";
export type { TerminalInfo } from "./TerminalInfo";
export type { TerminalEventKind } from "./TerminalEventKind";
export type { TerminalEvent } from "./TerminalEvent";
export type { ScriptTerminalConfig } from "./ScriptTerminalConfig";
"#;
    std::fs::write(out_dir.join("index.ts"), index).expect("write index.ts");
}
