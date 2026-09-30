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
use std::process::Stdio;
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

/// cwd + venv 候选链解析结果（run 与终端共用）。
pub struct ResolvedRunTarget {
    /// 生效工作目录
    pub cwd: PathBuf,
    /// 脚本绑定的 venv 引用（原始名；未绑定为 None——即使 venv 缺失也保留绑定名）
    pub bound_venv: Option<String>,
    /// 实际提供解释器的 venv 引用（绑定名 / 全局启用名 / ".venv"）；None = 全局 python
    pub venv_effective: Option<String>,
    /// 生效 venv 的解释器绝对路径（探测失败/无 venv 为 None）
    pub venv_python: Option<String>,
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

    pub async fn update(
        app: &AppHandle,
        db: &Database,
        id: i64,
        mut payload: ScriptUpdate,
    ) -> Result<ScriptDto, AppError> {
        payload.name = validate::validate_name(&payload.name)?;
        payload.description = payload.description.trim().to_string();
        validate::validate_params_schema(&payload.params_schema)?;
        // venv 绑定：非空须存在（`.venv` = 默认 workspace；命名 = venvs/<name>）
        if let Some(venv) = payload
            .venv_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let name = normalize_venv_ref(venv)?;
            require_venv_root(app, db, &name).await?;
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

    pub async fn save_settings(
        app: &AppHandle,
        db: &Database,
        payload: ScriptsSettingsSave,
    ) -> Result<(), AppError> {
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
        // 保存前以最严苛场景（命名 venv 创建 cwd = venvs 目录）探测 python 可用性：
        // mise/pyenv shim 按目录解析，workspace 能跑不代表 venvs 目录能跑
        probe_python(python, &venvs_dir(app)?).await?;

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

    /// black 格式化脚本代码（`python -m black --quiet -`，stdin 进 stdout 出）。
    /// 轻量 spawn（同 probe_python 模式，不占 runner 单飞）；解释器链与 Run 刻意不对称：
    /// **默认 workspace `.venv` > 全局 python**（不读脚本绑定、不读「设为默认」命名 venv）。
    /// 全链路 tracing（`[scripts.format]` 前缀，设置页日志面板可 grep 诊断）。
    pub async fn format_code(
        app: &AppHandle,
        db: &Database,
        code: &str,
    ) -> Result<String, AppError> {
        let started = std::time::Instant::now();
        let settings = Self::get_settings(db)?;
        let ws = settings.default_workspace.trim();
        let venv_python = if !ws.is_empty() {
            venv_python_at(Path::new(ws)).await
        } else {
            None
        };
        let chain_source = if venv_python.is_some() {
            "workspace .venv"
        } else {
            "global python"
        };
        let interpreter =
            prepare::resolve_interpreter(venv_python.as_deref(), &settings.python_path);
        if interpreter.is_empty() {
            return Err(AppError::ValidationError {
                message: "python interpreter path is empty".into(),
            });
        }
        tracing::info!(
            "[scripts.format] interpreter={} (chain: {}) code_bytes={}",
            interpreter,
            chain_source,
            code.len()
        );

        // cwd 固定 venvs 目录：防 mise/pyenv shim 按进程目录解析（无 cwd 继承 app 目录）
        let spawn_dir = venvs_dir(app)?;
        let mut spawn_cmd = tokio::process::Command::new(&interpreter);
        spawn_cmd
            .args(["-m", "black", "--quiet", "-"])
            .current_dir(&spawn_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // 超时路径 drop 即杀——30s 兜底后不泄漏 python.exe
            .kill_on_drop(true);
        // GUI 父进程必加：抑制 console 分配（0xC0000142 根因 + 闪窗）
        crate::infrastructure::process::no_window(&mut spawn_cmd);
        let mut child = spawn_cmd
            .spawn()
            .map_err(|e| AppError::InternalError {
                message: format!("start python for black ({interpreter}): {e}"),
            })?;
        // 写 stdin 后 wait_with_output（30s 超时兜底——spawn + black import 冷启动余量）
        {
            use tokio::io::AsyncWriteExt;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(code.as_bytes()).await;
            } // stdin drop → EOF，black 输出结果
        }
        let waited = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output()).await;
        let out = match waited {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                tracing::warn!("[scripts.format] wait failed: {e}");
                return Err(AppError::InternalError {
                    message: format!("wait black: {e}"),
                });
            }
            Err(_) => {
                tracing::warn!("[scripts.format] timed out after 30s");
                return Err(AppError::ValidationError {
                    message: "black formatting timed out (30s)".into(),
                });
            }
        };

        if out.status.success() {
            let formatted = String::from_utf8_lossy(&out.stdout).into_owned();
            tracing::info!(
                "[scripts.format] ok: out_bytes={} changed={} elapsed={}ms",
                formatted.len(),
                formatted.len() != code.len(),
                started.elapsed().as_millis()
            );
            Ok(formatted)
        } else {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            let trimmed = stderr.trim();
            let kind = classify_black_failure(trimmed);
            // 末尾 200 字符（双 reverse 保序，避免直接 rev().take 倒序）
            let stderr_tail: String = trimmed
                .chars()
                .rev()
                .take(200)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            tracing::warn!(
                "[scripts.format] failed: exit={:?} kind={} stderr_tail={:?}",
                out.status.code(),
                kind,
                stderr_tail
            );
            Err(AppError::ValidationError {
                message: match kind {
                    "missing" => format!(
                        "black is not installed for {interpreter} — open a terminal on the \
                         default workspace venv (settings → venv row → Terminal) and run \
                         `pip install black`, or create that .venv first"
                    ),
                    "broken" => format!(
                        "black is broken or shadowed for {interpreter} — reinstall into that \
                         environment (terminal → `pip install --force-reinstall black`)"
                    ),
                    _ => format!(
                        "black failed for {interpreter} (exit {:?}): {}",
                        out.status.code(),
                        trimmed
                    ),
                },
            })
        }
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
        let venv_started = std::time::Instant::now();
        tracing::info!(
            "[scripts.venv] create start: dir={} python={}",
            dir.display(),
            settings.python_path.trim()
        );
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

        let result = match runner::spawn_and_stream(
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
        .await
        {
            Ok(outcome) if outcome.exit_code == Some(0) => {
                tracing::info!(
                    "[scripts.venv] create ok: dir={} elapsed={}ms",
                    dir.display(),
                    venv_started.elapsed().as_millis()
                );
                Ok(())
            }
            Ok(outcome) => {
                // exit 9009 = Windows「找不到命令」；空输出非零退出多为 shim（mise/pyenv）
                // 在当前目录解析失败——两种情况 python 都没真正跑起来
                const CMD_NOT_FOUND: i32 = 9009;
                let unusable = outcome.exit_code == Some(CMD_NOT_FOUND)
                    || (outcome.stderr.trim().is_empty() && outcome.stdout.trim().is_empty());
                let hint = if unusable {
                    " — python not found or unusable: set the global Python path to the py \
                     launcher or an absolute python.exe path (mise/pyenv shims may not work in \
                     some directories)"
                } else {
                    ""
                };
                tracing::warn!(
                    "[scripts.venv] create failed: dir={} exit={:?} unusable={} stderr_bytes={} stdout_bytes={} elapsed={}ms",
                    dir.display(),
                    outcome.exit_code,
                    unusable,
                    outcome.stderr.len(),
                    outcome.stdout.len(),
                    venv_started.elapsed().as_millis()
                );
                Err(AppError::InternalError {
                    message: format!(
                        "python -m venv failed (exit {:?}): {}{hint}",
                        outcome.exit_code,
                        outcome.stderr.trim()
                    ),
                })
            }
            Err(e) => {
                tracing::warn!(
                    "[scripts.venv] create error: dir={} err={} elapsed={}ms",
                    dir.display(),
                    e,
                    venv_started.elapsed().as_millis()
                );
                Err(e)
            }
        };

        if let Err(err) = &result {
            // 失败清理半成品目录：python -m venv 在残留目录上必然再失败（该名称将永久失败）
            tracing::warn!(
                "[scripts.venv] create cleanup: dir={} error={}",
                dir.display(),
                err
            );
            if let Err(clean_err) = tokio::fs::remove_dir_all(dir).await {
                // NotFound = 目录本就不存在（如 spawn 未成功）——无残留，静默
                if clean_err.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("[scripts.venv] create cleanup failed: {clean_err}");
                }
            }
        }
        result
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
                let venv_root = entry.path();
                if let Some(py) = python_exists(&venv_root).await {
                    let python_version = read_venv_version(&venv_root).await;
                    out.push(ScriptVenvSummary {
                        name,
                        python_path: py,
                        python_version,
                        workspace: false,
                    });
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));

