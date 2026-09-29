//! 脚本库：CRUD + Python 进程执行（无沙盒）。

pub mod prepare;
pub mod validate;

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::AppHandle;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::{
    ScriptCreate, ScriptDto, ScriptRunRequest, ScriptRunResult, ScriptSummary, ScriptUpdate,
    ScriptsSettingsBundle, ScriptsSettingsSave, DEFAULT_SCRIPTS_ENV_JSON,
    DEFAULT_SCRIPTS_PYTHON_PATH, SETTING_SCRIPTS_DEFAULT_WORKSPACE, SETTING_SCRIPTS_ENV_JSON,
    SETTING_SCRIPTS_PYTHON_PATH,
};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem::scripts_run_dir;
use crate::repository::scripts::ScriptsRepo;
use crate::services::settings_service::SettingsService;

/// 单次运行硬编码超时（不进 settings）。
const RUN_TIMEOUT_SECS: u64 = 300;
/// stdout / stderr 各截断上限。
const OUTPUT_CAP_BYTES: usize = 1024 * 1024;

struct RunGate {
    lock: Mutex<()>,
    child_id: Mutex<Option<u32>>,
    cancelled: AtomicBool,
}

fn run_gate() -> &'static RunGate {
    static GATE: OnceLock<RunGate> = OnceLock::new();
    GATE.get_or_init(|| RunGate {
        lock: Mutex::new(()),
        child_id: Mutex::new(None),
        cancelled: AtomicBool::new(false),
    })
}

pub struct ScriptsService;

impl ScriptsService {
    pub fn list(db: &Database) -> Result<Vec<ScriptSummary>, AppError> {
        db.with_conn(ScriptsRepo::list)
    }

    pub fn get(db: &Database, id: i64) -> Result<ScriptDto, AppError> {
        db.with_conn(|conn| ScriptsRepo::get(conn, id))
    }

    pub fn create(db: &Database, mut payload: ScriptCreate) -> Result<ScriptDto, AppError> {
        payload.name = validate::validate_name(&payload.name)?;
        payload.description = payload.description.trim().to_string();
        db.with_conn(|conn| ScriptsRepo::create(conn, &payload))
    }

    pub fn update(
        db: &Database,
        id: i64,
        mut payload: ScriptUpdate,
    ) -> Result<ScriptDto, AppError> {
        payload.name = validate::validate_name(&payload.name)?;
        payload.description = payload.description.trim().to_string();
        validate::validate_params_schema(&payload.params_schema)?;
        db.with_conn(|conn| ScriptsRepo::update(conn, id, &payload))
    }

    pub fn delete(db: &Database, id: i64) -> Result<(), AppError> {
        db.with_conn(|conn| ScriptsRepo::delete(conn, id))
    }

    pub fn rename(db: &Database, id: i64, name: String) -> Result<ScriptDto, AppError> {
        let name = validate::validate_name(&name)?;
        db.with_conn(|conn| ScriptsRepo::rename(conn, id, &name))
    }

    pub fn get_settings(db: &Database) -> Result<ScriptsSettingsBundle, AppError> {
        let python_path = SettingsService::get(db, SETTING_SCRIPTS_PYTHON_PATH)?
            .unwrap_or_else(|| DEFAULT_SCRIPTS_PYTHON_PATH.to_string());
        let default_workspace =
            SettingsService::get(db, SETTING_SCRIPTS_DEFAULT_WORKSPACE)?.unwrap_or_default();
        let env_raw = SettingsService::get(db, SETTING_SCRIPTS_ENV_JSON)?
            .unwrap_or_else(|| DEFAULT_SCRIPTS_ENV_JSON.to_string());
        let env: HashMap<String, String> =
            serde_json::from_str(&env_raw).map_err(|e| AppError::ValidationError {
                message: format!("invalid scripts.env_json: {e}"),
            })?;
        Ok(ScriptsSettingsBundle {
            python_path,
            default_workspace,
            env,
        })
    }

    pub fn save_settings(db: &Database, payload: ScriptsSettingsSave) -> Result<(), AppError> {
        let python = payload.python_path.trim();
        if python.is_empty() {
            return Err(AppError::ValidationError {
                message: "scripts.python_path is required".into(),
            });
        }
        let env_json =
            serde_json::to_string(&payload.env).map_err(|e| AppError::InternalError {
                message: format!("serialize scripts.env: {e}"),
            })?;
        SettingsService::set_raw(db, SETTING_SCRIPTS_PYTHON_PATH, python)?;
        SettingsService::set_raw(
            db,
            SETTING_SCRIPTS_DEFAULT_WORKSPACE,
            payload.default_workspace.trim(),
        )?;
        SettingsService::set_raw(db, SETTING_SCRIPTS_ENV_JSON, &env_json)?;
        Ok(())
    }

    pub fn cancel_run() {
        let gate = run_gate();
        gate.cancelled.store(true, Ordering::SeqCst);
        // 尽力杀子进程（Windows/Unix）
        if let Ok(guard) = gate.child_id.try_lock() {
            if let Some(pid) = *guard {
                kill_pid(pid);
            }
        }
    }

