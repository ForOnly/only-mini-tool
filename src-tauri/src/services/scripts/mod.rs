//! 脚本库：CRUD + Python 进程执行（无沙盒）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::AppHandle;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::{
    ScriptCreate, ScriptDto, ScriptParamDef, ScriptParamPassAs, ScriptParamType, ScriptRunRequest,
    ScriptRunResult, ScriptSummary, ScriptUpdate, ScriptsSettingsBundle, ScriptsSettingsSave,
    DEFAULT_SCRIPTS_ENV_JSON, DEFAULT_SCRIPTS_PYTHON_PATH, SETTING_SCRIPTS_DEFAULT_WORKSPACE,
    SETTING_SCRIPTS_ENV_JSON, SETTING_SCRIPTS_PYTHON_PATH,
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

    pub fn create(db: &Database, payload: ScriptCreate) -> Result<ScriptDto, AppError> {
        db.with_conn(|conn| ScriptsRepo::create(conn, &payload))
    }

    pub fn update(db: &Database, id: i64, payload: ScriptUpdate) -> Result<ScriptDto, AppError> {
        validate_params_schema(&payload.params_schema)?;
        db.with_conn(|conn| ScriptsRepo::update(conn, id, &payload))
    }

    pub fn delete(db: &Database, id: i64) -> Result<(), AppError> {
        db.with_conn(|conn| ScriptsRepo::delete(conn, id))
    }

    pub fn rename(db: &Database, id: i64, name: String) -> Result<ScriptDto, AppError> {
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
        validate_run_params(&script.params_schema, &payload.params)?;

        let settings = Self::get_settings(db)?;
        let interpreter = script
            .interpreter_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(settings.python_path.trim());
        if interpreter.is_empty() {
            return Err(AppError::ValidationError {
                message: "python interpreter path is empty".into(),
            });
        }

        let cwd = resolve_cwd(app, &script.workspace_path, &settings.default_workspace)?;
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

        let mut env = HashMap::new();
        for (k, v) in std::env::vars() {
            env.insert(k, v);
        }
        for (k, v) in &settings.env {
            env.insert(k.clone(), v.clone());
        }
        for (k, v) in &script.env {
            env.insert(k.clone(), v.clone());
        }

        let mut dyn_args = Vec::new();
        for def in &script.params_schema {
            let raw = payload.params.get(&def.key).cloned().unwrap_or_default();
            match def.pass_as {
                ScriptParamPassAs::Env => {
                    let env_key = format!("PARAM_{}", def.key.to_uppercase());
                    match def.param_type {
                        ScriptParamType::Boolean => {
                            let on = is_truthy(&raw);
                            if on {
                                env.insert(env_key, "1".into());
                            } else {
                                env.insert(env_key, "0".into());
                            }
                        }
                        _ => {
                            env.insert(env_key, raw);
                        }
                    }
                }
                ScriptParamPassAs::Arg => match def.param_type {
                    ScriptParamType::Boolean => {
                        if is_truthy(&raw) {
                            dyn_args.push(format!("--{}", def.key));
                        }
                    }
                    _ => {
                        if !raw.is_empty() || def.required {
                            dyn_args.push(format!("--{}", def.key));
                            dyn_args.push(raw);
                        }
                    }
                },
            }
        }

        let mut args = Vec::new();
        args.extend(script.args_template.before.iter().cloned());
        args.push(script_path.to_string_lossy().into_owned());
        args.extend(dyn_args);
        args.extend(script.args_template.after.iter().cloned());

        let result = match spawn_and_wait(gate, interpreter, &args, &cwd, &env).await {
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

fn resolve_cwd(
    app: &AppHandle,
    script_workspace: &Option<String>,
    default_workspace: &str,
) -> Result<PathBuf, AppError> {
    if let Some(p) = script_workspace
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        return Ok(PathBuf::from(p));
    }
    let def = default_workspace.trim();
    if !def.is_empty() {
        return Ok(PathBuf::from(def));
    }
    Ok(scripts_run_dir(app)?.join("default-workspace"))
}

fn validate_params_schema(schema: &[ScriptParamDef]) -> Result<(), AppError> {
    let mut keys = std::collections::HashSet::new();
    for def in schema {
        let key = def.key.trim();
        if key.is_empty() {
            return Err(AppError::ValidationError {
                message: "param key is required".into(),
            });
        }
        if !keys.insert(key.to_string()) {
            return Err(AppError::ValidationError {
                message: format!("duplicate param key: {key}"),
            });
        }
        if matches!(def.param_type, ScriptParamType::Select) && def.options.is_empty() {
            return Err(AppError::ValidationError {
                message: format!("select param requires options: {key}"),
            });
        }
    }
    Ok(())
}

fn validate_run_params(
    schema: &[ScriptParamDef],
    params: &HashMap<String, String>,
) -> Result<(), AppError> {
    for def in schema {
        let raw = params.get(&def.key).map(|s| s.as_str()).unwrap_or("");
        if def.required
            && raw.trim().is_empty()
            && !matches!(def.param_type, ScriptParamType::Boolean)
        {
            return Err(AppError::ValidationError {
                message: format!("required param missing: {}", def.key),
            });
        }
        if matches!(def.param_type, ScriptParamType::Number)
            && !raw.trim().is_empty()
            && raw.trim().parse::<f64>().is_err()
        {
            return Err(AppError::ValidationError {
                message: format!("param {} must be a number", def.key),
            });
        }
        if matches!(def.param_type, ScriptParamType::Select)
            && !raw.is_empty()
            && !def.options.iter().any(|o| o == raw)
        {
            return Err(AppError::ValidationError {
                message: format!("param {} invalid option", def.key),
            });
        }
    }
    Ok(())
}

fn is_truthy(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
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