        let settings = Self::get_settings(db)?;
        let ws = settings.default_workspace.trim();
        if !ws.is_empty() {
            let venv_root = Path::new(ws).join(".venv");
            if let Some(py) = python_exists(&venv_root).await {
                let python_version = read_venv_version(&venv_root).await;
                out.push(ScriptVenvSummary {
                    name: ".venv".into(),
                    python_path: py,
                    python_version,
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

    /// cwd + venv 候选链解析（run 与终端共用；纯解析不建目录——目录创建留在调用方，
    /// run 路径时序不变）。链：脚本绑定 > 全局启用 > 生效 cwd 下 .venv；
    /// 任一级指向的 venv 不存在则静默回落下级（解析容错）。
    /// `venv_effective` 记录实际提供解释器的 venv 引用（绑定名/全局启用名/".venv"），
    /// 供终端徽标展示；run 路径不消费。
    pub async fn resolve_run_target(
        app: &AppHandle,
        db: &Database,
        script: &ScriptDto,
    ) -> Result<ResolvedRunTarget, AppError> {
        let settings = Self::get_settings(db)?;
        let fallback_dir = scripts_run_dir(app)?.join("default-workspace");
        let cwd = prepare::resolve_cwd(
            script.workspace_path.as_deref(),
            &settings.default_workspace,
            fallback_dir,
        );
        let bound_venv = script
            .venv_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let global_venv = settings.venv.trim().to_string();
        let (venv_effective, venv_python) = match bound_venv.as_deref() {
            Some(".venv") => {
                let ws = settings.default_workspace.trim();
                if ws.is_empty() {
                    (None, None)
                } else {
                    match venv_python_at(Path::new(ws)).await {
                        Some(p) => (Some(".venv".to_string()), Some(p)),
                        None => (None, None),
                    }
                }
            }
            Some(name) => match named_venv_python(app, name).await {
                Some(p) => (Some(name.to_string()), Some(p)),
                None => (None, None),
            },
            None => {
                if !global_venv.is_empty() {
                    if let Some(p) = named_venv_python(app, &global_venv).await {
                        (Some(global_venv.clone()), Some(p))
                    } else if let Some(p) = venv_python_at(&cwd).await {
                        (Some(".venv".to_string()), Some(p))
                    } else {
                        (None, None)
                    }
                } else {
                    match venv_python_at(&cwd).await {
                        Some(p) => (Some(".venv".to_string()), Some(p)),
                        None => (None, None),
                    }
                }
            }
        };
        Ok(ResolvedRunTarget {
            cwd,
            bound_venv,
            venv_effective,
            venv_python,
        })
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
        let target = Self::resolve_run_target(app, db, &script).await?;
        let cwd = target.cwd.clone();
        tokio::fs::create_dir_all(&cwd)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("create workspace cwd: {e}"),
            })?;
        let venv_python = target.venv_python.clone();
        let interpreter =
            prepare::resolve_interpreter(venv_python.as_deref(), &settings.python_path);
        if interpreter.is_empty() {
            return Err(AppError::ValidationError {
                message: "python interpreter path is empty".into(),
            });
        }
        let run_chain = match (target.bound_venv.as_deref(), venv_python.as_deref()) {
            (Some(name), _) => format!("script venv \"{name}\""),
            (None, Some(_)) => "global venv / workspace .venv".to_string(),
            (None, None) => "global python".to_string(),
        };
        let run_started = std::time::Instant::now();
        tracing::info!(
            "[scripts.run] script=\"{}\" interpreter={} (chain: {})",
            script.name,
            interpreter,
            run_chain
        );

        let run_dir = scripts_run_dir(app)?;
        tokio::fs::create_dir_all(&run_dir)
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("create scripts run dir: {e}"),
            })?;
        cleanup_orphan_run_scripts(&run_dir).await;
        // 参数模块：schema → dataclass + 本次运行实例（脚本同目录，sys.path[0] 直达）
        let params_module_src = prepare::render_params_module(&script.params_schema, &effective);
        let guard = TempScriptGuard::create(&run_dir, &script.body, &params_module_src).await?;

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

        tracing::info!("[scripts.run] command: {command}");

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

        tracing::info!(
            "[scripts.run] finished: script=\"{}\" exit={:?} cancelled={} stdout_bytes={} stderr_bytes={} elapsed={}ms",
            script.name,
            outcome.exit_code,
            outcome.cancelled,
            outcome.stdout.len(),
            outcome.stderr.len(),
            run_started.elapsed().as_millis()
        );
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

/// 规范化 venv 引用：`".venv"` 保留；命名走白名单校验。
fn normalize_venv_ref(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed == ".venv" {
        Ok(".venv".into())
    } else {
        validate::validate_venv_name(trimmed)
    }
}

/// 解析 venv 根目录：`".venv"` = 默认 workspace 下；命名 = `{app_data}/venvs/<name>/`。
/// 不存在或 workspace 未配置 → ValidationError。
async fn require_venv_root(app: &AppHandle, db: &Database, name: &str) -> Result<PathBuf, AppError> {
    if name == ".venv" {
        let settings = ScriptsService::get_settings(db)?;
        let ws = settings.default_workspace.trim();
        if ws.is_empty() {
            return Err(AppError::ValidationError {
                message: "scripts.default_workspace is not configured".into(),
            });
        }
        let dir = Path::new(ws).join(".venv");
        if python_exists(&dir).await.is_none() {
            return Err(AppError::ValidationError {
                message: "venv not found: .venv (default workspace)".into(),
            });
        }
        return Ok(dir);
    }
    let root = venvs_dir(app)?;
    let dir = root.join(name);
    if python_exists(&dir).await.is_none() {
        return Err(AppError::ValidationError {
            message: format!("venv not found: {name}"),
        });
    }
    Ok(dir)
}

/// black 失败分类：missing（未装）/ broken（损坏或同名包遮蔽）/ other。
fn classify_black_failure(stderr: &str) -> &'static str {
    let broken = stderr.contains("black.__main__")
        || stderr.contains("is a package and cannot be directly executed");
    if broken {
        return "broken";
    }
    // 真未装：带引号，或裸 `No module named black` 且非 __main__ 变体
    let missing = stderr.contains("No module named 'black'")
        || (stderr.contains("No module named black") && !stderr.contains("black.__main__"));
    if missing {
        "missing"
    } else {
        "other"
    }
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

/// 轻量探测 python 可用性（独立 spawn，不占运行单飞；继承进程 env）。
/// cwd 用调用方指定的真实场景目录（如 venvs 根——mise/pyenv shim 按目录解析）。
async fn probe_python(python: &str, probe_cwd: &Path) -> Result<(), AppError> {
    let probe = async {
        let mut cmd = tokio::process::Command::new(python);
        cmd.arg("--version")
            .current_dir(probe_cwd)
            .stdin(Stdio::null());
        // GUI 父进程必加：抑制 console 分配（0xC0000142 根因 + 闪窗）
        crate::infrastructure::process::no_window(&mut cmd);
        cmd.output().await
    };
    let output = match tokio::time::timeout(Duration::from_secs(10), probe).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return Err(AppError::ValidationError {
                message: format!(
                    "scripts.python_path {python:?} failed to start: {e} — use the py \
                     launcher or an absolute python.exe path"
                ),
            });
        }
        Err(_) => {
            return Err(AppError::ValidationError {
                message: format!("scripts.python_path {python:?} timed out on --version (10s)"),
            });
        }
    };
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    });
    Err(AppError::ValidationError {
        message: format!(
            "scripts.python_path {python:?} is not usable (exit {}): {} — use the py \
             launcher or an absolute python.exe path (mise/pyenv shims resolve per-directory \
             and may fail outside configured dirs)",
            output.status.code().unwrap_or(-1),
            detail.trim()
        ),
    })
}

