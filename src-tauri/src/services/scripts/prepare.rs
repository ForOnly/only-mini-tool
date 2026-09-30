//! 脚本运行准备（纯函数，可测）：有效参数、三通道投影、env 合并、cwd/解释器解析、argv 拼装。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use uuid::Uuid;

use crate::domain::{ScriptArgsTemplate, ScriptParamDef, ScriptParamPassAs, ScriptParamType};
use crate::errors::AppError;

use super::validate::is_truthy;

/// 参数有效值：key 缺失或空串时回退 default（与前端 `applyDefaultsFromSchema` 语义一致）。
pub fn effective_params(
    schema: &[ScriptParamDef],
    params: &HashMap<String, String>,
) -> HashMap<String, String> {
    schema
        .iter()
        .map(|def| {
            let value = params
                .get(&def.key)
                .map(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    def.default_value
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or("")
                .to_string();
            (def.key.clone(), value)
        })
        .collect()
}

/// 参数投影结果：env 注入项（`PARAM_*`）、argv 动态项（`--key value`）、stdin JSON 文档。
///
/// `passAs=stdin` 的参数只进 JSON 文档，不进 env/argv。
#[derive(Debug, Default)]
pub struct ProjectedParams {
    pub envs: Vec<(String, String)>,
    pub args: Vec<String>,
    pub stdin: Map<String, Value>,
}

pub fn project_params(
    schema: &[ScriptParamDef],
    params: &HashMap<String, String>,
    env_prefix: &str,
) -> ProjectedParams {
    let mut out = ProjectedParams::default();
    for def in schema {
        let raw = params.get(&def.key).cloned().unwrap_or_default();
        match def.pass_as {
            ScriptParamPassAs::Env => {
                let env_key = format!("{prefix}{}", def.key.to_uppercase(), prefix = env_prefix);
                let value = if matches!(def.param_type, ScriptParamType::Boolean) {
                    if is_truthy(&raw) {
                        "1".into()
                    } else {
                        "0".into()
                    }
                } else {
                    raw
                };
                out.envs.push((env_key, value));
            }
            ScriptParamPassAs::Arg => match def.param_type {
                ScriptParamType::Boolean => {
                    if is_truthy(&raw) {
                        out.args.push(format!("--{}", def.key));
                    }
                }
                _ => {
                    if !raw.is_empty() || def.required {
                        out.args.push(format!("--{}", def.key));
                        out.args.push(raw);
                    }
                }
            },
            ScriptParamPassAs::Stdin => {
                // boolean 有天然零值，始终写入；其余空值省略（required 已由校验保证非空）
                let include = match def.param_type {
                    ScriptParamType::Boolean => true,
                    _ => !raw.is_empty(),
                };
                if include {
                    out.stdin.insert(def.key.clone(), stdin_value(def, &raw));
                }
            }
        }
    }
    out
}

/// stdin JSON 值投影。number 整数优先（`"10"`→`10` 而非 `10.0`，
/// 避免 Python 侧 `range(10.0)` 报错），小数回退 f64。
fn stdin_value(def: &ScriptParamDef, raw: &str) -> Value {
    match def.param_type {
        ScriptParamType::Boolean => Value::Bool(is_truthy(raw)),
        ScriptParamType::Number => {
            let trimmed = raw.trim();
            if let Ok(i) = trimmed.parse::<i64>() {
                Value::from(i)
            } else if let Ok(f) = trimmed.parse::<f64>() {
                Value::from(f)
            } else {
                // 校验后不可达；防御性回退保留原始值
                Value::String(raw.to_string())
            }
        }
        _ => Value::String(raw.to_string()),
    }
}

/// env 合并：process < 全局 `scripts.env_json` < 脚本 env < `PARAM_*` 投影（最高）。
pub fn merge_env(
    process_env: HashMap<String, String>,
    global_env: &HashMap<String, String>,
    script_env: &HashMap<String, String>,
    param_envs: &[(String, String)],
) -> HashMap<String, String> {
    let mut env = process_env;
    for (k, v) in global_env {
        env.insert(k.clone(), v.clone());
    }
    for (k, v) in script_env {
        env.insert(k.clone(), v.clone());
    }
    for (k, v) in param_envs {
        env.insert(k.clone(), v.clone());
    }
    env
}

/// 解释器解析：**venv python**（脚本绑定/全局启用/workspace `.venv` 中先命中者）
/// 优先于全局 `scripts.python_path`。venv 候选由调用方按优先级探测（保持纯函数）；
/// 返回可能为空串（由调用方校验报错）。
pub fn resolve_interpreter(venv_python: Option<&str>, global_python: &str) -> String {
    venv_python
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(global_python.trim())
        .to_string()
}

/// cwd 解析：脚本 workspace > 全局默认 > fallback 目录。
pub fn resolve_cwd(
    script_workspace: Option<&str>,
    default_workspace: &str,
    fallback_dir: PathBuf,
) -> PathBuf {
    if let Some(p) = script_workspace.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(p);
    }
    let def = default_workspace.trim();
    if !def.is_empty() {
        return PathBuf::from(def);
    }
    fallback_dir
}

