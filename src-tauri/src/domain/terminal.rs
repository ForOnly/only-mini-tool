//! 终端模块 domain 类型（ts-rs 导出；无 i64 字段，前端无需 bigint 修正）。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 终端会话创建载荷。venv 激活策略见 services/terminal/shell.rs（env 注入式）。
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TerminalCreatePayload {
    /// 工作目录；空 = 用户主目录
    #[ts(optional)]
    pub cwd: Option<String>,
    /// 追加到继承的全量系统环境之上（终端不像 runner 管道那样 env_clear）
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// 初始列数（clamp 10..=500，默认 80）
    #[ts(optional)]
    pub cols: Option<u16>,
    /// 初始行数（clamp 4..=200，默认 24）
    #[ts(optional)]
    pub rows: Option<u16>,
    /// "pwsh" | "powershell" | "cmd" | 可执行文件路径；None = 自动探测
    #[ts(optional)]
    pub shell: Option<String>,
    /// 会话启动后追加写入的命令（逐条补 \r）；排在默认编码/提示符命令之后
    #[serde(default)]
    pub init_commands: Vec<String>,
    /// venv 引用（".venv" = 默认 workspace；命名 = venvs/<name>）；
    /// 缺失则静默跳过激活（解析容错，与运行链一致）
    #[ts(optional)]
    pub venv: Option<String>,
    /// 展示用标题（标签/子窗口）
    #[ts(optional)]
    pub title: Option<String>,
}

/// 终端会话快照（terminal_list / terminal_create 返回）。
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TerminalInfo {
    pub id: String,
    /// 生效 shell 标签（"pwsh"/"powershell"/"cmd"/自定义路径）
    pub shell: String,
    pub cwd: String,
    #[ts(optional)]
    pub venv: Option<String>,
    #[ts(optional)]
    pub title: Option<String>,
    /// 创建时间（UNIX 秒，字符串承载避免 ts-rs i64→bigint）
    pub created_at: String,
    pub exited: bool,
    #[ts(optional)]
    pub exit_code: Option<i32>,
}

/// Channel 消息类型：Replay（attach 时 scrollback 回放）/ Output（增量输出）/
/// Exit（shell 退出，exit_code 可能未就绪）/ Error（预留）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, rename_all = "lowercase")]
pub enum TerminalEventKind {
    Replay,
    Output,
    Exit,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct TerminalEvent {
    pub kind: TerminalEventKind,
    #[serde(default)]
    pub data: String,
    #[serde(default)]
    #[ts(optional)]
    pub exit_code: Option<i32>,
}

/// 脚本编辑页终端预设（复用 run 的 cwd/venv 解析链）。
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct ScriptTerminalConfig {
    pub cwd: String,
    /// 实际生效的 venv 引用；None = 全局链（展示「全局」徽标用）
    #[ts(optional)]
    pub venv_name: Option<String>,
    #[ts(optional)]
    pub venv_python: Option<String>,
}