/// 读 venv 根目录 `pyvenv.cfg` 的 `version =`（venv 标准产物；读不到为 None）。
async fn read_venv_version(venv_root: &Path) -> Option<String> {
    let cfg = tokio::fs::read_to_string(venv_root.join("pyvenv.cfg"))
        .await
        .ok()?;
    parse_pyvenv_version(&cfg)
}

/// 解析 `pyvenv.cfg` 内容中的版本行（容忍空格变体：`version = x` / `version=x`）。
fn parse_pyvenv_version(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("version")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('='))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    })
}

/// 下次运行前清理孤儿临时脚本（app 崩溃/断电残留）。
async fn cleanup_orphan_run_scripts(run_dir: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(run_dir).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let is_orphan = (name.starts_with("run-") && name.ends_with(".py"))
            || name == prepare::PARAMS_MODULE_FILE;
        if is_orphan {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn probe_python_rejects_missing_program() {
        let err = probe_python(
            "definitely-not-a-real-python-xyz",
            std::env::temp_dir().as_path(),
        )
        .await
        .expect_err("missing program must fail");
        assert!(
            matches!(err, AppError::ValidationError { .. }),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn probe_python_accepts_version_flag_program() {
        // robocopy 带未知参数 exit 16——验证非零退出被拒
        if !cfg!(windows) {
            return;
        }
        let err = probe_python("robocopy", std::env::temp_dir().as_path())
            .await
            .expect_err("robocopy --version exits non-zero");
        let msg = format!("{err:?}");
        assert!(msg.contains("not usable"), "got: {msg}");
    }

    #[test]
    fn parse_pyvenv_version_variants() {
        let cfg = "home = C:\\Python311\ninclude-system-site-packages = false\nversion = 3.11.15\n";
        assert_eq!(parse_pyvenv_version(cfg).as_deref(), Some("3.11.15"));
        assert_eq!(
            parse_pyvenv_version("version=3.9\n").as_deref(),
            Some("3.9")
        );
        assert_eq!(parse_pyvenv_version("home = x\n").as_deref(), None);
        assert_eq!(parse_pyvenv_version("").as_deref(), None);
        // 实机 workspace .venv 的 cfg 顺带验证
    }

    #[test]
    fn classify_black_failure_kinds() {
        assert_eq!(
            classify_black_failure("ModuleNotFoundError: No module named 'black'"),
            "missing"
        );
        assert_eq!(
            classify_black_failure(
                "No module named black.__main__; 'black' is a package and cannot be directly executed"
            ),
            "broken"
        );
        assert_eq!(
            classify_black_failure("SyntaxError: invalid syntax"),
            "other"
        );
    }

    #[tokio::test]
    async fn read_venv_version_from_real_workspace_venv() {
        let dir = Path::new(r"D:\workspace\environment\default_wp\.venv");
        if !dir.join("pyvenv.cfg").exists() {
            return; // 目录不在时跳过（非用户机器）
        }
        let v = read_venv_version(dir).await;
        assert!(v.is_some(), "workspace .venv pyvenv.cfg should parse");
    }
}
