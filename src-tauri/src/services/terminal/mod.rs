//! 终端模块：会话注册表 + PTY 会话生命周期。
//!
//! 会话不进 ScriptRunRegistry（多会话并发是核心诉求，与脚本运行单飞互不干扰）。
//! 全局上限 16 会话防失控。

pub mod session;
pub mod shell;
pub mod utf8;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use tauri::AppHandle;

use crate::domain::{TerminalCreatePayload, TerminalInfo};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem::venvs_dir;
use crate::services::scripts::validate;
use crate::services::scripts::ScriptsService;
pub use crate::services::terminal::session::{SessionSpawn, TerminalSession};

/// 全局会话上限（防失控；超限提示用户关闭旧会话）。
const MAX_SESSIONS: usize = 16;

pub struct TerminalRegistry {
    sessions: Mutex<HashMap<String, Arc<TerminalSession>>>,
}

impl TerminalRegistry {
    pub fn global() -> &'static TerminalRegistry {
        static REGISTRY: OnceLock<TerminalRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| TerminalRegistry {
            sessions: Mutex::new(HashMap::new()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<TerminalSession>>> {
        self.sessions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 创建会话：探测 shell → 解析 cwd/venv → env 组装 → PTY spawn → 注册。
    pub fn create(
        &self,
        app: &AppHandle,
        db: &Database,
        payload: TerminalCreatePayload,
    ) -> Result<TerminalInfo, AppError> {
        {
            let sessions = self.lock();
            if sessions.len() >= MAX_SESSIONS {
                return Err(AppError::ValidationError {
                    message: format!("terminal session limit reached ({MAX_SESSIONS})"),
                });
            }
        }

        let kind = payload
            .shell
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(shell::parse_shell_request)
            .unwrap_or_else(shell::detect_shell);

        let cwd = resolve_cwd(payload.cwd.as_deref())?;
        let venv_ref = payload
            .venv
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let venv_root = match venv_ref {
            Some(name) => resolve_venv_root(app, db, name)?,
            None => None,
        };

        // env 组装：载荷覆盖项 + venv 激活注入（venv 侧键胜出——VIRTUAL_ENV/PATH/PROMPT/PYTHONUTF8）
        let mut env_overlay = payload.env.clone();
        if let (Some(root), Some(name)) = (&venv_root, venv_ref) {
            for (key, value) in shell::venv_env(root, name) {
                env_overlay.insert(key, value);
            }
        }

        let spawn_cfg = shell::spawn_command(&kind);
        let init_commands = {
            let mut cmds = shell::default_init_commands(&kind, venv_ref);
            cmds.extend(payload.init_commands.iter().cloned());
            cmds
        };

        let session = TerminalSession::spawn(SessionSpawn {
            program: spawn_cfg.program,
            args: spawn_cfg.args,
            cwd,
            env_overlay,
            cols: payload.cols.unwrap_or(80).clamp(10, 500),
            rows: payload.rows.unwrap_or(24).clamp(4, 200),
            shell_label: kind.label(),
            venv_label: venv_ref.map(str::to_string),
            title: payload.title.clone(),
            init_commands,
        })?;

        let info = session.snapshot();
        self.lock().insert(session.id.clone(), session);
        Ok(info)
    }

    pub fn get(&self, id: &str) -> Option<Arc<TerminalSession>> {
        self.lock().get(id).cloned()
    }

    pub fn remove(&self, id: &str) {
        self.lock().remove(id);
    }

    pub fn list(&self) -> Vec<TerminalInfo> {
        self.lock().values().map(|s| s.snapshot()).collect()
    }

    /// app 退出清理：杀全部会话进程树（PTY 随进程树终结关闭）。
    pub fn dispose_all(&self) {
        let sessions: Vec<Arc<TerminalSession>> = self.lock().values().cloned().collect();
        for session in sessions {
            session.dispose();
        }
        self.lock().clear();
    }
}

/// cwd 解析：空 = 用户主目录；给定但不存在 = ValidationError（终端不猜目录）。
fn resolve_cwd(cwd: Option<&str>) -> Result<PathBuf, AppError> {
    match cwd.map(str::trim).filter(|s| !s.is_empty()) {
        Some(dir) => {
            let path = PathBuf::from(dir);
            if path.is_dir() {
                Ok(path)
            } else {
                Err(AppError::ValidationError {
                    message: format!("terminal cwd not found: {dir}"),
                })
            }
        }
        None => {
            let home = std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            Ok(home)
        }
    }
}

/// venv 根解析（静默容错版，与运行链一致）：`".venv"` = 默认 workspace 下；
/// 命名 = `venvs/<name>`（白名单校验防路径穿越）。探测不到 python → None（跳过激活）。
fn resolve_venv_root(
    app: &AppHandle,
    db: &Database,
    name: &str,
) -> Result<Option<PathBuf>, AppError> {
    if name == ".venv" {
        let settings = ScriptsService::get_settings(db)?;
        let ws = settings.default_workspace.trim();
        if ws.is_empty() {
            return Ok(None);
        }
        let dir = std::path::Path::new(ws).join(".venv");
        return Ok(venv_python_exists(&dir).then_some(dir));
    }
    let name = validate::validate_venv_name(name)?;
    let dir = venvs_dir(app)?.join(&name);
    Ok(venv_python_exists(&dir).then_some(dir))
}

/// venv 根下平台 python 是否存在（同步探测；终端创建是低频操作）。
fn venv_python_exists(venv_root: &std::path::Path) -> bool {
    let candidate = if cfg!(windows) {
        venv_root.join("Scripts").join("python.exe")
    } else {
        venv_root.join("bin").join("python")
    };
    candidate.is_file()
}
