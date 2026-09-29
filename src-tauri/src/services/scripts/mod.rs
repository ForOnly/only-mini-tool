//! 脚本库：CRUD + Python 进程执行（无沙盒）。
//!
//! 分工：validate（校验纯函数）/ prepare（运行准备纯函数 + 临时脚本守卫）/
//! runner（执行内核：流式截断、取消令牌、杀树、运行注册表）。
//! 单飞策略在本层表达（registry 原子注册），内核对策略无感知。

pub mod prepare;
pub mod runner;
pub mod validate;

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::domain::{
    ScriptCreate, ScriptDto, ScriptRunRequest, ScriptRunResult, ScriptSummary, ScriptUpdate,
    ScriptVenvSummary, ScriptsSettingsBundle, ScriptsSettingsSave, DEFAULT_SCRIPTS_ENV_JSON,
    DEFAULT_SCRIPTS_ENV_PREFIX, DEFAULT_SCRIPTS_PYTHON_PATH, SETTING_SCRIPTS_DEFAULT_WORKSPACE,
    SETTING_SCRIPTS_ENV_JSON, SETTING_SCRIPTS_ENV_PREFIX, SETTING_SCRIPTS_PYTHON_PATH,
    SETTING_SCRIPTS_VENV,
};
use crate::errors::AppError;
use crate::infrastructure::database::Database;
use crate::infrastructure::filesystem::{scripts_run_dir, venvs_dir};
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

    pub async fn update(
        app: &AppHandle,
        db: &Database,
        id: i64,
        mut payload: ScriptUpdate,
    ) -> Result<ScriptDto, AppError> {
        payload.name = validate::validate_name(&payload.name)?;
        payload.description = payload.description.trim().to_string();
        validate::validate_params_schema(&payload.params_schema)?;
        // venv 绑定：非空须名称合法且 venv 存在（防拼写错误静默回落）
        if let Some(venv) = payload
            .venv_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let name = validate::validate_venv_name(venv)?;
            if named_venv_python(app, &name).await.is_none() {
                return Err(AppError::ValidationError {
                    message: format!("venv not found: {name}"),
                });
            }
            payload.venv_name = Some(name);
        }
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
        let venv = SettingsService::get(db, SETTING_SCRIPTS_VENV)?.unwrap_or_default();
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
            venv,
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
        // 全局启用 venv：仅校验名称合法（不校验存在性——运行时解析容错回落）
        let venv = payload.venv.trim();
        if !venv.is_empty() {
            validate::validate_venv_name(venv)?;
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
        SettingsService::set_raw(db, SETTING_SCRIPTS_ENV_PREFIX, &env_prefix)?;
        SettingsService::set_raw(db, SETTING_SCRIPTS_VENV, venv)?;
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
        let dir = Path::new(workspace).join(".venv");
        if python_exists(&dir).await.is_some() {
            return Ok(()); // 已存在：幂等
        }
        Self::spawn_venv_create(&settings, &dir).await
    }

    /// 创建命名 venv（工具托管 `venvs/<name>/`）。
    pub async fn create_named_venv(
        app: &AppHandle,
        db: &Database,
        name: &str,
    ) -> Result<(), AppError> {
        let name = validate::validate_venv_name(name)?;
        let settings = Self::get_settings(db)?;
        let root = venvs_dir(app)?;
        tokio::fs::create_dir_all(&root)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("create venvs dir: {e}"),
            })?;
        let dir = root.join(&name);
        if python_exists(&dir).await.is_some() {
            return Ok(()); // 已存在：幂等
        }
        Self::spawn_venv_create(&settings, &dir).await
    }

    /// 单飞守卫下执行 `python -m venv <dir>`（与脚本运行共享 registry 互斥）。
    async fn spawn_venv_create(
        settings: &ScriptsSettingsBundle,
        dir: &Path,
    ) -> Result<(), AppError> {
        let registry = ScriptRunRegistry::global();
        let run_id = Uuid::new_v4();
        let Some(token) = registry.try_register(run_id) else {
            return Err(AppError::ScriptsBusy {
                message: "a script is already running".into(),
            });
        };
        let result = Self::venv_create_registered(settings, dir, token).await;
        registry.unregister(&run_id);
        result
    }

    async fn venv_create_registered(
        settings: &ScriptsSettingsBundle,
        dir: &Path,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        for (k, v) in &settings.env {
            env.insert(k.clone(), v.clone());
        }
        prepare::ensure_stdio_utf8(&mut env);

        // cwd 取目标父目录（workspace 场景即 workspace 本身）；不存在则创建
        let cwd = dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| dir.to_path_buf());
        tokio::fs::create_dir_all(&cwd)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("create venv parent dir: {e}"),
            })?;

        let outcome = runner::spawn_and_stream(
            SpawnOptions {
                interpreter: settings.python_path.trim().to_string(),
                args: vec![
                    "-m".into(),
                    "venv".into(),
                    dir.to_string_lossy().into_owned(),
                ],
                cwd,
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

    /// venv 列表：托管 `venvs/` 扫描 + 默认 workspace `.venv` 特殊条目。
    pub async fn list_venvs(
        app: &AppHandle,
        db: &Database,
    ) -> Result<Vec<ScriptVenvSummary>, AppError> {
        let mut out = Vec::new();
        let root = venvs_dir(app)?;
        if let Ok(mut entries) = tokio::fs::read_dir(&root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if validate::validate_venv_name(&name).is_err() {
                    continue; // 非法目录名不纳入管理
                }
                if let Some(py) = python_exists(&entry.path()).await {
                    out.push(ScriptVenvSummary {
                        name,
                        python_path: py,
                        workspace: false,
                    });
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));

        let settings = Self::get_settings(db)?;
        let ws = settings.default_workspace.trim();
        if !ws.is_empty() {
            if let Some(py) = venv_python_at(Path::new(ws)).await {
                out.push(ScriptVenvSummary {
                    name: ".venv".into(),
                    python_path: py,
                    workspace: true,
                });
            }
        }
        Ok(out)
    }

    /// 删除 venv。`name == ".venv"` 删默认 workspace 下的 `.venv`（特殊条目）；
    /// 命名 venv 三重防护：白名单、canonicalize 仍在 `venvs/` 前缀下、
    /// 引用检查（脚本绑定或全局启用时拒绝）。
    pub async fn delete_venv(app: &AppHandle, db: &Database, name: &str) -> Result<(), AppError> {
        if name == ".venv" {
            let settings = Self::get_settings(db)?;
            let ws = settings.default_workspace.trim();
            if ws.is_empty() {
                return Err(AppError::ValidationError {
                    message: "scripts.default_workspace is not configured".into(),
                });
            }
            let dir = Path::new(ws).join(".venv");
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| AppError::InternalError {
                    message: format!("remove workspace .venv: {e}"),
                })?;
            return Ok(());
        }

        let name = validate::validate_venv_name(name)?;

        // 引用检查：脚本绑定
        let bound: Vec<String> = db.with_conn(|conn| ScriptsRepo::names_by_venv(conn, &name))?;
        if !bound.is_empty() {
            return Err(AppError::ValidationError {
                message: format!(
                    "venv \"{name}\" is bound to scripts: {} (unbind them first)",
                    bound.join(", ")
                ),
            });
        }
        // 引用检查：全局启用
        let settings = Self::get_settings(db)?;
        if settings.venv.trim() == name {
            return Err(AppError::ValidationError {
                message: format!(
                    "venv \"{name}\" is globally enabled (disable it in settings first)"
                ),
            });
        }

        // 路径防护：canonicalize 后必须仍在 venvs 根下
        let root = venvs_dir(app)?;
        let dir = root.join(&name);
        let root_canon =
            tokio::fs::canonicalize(&root)
                .await
                .map_err(|e| AppError::InternalError {
                    message: format!("resolve venvs dir: {e}"),
                })?;
        let dir_canon =
            tokio::fs::canonicalize(&dir)
                .await
                .map_err(|_| AppError::ValidationError {
                    message: format!("venv not found: {name}"),
                })?;
        if !dir_canon.starts_with(&root_canon) {
            return Err(AppError::ValidationError {
                message: "venv path escapes the managed venvs directory".into(),
            });
        }
        tokio::fs::remove_dir_all(&dir_canon)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("remove venv: {e}"),
            })?;
        Ok(())
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
        // venv 候选（5 级链的 venv 部分）：脚本绑定 > 全局启用 > workspace .venv；
        // 任一级指向的 venv 不存在则静默回落下级（解析容错）
        let bound_venv = script
            .venv_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let venv_python = match bound_venv {
            Some(name) => named_venv_python(app, name).await,
            None => {
                let global_venv = settings.venv.trim();
                if !global_venv.is_empty() {
                    named_venv_python(app, global_venv)
                        .await
                        .or(venv_python_at(&cwd).await)
                } else {
                    venv_python_at(&cwd).await
                }
            }
        };
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
async fn venv_python_at(workspace: &Path) -> Option<String> {
    python_exists(&workspace.join(".venv")).await
}

/// 探测 venv 根目录下的平台 python 可执行（存在才返回路径字符串）。
async fn python_exists(venv_root: &Path) -> Option<String> {
    let candidate = if cfg!(windows) {
        venv_root.join("Scripts").join("python.exe")
    } else {
        venv_root.join("bin").join("python")
    };
    match tokio::fs::try_exists(&candidate).await {
        Ok(true) => Some(candidate.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// 命名 venv 的解释器路径（`venvs/<name>/`，存在才返回）。
async fn named_venv_python(app: &AppHandle, name: &str) -> Option<String> {
    let root = venvs_dir(app).ok()?;
    python_exists(&root.join(name)).await
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