/// argv 拼装：`args_template.before` → 脚本路径 → 动态参数 → `args_template.after`。
pub fn build_args(
    args_template: &ScriptArgsTemplate,
    script_path: &Path,
    dyn_args: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    args.extend(args_template.before.iter().cloned());
    args.push(script_path.to_string_lossy().into_owned());
    args.extend_from_slice(dyn_args);
    args.extend(args_template.after.iter().cloned());
    args
}

/// 等效命令回显：解释器 + before + **脚本名**（非随机临时路径，保可读）+ 动态参数 + after。
/// 含空白/空串的片段加引号；仅供展示，不保证可直接粘贴执行。
pub fn display_command(
    interpreter: &str,
    script_name: &str,
    args_template: &ScriptArgsTemplate,
    dyn_args: &[String],
) -> String {
    let mut parts: Vec<String> = vec![quote_if_needed(interpreter)];
    parts.extend(args_template.before.iter().map(|s| quote_if_needed(s)));
    parts.push(quote_if_needed(&format!("{script_name}.py")));
    parts.extend(dyn_args.iter().map(|s| quote_if_needed(s)));
    parts.extend(args_template.after.iter().map(|s| quote_if_needed(s)));
    parts.join(" ")
}

fn quote_if_needed(s: &str) -> String {
    if s.is_empty() {
        "\"\"".to_string()
    } else if s.contains(char::is_whitespace) {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

/// env 未设 `PYTHONIOENCODING` 时注入 utf-8：使 Python 子进程管道 stdio 与工具统一
/// UTF-8（stdin JSON 通道与 stdout 解码一致；对齐 Python 3.15 起的默认行为）。
/// 用户在进程/全局/脚本任一层显式设置则尊重原值；大小写不敏感检查避免
/// Windows env block 产生仅大小写不同的重复键。
pub fn ensure_stdio_utf8(env: &mut HashMap<String, String>) {
    let unset = !env
        .keys()
        .any(|k| k.eq_ignore_ascii_case("PYTHONIOENCODING"));
    if unset {
        env.insert("PYTHONIOENCODING".to_string(), "utf-8".to_string());
    }
}

/// 生成参数模块文件名（固定名：应用专属前缀防用户脚本模块冲突；单飞保证无并发碰撞）。
pub const PARAMS_MODULE_FILE: &str = "onlytool_params.py";
/// 参数模块导入名（去 .py）。
pub const PARAMS_MODULE_NAME: &str = "onlytool_params";

/// Python 关键字（codegen 字段名 + schema 保存校验共用）。
pub fn is_python_keyword(word: &str) -> bool {
    matches!(
        word,
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

/// Python 字符串字面量转义：`\ " \n \r \t` 与控制字符（\xNN）。
fn py_escape(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for c in raw.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 参数值的 Python 表达式（按 schema 类型）：
/// string/select/path → 字符串字面量（path 包 `Path(...)`）；
/// number → 整数优先 `int`，否则 `float`，NaN/Inf 特判 `float("nan")` 防 NameError；
/// boolean → True/False。
fn py_value_expr(def: &ScriptParamDef, raw: &str) -> String {
    match def.param_type {
        ScriptParamType::Boolean => {
            if is_truthy(raw) {
                "True".into()
            } else {
                "False".into()
            }
        }
        ScriptParamType::Number => {
            let trimmed = raw.trim();
            if trimmed.parse::<i64>().is_ok() {
                trimmed.to_string()
            } else if let Ok(f) = trimmed.parse::<f64>() {
                if f.is_nan() {
                    "float(\"nan\")".to_string()
                } else if f.is_infinite() {
                    if f > 0.0 {
                        "float(\"inf\")".to_string()
                    } else {
                        "float(\"-inf\")".to_string()
                    }
                } else {
                    format!("{f}")
                }
            } else {
                // 校验后不可达；防御性回退字符串
                py_escape(raw)
            }
        }
        ScriptParamType::Path => format!("Path({})", py_escape(raw)),
        _ => py_escape(raw),
    }
}

/// dataclass 字段的 Python 类型标注表达式。
fn py_type_expr(def: &ScriptParamDef) -> String {
    match def.param_type {
        ScriptParamType::String | ScriptParamType::Path => {
            if matches!(def.param_type, ScriptParamType::Path) {
                "Path".to_string()
            } else {
                "str".to_string()
            }
        }
        ScriptParamType::Number => "Union[int, float]".to_string(),
        ScriptParamType::Boolean => "bool".to_string(),
        ScriptParamType::Select => {
            let opts: Vec<String> = def.options.iter().map(|o| py_escape(o)).collect();
            format!("Literal[{}]", opts.join(", "))
        }
    }
}

/// 生成参数模块源码：schema → frozen dataclass `Params` + 预填实例 `params`。
/// 脚本同目录运行时 Python 自动加入 `sys.path[0]`，`from onlytool_params import params` 即用。
pub fn render_params_module(
    schema: &[ScriptParamDef],
    effective: &HashMap<String, String>,
) -> String {
    // required 无默认在前、optional 带默认在后（dataclass 字段序约束）
    let mut ordered: Vec<&ScriptParamDef> = schema.iter().collect();
    ordered.sort_by_key(|d| d.required == false);

    let mut needs_literal = false;
    let mut class_fields = String::new();
    let mut ctor_args = String::new();

    for def in ordered {
        // Python 关键字 key 追加 `_`（保存校验会拒绝新关键字 key，存量防御）
        let field_name = if is_python_keyword(&def.key) {
            format!("{}_", def.key)
        } else {
            def.key.clone()
        };
        if matches!(def.param_type, ScriptParamType::Select) {
            needs_literal = true;
        }
        let value = effective.get(&def.key).cloned().unwrap_or_default();
        let value_expr = py_value_expr(def, &value);

        if def.required {
            class_fields.push_str(&format!("    {}: {}\n", field_name, py_type_expr(def)));
        } else {
            class_fields.push_str(&format!(
                "    {}: {} = {}\n",
                field_name,
                py_type_expr(def),
                py_value_expr(def, def.default_value.as_deref().unwrap_or(""))
            ));
        }
        ctor_args.push_str(&format!("    {}={},\n", field_name, value_expr));
    }

    let typing_import = if needs_literal {
        "from typing import Literal, Union"
    } else {
        "from typing import Union"
    };
    // 空 schema 也生成合法模块（空类体需 pass）
    let class_body = if class_fields.is_empty() {
        "    pass".to_string()
    } else {
        class_fields
    };

    format!(
        r#"# 由 only-mini-tool 运行时生成，勿手改（参数 schema → dataclass + 本次运行实例）。
# 用法：from onlytool_params import params
# 注意：args_template.before 含 -c/-m/-I 等改变 sys.path[0] 语义的参数时本模块不可用。
from dataclasses import dataclass
{typing_import}
from pathlib import Path


@dataclass(frozen=True)
class Params:
{class_body}

params = Params(
{ctor_args})
"#
    )
}

/// 临时脚本守卫：写 `run-{uuid}.py` + `onlytool_params.py` 至 run_dir，Drop 时一并删除。
/// 正常路径随 guard drop 删除；app 崩溃残留由下次运行前的孤儿清理兜底。
pub struct TempScriptGuard {
    path: PathBuf,
    params_module_path: PathBuf,
}

impl TempScriptGuard {
    pub async fn create(
        run_dir: &Path,
        body: &str,
        params_module_src: &str,
    ) -> Result<Self, AppError> {
        let path = run_dir.join(format!("run-{}.py", Uuid::new_v4()));
        tokio::fs::write(&path, body.as_bytes())
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("write temp script: {e}"),
            })?;
        let params_module_path = run_dir.join(PARAMS_MODULE_FILE);
        tokio::fs::write(&params_module_path, params_module_src.as_bytes())
            .await
            .map_err(|e| AppError::InternalError {
                message: format!("write params module: {e}"),
            })?;
        Ok(TempScriptGuard {
            path,
            params_module_path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempScriptGuard {
    fn drop(&mut self) {
        // Drop 不能 await；同步删除，失败无害（孤儿清理兜底）
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.params_module_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(key: &str, param_type: ScriptParamType, pass_as: ScriptParamPassAs) -> ScriptParamDef {
        ScriptParamDef {
            key: key.into(),
            label: key.into(),
            param_type,
            required: false,
            default_value: None,
            options: Vec::new(),
            path_mode: None,
            pass_as,
        }
    }

    #[test]
    fn effective_params_falls_back_to_default() {
        let mut d = def("n", ScriptParamType::Number, ScriptParamPassAs::Env);
        d.default_value = Some("42".into());
        let schema = vec![d, def("s", ScriptParamType::String, ScriptParamPassAs::Env)];

        let out = effective_params(
            &schema,
            &HashMap::from([("s".to_string(), "v".to_string())]),
        );
        assert_eq!(out.get("n").map(String::as_str), Some("42"));
        assert_eq!(out.get("s").map(String::as_str), Some("v"));

        // 显式空串也回退 default（与前端语义一致）
        let out = effective_params(
            &schema,
            &HashMap::from([
                ("n".to_string(), String::new()),
                ("s".to_string(), String::new()),
            ]),
        );
        assert_eq!(out.get("n").map(String::as_str), Some("42"));
        assert_eq!(out.get("s").map(String::as_str), Some(""));

        // 无 default 无值 → 空串
        let schema = vec![def("x", ScriptParamType::String, ScriptParamPassAs::Env)];
        let out = effective_params(&schema, &HashMap::new());
        assert_eq!(out.get("x").map(String::as_str), Some(""));
    }

    #[test]
    fn env_projection_uses_custom_prefix() {
        let schema = vec![def("name", ScriptParamType::String, ScriptParamPassAs::Env)];
        let params = HashMap::from([("name".to_string(), "hi".to_string())]);
        let out = project_params(&schema, &params, "CFG_");
        assert_eq!(out.envs[0], ("CFG_NAME".to_string(), "hi".to_string()));
    }

    #[test]
    fn env_projection_boolean_encoding() {
        let schema = vec![
            def("flag", ScriptParamType::Boolean, ScriptParamPassAs::Env),
            def("name", ScriptParamType::String, ScriptParamPassAs::Env),
        ];
        let params = HashMap::from([
            ("flag".to_string(), "true".to_string()),
            ("name".to_string(), "hi".to_string()),
        ]);
        let out = project_params(&schema, &params, "PARAM_");
        assert_eq!(out.envs[0], ("PARAM_FLAG".to_string(), "1".to_string()));
        assert_eq!(out.envs[1], ("PARAM_NAME".to_string(), "hi".to_string()));
        assert!(out.args.is_empty());
        assert!(out.stdin.is_empty());

        let params = HashMap::from([("flag".to_string(), "false".to_string())]);
        let out = project_params(&schema, &params, "PARAM_");
        assert_eq!(out.envs[0], ("PARAM_FLAG".to_string(), "0".to_string()));
    }

    #[test]
    fn arg_projection_rules() {
        let mut req = def("must", ScriptParamType::String, ScriptParamPassAs::Arg);
        req.required = true;
        let schema = vec![
            req,
            def("flag", ScriptParamType::Boolean, ScriptParamPassAs::Arg),
            def("opt", ScriptParamType::String, ScriptParamPassAs::Arg),
        ];
        let params = HashMap::from([
            ("must".to_string(), "v".to_string()),
            ("flag".to_string(), "true".to_string()),
            ("opt".to_string(), String::new()),
        ]);
        let out = project_params(&schema, &params, "PARAM_");
        assert_eq!(out.args, vec!["--must", "v", "--flag"]);

        // required 空值仍传键（后端校验会拦，投影保持显式）
        let params = HashMap::from([("must".to_string(), String::new())]);
        let out = project_params(&schema, &params, "PARAM_");
        assert_eq!(out.args, vec!["--must", ""]);
        assert!(out.envs.is_empty());
    }

    #[test]
    fn stdin_projection_integer_first_and_exclusion() {
        let schema = vec![
            def("n", ScriptParamType::Number, ScriptParamPassAs::Stdin),
            def("f", ScriptParamType::Number, ScriptParamPassAs::Stdin),
            def("flag", ScriptParamType::Boolean, ScriptParamPassAs::Stdin),
            def("s", ScriptParamType::String, ScriptParamPassAs::Stdin),
        ];
        let params = HashMap::from([
            ("n".to_string(), "10".to_string()),
            ("f".to_string(), "2.5".to_string()),
            ("flag".to_string(), "false".to_string()),
        ]);
        let out = project_params(&schema, &params, "PARAM_");

        // 整数优先：10 而非 10.0
        assert_eq!(out.stdin.get("n"), Some(&serde_json::json!(10)));
        assert!(out.stdin.get("n").unwrap().is_i64());
        assert_eq!(out.stdin.get("f"), Some(&serde_json::json!(2.5)));
        assert_eq!(out.stdin.get("flag"), Some(&serde_json::json!(false)));
        // 空串省略
        assert!(!out.stdin.contains_key("s"));
        // stdin 参数不进 env/argv
        assert!(out.envs.is_empty());
        assert!(out.args.is_empty());
    }

    #[test]
    fn env_merge_priority() {
        let process = HashMap::from([
            ("A".to_string(), "p".to_string()),
            ("B".to_string(), "p".to_string()),
            ("C".to_string(), "p".to_string()),
        ]);
        let global = HashMap::from([("B".to_string(), "g".to_string())]);
        let script = HashMap::from([("C".to_string(), "s".to_string())]);
        let params = vec![("PARAM_X".to_string(), "1".to_string())];
        let env = merge_env(process, &global, &script, &params);
        assert_eq!(env["A"], "p");
        assert_eq!(env["B"], "g");
        assert_eq!(env["C"], "s");
        assert_eq!(env["PARAM_X"], "1");
    }

    #[test]
    fn interpreter_and_cwd_chains() {
        const VENV: &str = "W:\\ws\\.venv\\Scripts\\python.exe";
        // venv 优先于全局
        assert_eq!(resolve_interpreter(Some(VENV), "python"), VENV);
        // 空白/缺失 venv → 全局
        assert_eq!(resolve_interpreter(Some("  "), " python "), "python");
        assert_eq!(resolve_interpreter(None, " python "), "python");

        assert_eq!(
            resolve_cwd(Some(" W "), "g", PathBuf::from("f")),
            PathBuf::from("W")
        );
        assert_eq!(
            resolve_cwd(None, " g ", PathBuf::from("f")),
            PathBuf::from("g")
        );
        assert_eq!(
            resolve_cwd(None, "", PathBuf::from("f")),
            PathBuf::from("f")
        );
    }

    #[test]
    fn args_order() {
        let template = ScriptArgsTemplate {
            before: vec!["-u".into()],
            after: vec!["--tail".into()],
        };
        let args = build_args(
            &template,
            Path::new("C:/tmp/run-x.py"),
            &["--k".to_string(), "v".to_string()],
        );
        assert_eq!(args, vec!["-u", "C:/tmp/run-x.py", "--k", "v", "--tail"]);
    }

    #[test]
    fn ensure_stdio_utf8_respects_existing_any_case() {
        let mut env = HashMap::new();
        ensure_stdio_utf8(&mut env);
        assert_eq!(env["PYTHONIOENCODING"], "utf-8");

        // 用户显式设置（任意层）不被覆盖
        let mut env = HashMap::from([("PYTHONIOENCODING".to_string(), "gbk".to_string())]);
        ensure_stdio_utf8(&mut env);
        assert_eq!(env["PYTHONIOENCODING"], "gbk");

        // 大小写变体也视为已设置（避免 Windows env block 重复键）
        let mut env = HashMap::from([("pythonioencoding".to_string(), "utf-8".to_string())]);
        ensure_stdio_utf8(&mut env);
        assert_eq!(env.len(), 1);
    }

    #[test]
    fn display_command_quotes_whitespace_and_uses_script_name() {
        let template = ScriptArgsTemplate {
            before: vec!["-u".into()],
            after: vec![],
        };
        let cmd = display_command(
            "python",
            "my script",
            &template,
            &["--k".to_string(), "a b".to_string(), String::new()],
        );
        assert_eq!(cmd, r#"python -u "my script.py" --k "a b" """#);
    }

    #[tokio::test]
    async fn temp_script_guard_creates_and_removes() {
        let dir = std::env::temp_dir().join(format!("omt-guard-test-{}", Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let (path, module_path) = {
            let guard = TempScriptGuard::create(&dir, "print(1)", "# params")
                .await
                .unwrap();
            let p = guard.path().to_path_buf();
            let m = dir.join(PARAMS_MODULE_FILE);
            assert!(p.exists());
            assert!(m.exists(), "params module must be written alongside");
            (p, m)
        };
        assert!(!path.exists(), "guard drop must remove temp script");
        assert!(
            !module_path.exists(),
            "guard drop must remove params module"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_params_module_full_shape() {
        let mut name = def("name", ScriptParamType::String, ScriptParamPassAs::Env);
        name.required = true;
        let mut limit = def("limit", ScriptParamType::Number, ScriptParamPassAs::Stdin);
        limit.default_value = Some("10".into());
        let mut mode = def("mode", ScriptParamType::Select, ScriptParamPassAs::Env);
        mode.default_value = Some("a".into());
        mode.options = vec!["a".into(), "b".into()];
        let mut debug = def("debug", ScriptParamType::Boolean, ScriptParamPassAs::Env);
        debug.default_value = Some("false".into());
        let mut input = def("input", ScriptParamType::Path, ScriptParamPassAs::Arg);
        input.default_value = Some("".into());

        let effective = HashMap::from([
            ("name".to_string(), "hi \"q\"\\\n".to_string()),
            ("limit".to_string(), "42".to_string()),
            ("mode".to_string(), "b".to_string()),
            ("debug".to_string(), "true".to_string()),
            ("input".to_string(), "D:/ws/a.txt".to_string()),
        ]);
        let src = render_params_module(&[name, limit, mode, debug, input], &effective);

        // required 无默认在前
        let name_pos = src.find("    name: str").unwrap();
        let limit_pos = src.find("    limit: Union[int, float] = 10").unwrap();
        assert!(name_pos < limit_pos, "required field must come first");
        // 类型映射与转义
        assert!(src.contains("    mode: Literal[\"a\", \"b\"] = \"a\""));
        assert!(src.contains("    debug: bool = False"));
        assert!(src.contains("    input: Path = Path(\"\")"));
        // 实例值：转义（\" \\ \n）、int、Literal 值、True、Path 包裹
        assert!(src.contains("    name=\"hi \\\"q\\\"\\\\\\n\","));
        assert!(src.contains("    limit=42,"));
        assert!(src.contains("    mode=\"b\","));
        assert!(src.contains("    debug=True,"));
        assert!(src.contains("    input=Path(\"D:/ws/a.txt\"),"));
        // 头部契约与 frozen
        assert!(src.contains("from onlytool_params import params"));
        assert!(src.contains("@dataclass(frozen=True)"));
        assert!(src.contains("from typing import Literal, Union"));
    }

    #[test]
    fn render_params_module_edge_cases() {
        // 空 schema：空 dataclass（pass）+ 空实例，import 不炸
        let src = render_params_module(&[], &HashMap::new());
        assert!(src.contains("class Params:\n    pass\n"));
        assert!(src.contains("params = Params("));
        assert!(src.contains("from typing import Union"));

        // number NaN/Inf 特判
        let mut n = def("n", ScriptParamType::Number, ScriptParamPassAs::Env);
        n.default_value = Some("0".into());
        let src = render_params_module(
            &[n.clone()],
            &HashMap::from([("n".to_string(), "nan".to_string())]),
        );
        assert!(src.contains("    n=float(\"nan\"),"));
        let src = render_params_module(
            &[n.clone()],
            &HashMap::from([("n".to_string(), "-inf".to_string())]),
        );
        assert!(src.contains("    n=float(\"-inf\"),"));
        // 小数
        let src =
            render_params_module(&[n], &HashMap::from([("n".to_string(), "2.5".to_string())]));
        assert!(src.contains("    n=2.5,"));

        // Python 关键字 key（存量防御）：字段名追加 _
        let kw = def("class", ScriptParamType::String, ScriptParamPassAs::Env);
        let src = render_params_module(
            &[kw],
            &HashMap::from([("class".to_string(), "x".to_string())]),
        );
        assert!(src.contains("    class_: str"));
        assert!(src.contains("    class_=\"x\","));
    }
}
