//! 脚本库：CRUD + Python 进程执行（无沙盒）。
//!
//! 分工：validate（校验纯函数）/ prepare（运行准备纯函数 + 临时脚本守卫）/
//! runner（执行内核：流式截断、取消令牌、杀树、运行注册表）。
//! 单飞策略在本层表达（registry 原子注册），内核对策略无感知。

pub mod prepare;
pub mod runner;
pub mod validate;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::{
    ScriptCreate, ScriptDto, ScriptRunRequest, ScriptRunResult, ScriptSummary, ScriptUpdate,
    ScriptsSettingsBundle, ScriptsSettingsSave, VenvStatus, DEFAULT_SCRIPTS_ENV_JSON,
    DEFAULT_SCRIPTS_ENV_PREFIX, DEFAULT_SCRIPTS_PYTHON_PATH, SETTING_SCRIPTS_DEFAULT_WORKSPACE,
    SETTING_SCRIPTS_ENV_JSON, SETTING_SCRIPTS_ENV_PREFIX, SETTING_SCRIPTS_PYTHON_PATH,
};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem::scripts_run_dir;
use crate::repository::scripts::ScriptsRepo;
use crate::services::scripts::prepare::TempScriptGuard;
use crate::services::scripts::runner::{ScriptRunRegistry, SpawnOptions};
use crate::services::settings_service::SettingsService;

