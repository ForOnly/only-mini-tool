//! 脚本库 DTO（ts-rs 导出）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptParamType {
    String,
    Number,
    Boolean,
    Select,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptParamPassAs {
    Env,
    Arg,
    /// 以类型化 JSON 文档写入子进程 stdin（脚本 `json.load(sys.stdin)` 读取）。
    Stdin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptPathMode {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptParamDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub param_type: ScriptParamType,
    #[serde(default)]
    pub required: bool,
    #[serde(default, rename = "default")]
    #[ts(optional)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    #[ts(optional)]
    pub path_mode: Option<ScriptPathMode>,
    pub pass_as: ScriptParamPassAs,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptArgsTemplate {
    #[serde(default)]
    pub before: Vec<String>,
    #[serde(default)]
    pub after: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptSummary {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub language: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptDto {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub language: String,
    pub body: String,
    #[ts(optional)]
    pub workspace_path: Option<String>,
    #[ts(optional)]
    pub interpreter_path: Option<String>,
    /// 绑定的命名 venv（None = 跟随全局链）。
    #[ts(optional)]
    pub venv_name: Option<String>,
    pub env: HashMap<String, String>,
    pub params_schema: Vec<ScriptParamDef>,
    pub args_template: ScriptArgsTemplate,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptCreate {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptUpdate {
    pub name: String,
    pub description: String,
    pub body: String,
    #[ts(optional)]
    pub workspace_path: Option<String>,
    #[ts(optional)]
    pub interpreter_path: Option<String>,
    #[ts(optional)]
    pub venv_name: Option<String>,
    pub env: HashMap<String, String>,
    pub params_schema: Vec<ScriptParamDef>,
    pub args_template: ScriptArgsTemplate,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptsSettingsBundle {
    pub python_path: String,
    pub default_workspace: String,
    pub env_prefix: String,
    /// 全局启用的命名 venv（空 = 未启用）。
    pub venv: String,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptsSettingsSave {
    pub python_path: String,
    pub default_workspace: String,
    pub env_prefix: String,
    pub venv: String,
    pub env: HashMap<String, String>,
}

/// venv 列表条目（`list_script_venvs`）。
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptVenvSummary {
    pub name: String,
    pub python_path: String,
    /// venv 的 Python 版本（读 `pyvenv.cfg` 的 `version =`；读不到为 None）。
    #[serde(default)]
    #[ts(optional)]
    pub python_version: Option<String>,
    /// true = 默认 workspace 下的 `.venv`（自动探测条目，非托管命名 venv）。
    pub workspace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptRunRequest {
    pub script_id: i64,
    /// 表单取值，统一为字符串（boolean 用 "true"/"false"）。
    pub params: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub cancelled: bool,
    /// 等效命令回显：解释器 + args_template + 脚本名 + 动态参数（非临时路径，仅供阅读/复制）。
    pub command: String,
}
