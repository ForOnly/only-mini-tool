//! 脚本运行准备（纯函数，可测）：有效参数、三通道投影、env 合并、cwd/解释器解析、argv 拼装。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::domain::{ScriptArgsTemplate, ScriptParamDef, ScriptParamPassAs, ScriptParamType};

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
) -> ProjectedParams {
    let mut out = ProjectedParams::default();
    for def in schema {
        let raw = params.get(&def.key).cloned().unwrap_or_default();
        match def.pass_as {
            ScriptParamPassAs::Env => {
                let env_key = format!("PARAM_{}", def.key.to_uppercase());
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

/// 解释器解析：脚本覆盖 > 全局。
pub fn resolve_interpreter(script_interpreter: Option<&str>, global_python: &str) -> String {
    script_interpreter
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
    fn env_projection_boolean_encoding() {
        let schema = vec![
            def("flag", ScriptParamType::Boolean, ScriptParamPassAs::Env),
            def("name", ScriptParamType::String, ScriptParamPassAs::Env),
        ];
        let params = HashMap::from([
            ("flag".to_string(), "true".to_string()),
            ("name".to_string(), "hi".to_string()),
        ]);
        let out = project_params(&schema, &params);
        assert_eq!(out.envs[0], ("PARAM_FLAG".to_string(), "1".to_string()));
        assert_eq!(out.envs[1], ("PARAM_NAME".to_string(), "hi".to_string()));
        assert!(out.args.is_empty());
        assert!(out.stdin.is_empty());

        let params = HashMap::from([("flag".to_string(), "false".to_string())]);
        let out = project_params(&schema, &params);
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
        let out = project_params(&schema, &params);
        assert_eq!(out.args, vec!["--must", "v", "--flag"]);

        // required 空值仍传键（后端校验会拦，投影保持显式）
        let params = HashMap::from([("must".to_string(), String::new())]);
        let out = project_params(&schema, &params);
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
        let out = project_params(&schema, &params);

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
        assert_eq!(
            resolve_interpreter(Some(" C:\\py\\x.exe "), "python"),
            "C:\\py\\x.exe"
        );
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
}