/// 单次运行硬编码超时（不进 settings，Won't：scripts.timeout_ms）。
const RUN_TIMEOUT_SECS: u64 = 300;

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
        let env_prefix = SettingsService::get(db, SETTING_SCRIPTS_ENV_PREFIX)?
            .unwrap_or_else(|| DEFAULT_SCRIPTS_ENV_PREFIX.to_string());
        let env_raw = SettingsService::get(db, SETTING_SCRIPTS_ENV_JSON)?
            .unwrap_or_else(|| DEFAULT_SCRIPTS_ENV_JSON.to_string());
        let env: HashMap<String, String> =
            serde_json::from_str(&env_raw).map_err(|e| AppError::ValidationError {
                message: format!("invalid scripts.env_json: {e}"),
            })?;
        Ok(ScriptsSettingsBundle {
            python_path,
            default_workspace,
            env_prefix,
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
        let env_prefix = validate::validate_env_prefix(&payload.env_prefix)?;
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
        SettingsService::set_raw(db, SETTING_SCRIPTS_ENV_PREFIX, &env_prefix)?;
        SettingsService::set_raw(db, SETTING_SCRIPTS_ENV_JSON, &env_json)?;
        Ok(())
    }

    /// 取消当前运行（无运行时为无害空操作）。
    pub fn cancel_run() {
        ScriptRunRegistry::global().cancel_all();
    }

    /// 在全局默认 workspace 创建 `.venv`（复用执行内核，与脚本运行共享单飞）。
    pub async fn create_venv(db: &Database) -> Result<(), AppError> {
        let settings = Self::get_settings(db)?;
        let workspace = settings.default_workspace.trim();
        if workspace.is_empty() {
            return Err(AppError::ValidationError {
                message: "scripts.default_workspace is required to create a venv".into(),
            });
        }
        let venv = venv_python_at(Path::new(workspace)).await;
        if venv.is_some() {
            return Ok(()); // 已存在：幂等
        }

        let registry = ScriptRunRegistry::global();
        let run_id = Uuid::new_v4();
        let Some(token) = registry.try_register(run_id) else {
            return Err(AppError::ScriptsBusy {
                message: "a script is already running".into(),
            });
        };
        let result = Self::create_venv_registered(&settings, workspace, token).await;
        registry.unregister(&run_id);
        result
    }

    async fn create_venv_registered(
        settings: &ScriptsSettingsBundle,
        workspace: &str,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        for (k, v) in &settings.env {
            env.insert(k.clone(), v.clone());
        }
        prepare::ensure_stdio_utf8(&mut env);

        let outcome = runner::spawn_and_stream(
            SpawnOptions {
                interpreter: settings.python_path.trim().to_string(),
                args: vec!["-m".into(), "venv".into(), ".venv".into()],
                cwd: PathBuf::from(workspace),
                env,
                stdin_json: serde_json::json!({}),
                timeout: Duration::from_secs(RUN_TIMEOUT_SECS),
            },
            token,
        )
        .await?;

        if outcome.exit_code == Some(0) {
            Ok(())
        } else {
            Err(AppError::InternalError {
                message: format!(
                    "python -m venv failed (exit {:?}): {}",
                    outcome.exit_code,
                    outcome.stderr.trim()
                ),
            })
        }
    }

    /// workspace venv 状态（前端无 fs 权限，由后端报）。
    pub async fn venv_status(db: &Database) -> Result<VenvStatus, AppError> {
        let settings = Self::get_settings(db)?;
        let workspace = settings.default_workspace.trim().to_string();
        let venv_python = if workspace.is_empty() {
            None
        } else {
            venv_python_at(Path::new(&workspace)).await
        };
        Ok(VenvStatus {
            workspace,
            venv_python,
        })
    }

    pub async fn run(
        app: &AppHandle,
        db: &Database,
        payload: ScriptRunRequest,
    ) -> Result<ScriptRunResult, AppError> {
        // 单飞策略：registry 原子 check+insert（机制与策略分离）
        let registry = ScriptRunRegistry::global();
        let run_id = Uuid::new_v4();
        let Some(token) = registry.try_register(run_id) else {
            return Err(AppError::ScriptsBusy {
                message: "a script is already running".into(),
            });
        };

        let result = Self::run_registered(app, db, payload, token).await;
        registry.unregister(&run_id);
        result
    }

    async fn run_registered(
        app: &AppHandle,
        db: &Database,
        payload: ScriptRunRequest,
        token: CancellationToken,
    ) -> Result<ScriptRunResult, AppError> {
        let script = Self::get(db, payload.script_id)?;
        if script.language != "python" {
            return Err(AppError::ValidationError {
                message: format!("unsupported language: {}", script.language),
            });
        }

        let effective = prepare::effective_params(&script.params_schema, &payload.params);
        validate::validate_run_params(&script.params_schema, &effective)?;
        let settings = Self::get_settings(db)?;
        let projected =
            prepare::project_params(&script.params_schema, &effective, &settings.env_prefix);

        // 先解析 cwd，再探测其下 venv，最后定解释器（链：脚本覆盖 > venv > 全局）
        let fallback_dir = scripts_run_dir(app)?.join("default-workspace");
        let cwd = prepare::resolve_cwd(
            script.workspace_path.as_deref(),
            &settings.default_workspace,
            fallback_dir,
        );
        tokio::fs::create_dir_all(&cwd)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("create workspace cwd: {e}"),
            })?;
        let venv_python = venv_python_at(&cwd).await;
        let interpreter = prepare::resolve_interpreter(
            script.interpreter_path.as_deref(),
            venv_python.as_deref(),
            &settings.python_path,
        );
        if interpreter.is_empty() {
            return Err(AppError::ValidationError {
                message: "python interpreter path is empty".into(),
            });
        }

        let run_dir = scripts_run_dir(app)?;
        tokio::fs::create_dir_all(&run_dir)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("create scripts run dir: {e}"),
            })?;
        cleanup_orphan_run_scripts(&run_dir).await;
        let guard = TempScriptGuard::create(&run_dir, &script.body).await?;

        let process_env: HashMap<String, String> = std::env::vars().collect();
        let mut env = prepare::merge_env(process_env, &settings.env, &script.env, &projected.envs);
        prepare::ensure_stdio_utf8(&mut env);
        let args = prepare::build_args(&script.args_template, guard.path(), &projected.args);
        let command = prepare::display_command(
            &interpreter,
            &script.name,
            &script.args_template,
            &projected.args,
        );

        let outcome = runner::spawn_and_stream(
            SpawnOptions {
                interpreter,
                args,
                cwd,
                env,
                stdin_json: Value::Object(projected.stdin),
                timeout: Duration::from_secs(RUN_TIMEOUT_SECS),
            },
            token,
        )
        .await?;

        Ok(ScriptRunResult {
            exit_code: outcome.exit_code,
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            cancelled: outcome.cancelled,
            command,
        })
    }
}

/// 探测 workspace 下 `.venv` 的解释器路径（存在才返回）。
/// Windows `.venv/Scripts/python.exe`，Unix `.venv/bin/python`。
async fn venv_python_at(workspace: &Path) -> Option<String> {
    let candidate = if cfg!(windows) {
        workspace.join(".venv").join("Scripts").join("python.exe")
    } else {
        workspace.join(".venv").join("bin").join("python")
    };
    match tokio::fs::try_exists(&candidate).await {
        Ok(true) => Some(candidate.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// 下次运行前清理孤儿临时脚本（app 崩溃/断电残留）。
async fn cleanup_orphan_run_scripts(run_dir: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(run_dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("run-") && name.ends_with(".py") {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}
