//! 脚本库全局 settings 键（scripts.*）。

pub const SETTING_SCRIPTS_PYTHON_PATH: &str = "scripts.python_path";
pub const SETTING_SCRIPTS_DEFAULT_WORKSPACE: &str = "scripts.default_workspace";
pub const SETTING_SCRIPTS_ENV_JSON: &str = "scripts.env_json";
pub const SETTING_SCRIPTS_ENV_PREFIX: &str = "scripts.env_prefix";
/// 全局启用的命名 venv（空 = 未启用；经 set_setting 单键写入，不做存在性校验——
/// 运行时解析容错：指向的 venv 不存在则静默回落 workspace .venv / 全局 python）。
pub const SETTING_SCRIPTS_VENV: &str = "scripts.venv";

pub const DEFAULT_SCRIPTS_PYTHON_PATH: &str = "python";
pub const DEFAULT_SCRIPTS_ENV_JSON: &str = "{}";
pub const DEFAULT_SCRIPTS_ENV_PREFIX: &str = "PARAM_";
