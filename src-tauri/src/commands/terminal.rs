//! 终端 IPC 薄层（一 command 一函数，业务在 services/terminal）。

use tauri::ipc::Channel;
use tauri::{AppHandle, State};

use crate::domain::{
    ScriptTerminalConfig, TerminalCreatePayload, TerminalEvent, TerminalInfo,
};
use crate::errors::AppError;
use crate::services::scripts::ScriptsService;
use crate::services::terminal::TerminalRegistry;
use crate::state::AppState;

#[tauri::command]
pub fn terminal_create(
    app: AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    payload: TerminalCreatePayload,
) -> Result<TerminalInfo, AppError> {
    TerminalRegistry::global().create(&app, &state.db, payload, window.label())
}

/// 附着会话：Channel 首条消息为 Replay（scrollback 快照），此后为增量 Output；
/// 返回 attachId 供视图卸载时 detach。非 main 窗口 attach 即取得会话归属
/// （关窗 = Destroyed → 自动销毁，覆盖子窗异常消亡）。
#[tauri::command]
pub fn terminal_attach(
    window: tauri::Window,
    id: String,
    on_event: Channel<TerminalEvent>,
) -> Result<String, AppError> {
    let session = find_session(&id)?;
    let attach_id = session.attach(on_event);
    TerminalRegistry::global().attach_owner(&id, window.label());
    Ok(attach_id)
}

#[tauri::command]
pub fn terminal_detach(id: String, attach_id: String) -> Result<(), AppError> {
    if let Some(session) = TerminalRegistry::global().get(&id) {
        session.detach(&attach_id);
    }
    Ok(())
}

#[tauri::command]
pub fn terminal_write(id: String, data: String) -> Result<(), AppError> {
    find_session(&id)?.write(&data)
}

#[tauri::command]
pub fn terminal_resize(id: String, cols: u16, rows: u16) -> Result<(), AppError> {
    find_session(&id)?.resize(cols, rows)
}

/// 幂等销毁：会话不存在视为已销毁。
#[tauri::command]
pub fn terminal_dispose(id: String) -> Result<(), AppError> {
    if let Some(session) = TerminalRegistry::global().get(&id) {
        session.dispose();
        TerminalRegistry::global().remove(&id);
    }
    Ok(())
}

#[tauri::command]
pub fn terminal_list() -> Result<Vec<TerminalInfo>, AppError> {
    Ok(TerminalRegistry::global().list())
}

/// 脚本编辑页终端预设：复用 run 的 cwd/venv 解析链（行为一致）。
#[tauri::command]
pub async fn resolve_script_terminal(
    app: AppHandle,
    state: State<'_, AppState>,
    script_id: i64,
) -> Result<ScriptTerminalConfig, AppError> {
    let script = ScriptsService::get(&state.db, script_id)?;
    let target = ScriptsService::resolve_run_target(&app, &state.db, &script).await?;
    // 终端侧确保目录存在（run 路径的 create_dir_all 留在 run 内，此处补齐）
    let _ = tokio::fs::create_dir_all(&target.cwd).await;
    Ok(ScriptTerminalConfig {
        cwd: target.cwd.to_string_lossy().into_owned(),
        venv_name: target.venv_effective.clone(),
        venv_python: target.venv_python.clone(),
    })
}

fn find_session(
    id: &str,
) -> Result<std::sync::Arc<crate::services::terminal::TerminalSession>, AppError> {
    TerminalRegistry::global().get(id).ok_or_else(|| {
        AppError::TerminalNotFound {
            message: format!("terminal session not found: {id}"),
        }
    })
}