    pub async fn run(
        app: &AppHandle,
        db: &Database,
        payload: ScriptRunRequest,
    ) -> Result<ScriptRunResult, AppError> {
        let gate = run_gate();
        let _guard = match gate.lock.try_lock() {
            Ok(g) => g,
            Err(_) => {
                return Err(AppError::ScriptsBusy {
                    message: "a script is already running".into(),
                });
            }
        };
        gate.cancelled.store(false, Ordering::SeqCst);
        *gate.child_id.lock().await = None;

        let script = Self::get(db, payload.script_id)?;
        if script.language != "python" {
            return Err(AppError::ValidationError {
                message: format!("unsupported language: {}", script.language),
            });
        }

        let effective = prepare::effective_params(&script.params_schema, &payload.params);
        validate::validate_run_params(&script.params_schema, &effective)?;

        let settings = Self::get_settings(db)?;
        let interpreter =
            prepare::resolve_interpreter(script.interpreter_path.as_deref(), &settings.python_path);
        if interpreter.is_empty() {
            return Err(AppError::ValidationError {
                message: "python interpreter path is empty".into(),
            });
        }

        let fallback_dir = scripts_run_dir(app)?.join("default-workspace");
        let cwd = prepare::resolve_cwd(
            script.workspace_path.as_deref(),
            &settings.default_workspace,
            fallback_dir,
        );
        std::fs::create_dir_all(&cwd).map_err(|e| AppError::InternalError {
            message: format!("create workspace cwd: {e}"),
        })?;

        let run_dir = scripts_run_dir(app)?;
        std::fs::create_dir_all(&run_dir).map_err(|e| AppError::InternalError {
            message: format!("create scripts run dir: {e}"),
        })?;
        cleanup_orphan_run_scripts(&run_dir);
        let script_path = run_dir.join(format!("run-{}.py", Uuid::new_v4()));
        std::fs::write(&script_path, script.body.as_bytes()).map_err(|e| {
            AppError::InternalError {
                message: format!("write temp script: {e}"),
            }
        })?;

        let projected = prepare::project_params(&script.params_schema, &effective);
        let process_env: HashMap<String, String> = std::env::vars().collect();
        let env = prepare::merge_env(process_env, &settings.env, &script.env, &projected.envs);
        // passAs=stdin 的参数（projected.stdin）由执行内核以 JSON 文档写入子进程 stdin
        let args = prepare::build_args(&script.args_template, &script_path, &projected.args);

        let result = match spawn_and_wait(gate, &interpreter, &args, &cwd, &env).await {
            Ok(r) => r,
            Err(e) => {
                let _ = std::fs::remove_file(&script_path);
                *gate.child_id.lock().await = None;
                return Err(e);
            }
        };

        let _ = std::fs::remove_file(&script_path);
        *gate.child_id.lock().await = None;
        Ok(result)
    }
}

async fn spawn_and_wait(
    gate: &RunGate,
    interpreter: &str,
    args: &[String],
    cwd: &Path,
    env: &HashMap<String, String>,
) -> Result<ScriptRunResult, AppError> {
    let mut cmd = Command::new(interpreter);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| AppError::InternalError {
        message: format!("failed to start python ({interpreter}): {e}"),
    })?;

    if let Some(pid) = child.id() {
        *gate.child_id.lock().await = Some(pid);
    }

    let cancel_watch = async {
        loop {
            if gate.cancelled.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    };

    let wait_fut = child.wait_with_output();
    tokio::pin!(wait_fut);

    let output = tokio::select! {
        _ = cancel_watch => {
            if let Ok(guard) = gate.child_id.try_lock() {
                if let Some(pid) = *guard {
                    kill_pid(pid);
                }
            }
            match wait_fut.await {
                Ok(out) => finish_output(out, true, false),
                Err(e) => {
                    return Err(AppError::InternalError {
                        message: format!("wait cancelled script: {e}"),
                    });
                }
            }
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(RUN_TIMEOUT_SECS)) => {
            if let Ok(guard) = gate.child_id.try_lock() {
                if let Some(pid) = *guard {
                    kill_pid(pid);
                }
            }
            match wait_fut.await {
                Ok(out) => finish_output(out, false, true),
                Err(e) => {
                    return Err(AppError::InternalError {
                        message: format!("wait timed-out script: {e}"),
                    });
                }
            }
        }
        res = &mut wait_fut => {
            match res {
                Ok(out) => finish_output(out, gate.cancelled.load(Ordering::SeqCst), false),
                Err(e) => {
                    return Err(AppError::InternalError {
                        message: format!("wait script process: {e}"),
                    });
                }
            }
        }
    };

    Ok(output)
}

fn finish_output(out: std::process::Output, cancelled: bool, timed_out: bool) -> ScriptRunResult {
    let stdout = truncate_output(String::from_utf8_lossy(&out.stdout).into_owned());
    let mut stderr = truncate_output(String::from_utf8_lossy(&out.stderr).into_owned());
    if timed_out {
        let note = format!("\n\n[scripts] run timed out after {RUN_TIMEOUT_SECS}s and was killed");
        if stderr.is_empty() {
            stderr = note.trim_start().to_string();
        } else {
            stderr.push_str(&note);
        }
    }
    ScriptRunResult {
        exit_code: out.status.code(),
        stdout,
        stderr,
        cancelled,
    }
}

fn truncate_output(raw: String) -> String {
    if raw.len() <= OUTPUT_CAP_BYTES {
        return raw;
    }
    let mut cut = OUTPUT_CAP_BYTES;
    while cut > 0 && !raw.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(
        "{}\n\n… [truncated, total {} bytes, cap {}]",
        &raw[..cut],
        raw.len(),
        OUTPUT_CAP_BYTES
    )
}

fn cleanup_orphan_run_scripts(run_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(run_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("run-") && name.ends_with(".py") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}
