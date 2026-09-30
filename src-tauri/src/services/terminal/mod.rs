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

/// 登记表条目：会话 + 归属窗口 label（创建窗口；被非 main 窗口 attach 后移交）。
struct OwnedSession {
    session: Arc<TerminalSession>,
    owner: String,
}

#[derive(Default)]
struct RegistryState {
    map: HashMap<String, OwnedSession>,
    /// spawn 进行中的预留计数（与 map 合计参与上限判定，防并发超限）。
    reserved: usize,
}

/// 上限预留（单锁临界区内原子 check+reserve；独立纯函数便于单测）。
fn try_reserve(state: &mut RegistryState) -> Result<(), AppError> {
    if state.map.len() + state.reserved >= MAX_SESSIONS {
        return Err(AppError::TerminalLimitReached {
            message: format!(
                "terminal session limit reached ({MAX_SESSIONS}) — close old sessions first"
            ),
        });
    }
    state.reserved += 1;
    Ok(())
}

pub struct TerminalRegistry {
    state: Mutex<RegistryState>,
}

impl TerminalRegistry {
    pub fn global() -> &'static TerminalRegistry {
        static REGISTRY: OnceLock<TerminalRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| TerminalRegistry {
            state: Mutex::new(RegistryState::default()),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RegistryState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 创建会话：探测 shell → 解析 cwd/venv → env 组装 → PTY spawn → 注册。
    /// `owner` = 创建窗口 label（子窗 attach 后经 attach_owner 移交）。
    pub fn create(
        &self,
        app: &AppHandle,
        db: &Database,
        payload: TerminalCreatePayload,
        owner: &str,
    ) -> Result<TerminalInfo, AppError> {
        try_reserve(&mut self.lock())?;

        let spawned = self.spawn_session(app, db, &payload);
        let mut state = self.lock();
        state.reserved = state.reserved.saturating_sub(1);
        match spawned {
            Ok(session) => {
                let info = session.snapshot();
                state.map.insert(
                    session.id.clone(),
                    OwnedSession {
                        session,
                        owner: owner.to_string(),
                    },
                );
                Ok(info)
            }
            Err(err) => Err(err),
        }
    }

    fn spawn_session(
        &self,
        app: &AppHandle,
        db: &Database,
        payload: &TerminalCreatePayload,
    ) -> Result<Arc<TerminalSession>, AppError> {
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
        let mut env_overlay = payload.env.clone().unwrap_or_default();
        if let (Some(root), Some(name)) = (&venv_root, venv_ref) {
            for (key, value) in shell::venv_env(root, name) {
                env_overlay.insert(key, value);
            }
        }

        let spawn_cfg = shell::spawn_command(&kind, venv_ref);
        // 编码/提示符经启动参数注入（零回显）；init_commands 仅剩用户自定义
        // （会以敲入形式回显——显式 opt-in，文档注明）
        let init_commands = payload.init_commands.clone().unwrap_or_default();

        TerminalSession::spawn(SessionSpawn {
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
        })
    }

    pub fn get(&self, id: &str) -> Option<Arc<TerminalSession>> {
        self.lock().map.get(id).map(|o| Arc::clone(&o.session))
    }

    /// 归属移交：非 main 窗口 attach 即取得所有权（main 的双附着不夺回——
    /// 弹出子窗才是归属方；关窗 = Destroyed → dispose_owned_by）。
    pub fn attach_owner(&self, id: &str, caller: &str) {
        if caller == "main" {
            return;
        }
        if let Some(owned) = self.lock().map.get_mut(id) {
            owned.owner = caller.to_string();
        }
    }

    pub fn remove(&self, id: &str) {
        self.lock().map.remove(id);
    }

    pub fn list(&self) -> Vec<TerminalInfo> {
        self.lock()
            .map
            .values()
            .map(|o| o.session.snapshot())
            .collect()
    }

    /// 销毁某窗口名下全部会话（窗口 Destroyed 时调用——覆盖子窗正常关闭与
    /// webview 异常消亡）。返回销毁数（供 tracing）。
    pub fn dispose_owned_by(&self, label: &str) -> usize {
        let mut state = self.lock();
        let ids: Vec<String> = state
            .map
            .iter()
            .filter(|(_, owned)| owned.owner == label)
            .map(|(id, _)| id.clone())
            .collect();
        for id in &ids {
            if let Some(owned) = state.map.remove(id) {
                owned.session.dispose();
            }
        }
        ids.len()
    }

    /// app 退出清理：杀全部会话进程树（PTY 随进程树终结关闭）。
    pub fn dispose_all(&self) {
        let sessions: Vec<Arc<TerminalSession>> = self
            .lock()
            .map
            .values()
            .map(|o| Arc::clone(&o.session))
            .collect();
        for session in sessions {
            session.dispose();
        }
        self.lock().map.clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_enforces_limit_atomically() {
        let mut state = RegistryState::default();
        for _ in 0..MAX_SESSIONS {
            try_reserve(&mut state).expect("reserve within limit");
        }
        assert!(try_reserve(&mut state).is_err(), "over limit rejected");
        // 释放一个预留后可再预留
        state.reserved -= 1;
        assert!(try_reserve(&mut state).is_ok());
        // 登记数同样计入上限
        state.reserved = 0;
        for _ in 0..MAX_SESSIONS {
            let session = TerminalSession::spawn(SessionSpawn {
                program: if cfg!(windows) { "cmd.exe".into() } else { "/bin/sh".into() },
                args: vec![],
                cwd: std::env::temp_dir(),
                env_overlay: Default::default(),
                cols: 20,
                rows: 5,
                shell_label: "test".into(),
                venv_label: None,
                title: None,
                init_commands: vec![],
            })
            .expect("spawn test session");
            state.map.insert(
                session.id.clone(),
                OwnedSession {
                    session,
                    owner: "test".into(),
                },
            );
        }
        assert!(try_reserve(&mut state).is_err(), "map count also enforces limit");
    }
}
