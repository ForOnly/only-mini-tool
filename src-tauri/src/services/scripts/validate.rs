//! 脚本库校验（service 层纯函数）。

use std::collections::{HashMap, HashSet};

use crate::domain::{ScriptParamDef, ScriptParamType};
use crate::errors::AppError;

/// param key 长度上限。
const PARAM_KEY_MAX_LEN: usize = 64;

/// 合法 param key：字母/下划线开头，仅 ASCII 字母数字下划线，长度 ≤64。
/// 同时满足 env 注入（`PARAM_{大写}`）与 argv 选项（`--key`）的安全。
fn is_valid_param_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    key.len() <= PARAM_KEY_MAX_LEN && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// name 归一化 + 非空校验，返回 trim 后的值。
pub fn validate_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::ValidationError {
            message: "script name is required".into(),
        });
    }
    Ok(trimmed.to_string())
}

/// env 参数前缀校验：非空、`^[A-Za-z_][A-Za-z0-9_]{0,31}$`（env 名安全；
/// 禁止空前缀——裸 KEY 大写后会与真实 env 碰撞，如 `path` → `PATH`）。
pub fn validate_env_prefix(prefix: &str) -> Result<String, AppError> {
    let trimmed = prefix.trim();
    let valid = {
        let mut chars = trimmed.chars();
        matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
            && trimmed.len() <= 32
            && trimmed
                .chars()
                .skip(1)
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if !valid {
        return Err(AppError::ValidationError {
            message: "scripts.env_prefix must be ASCII letters/digits/underscore, start with \
                      letter or underscore, 1-32 chars"
                .into(),
        });
    }
    Ok(trimmed.to_string())
}

/// 命名 venv 名称校验：`^[A-Za-z0-9_-]{1,64}$`（白名单即防路径穿越）。
pub fn validate_venv_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    let valid = !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !valid {
        return Err(AppError::ValidationError {
            message: "venv name must be ASCII letters/digits/underscore/hyphen, 1-64 chars".into(),
        });
    }
    Ok(trimmed.to_string())
}

/// 保存时校验参数 schema：key 字符集/去重、select 必须有 options、default 与类型匹配。
pub fn validate_params_schema(schema: &[ScriptParamDef]) -> Result<(), AppError> {
    let mut keys = HashSet::new();
    for def in schema {
        let key = def.key.trim();
        if key.is_empty() {
            return Err(AppError::ValidationError {
                message: "param key is required".into(),
            });
        }
        if !is_valid_param_key(key) {
            return Err(AppError::ValidationError {
                message: format!(
                    "param key \"{key}\" must be ASCII letters/digits/underscore, start with \
                     letter or underscore, max {PARAM_KEY_MAX_LEN} chars"
                ),
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
        validate_default(def)?;
    }
    Ok(())
}

/// default 若存在必须与参数类型匹配（number 可解析、select 必须是选项之一）。
fn validate_default(def: &ScriptParamDef) -> Result<(), AppError> {
    let Some(default) = def
        .default_value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    match def.param_type {
        ScriptParamType::Number if default.parse::<f64>().is_err() => {
            return Err(AppError::ValidationError {
                message: format!("param {} default must be a number", def.key),
            });
        }
        ScriptParamType::Select if !def.options.iter().any(|o| o == default) => {
            return Err(AppError::ValidationError {
                message: format!("param {} default must be one of options", def.key),
            });
        }
        _ => {}
    }
    Ok(())
}

/// 运行时校验（在 default 兜底后的有效值上执行）。
pub fn validate_run_params(
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

/// 宽容布尔解析（表单值可能是 "true"/"false"，也可能来自 API 的 "1"/"yes"）。
pub fn is_truthy(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(
        key: &str,
        param_type: ScriptParamType,
        pass_as: crate::domain::ScriptParamPassAs,
    ) -> ScriptParamDef {
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
    fn name_is_trimmed_and_required() {
        assert_eq!(validate_name("  hello  ").unwrap(), "hello");
        assert!(matches!(
            validate_name("   "),
            Err(AppError::ValidationError { .. })
        ));
    }

    #[test]
    fn env_prefix_rules() {
        assert_eq!(validate_env_prefix(" PARAM_ ").unwrap(), "PARAM_");
        assert_eq!(validate_env_prefix("X").unwrap(), "X");
        assert_eq!(validate_env_prefix("_a1").unwrap(), "_a1");

        for bad in [
            "",
            "  ",
            "1A",
            "PA-RAM",
            "PA RAM",
            "前缀",
            "P$",
            "P".repeat(33).as_str(),
        ] {
            assert!(
                matches!(
                    validate_env_prefix(bad),
                    Err(AppError::ValidationError { .. })
                ),
                "prefix {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn venv_name_rules() {
        assert_eq!(validate_venv_name(" web-scraper ").unwrap(), "web-scraper");
        assert_eq!(validate_venv_name("a").unwrap(), "a");
        assert_eq!(validate_venv_name("_1").unwrap(), "_1");

        for bad in [
            "",
            "  ",
            "a/b",
            "..",
            "a b",
            "名",
            "a$b",
            "a".repeat(65).as_str(),
        ] {
            assert!(
                matches!(
                    validate_venv_name(bad),
                    Err(AppError::ValidationError { .. })
                ),
                "venv name {bad:?} should be rejected"
            );
        }
    }

    #[test]
    fn param_key_charset_rules() {
        assert!(validate_params_schema(&[def(
            "a",
            ScriptParamType::String,
            crate::domain::ScriptParamPassAs::Env
        )])
        .is_ok());
        assert!(validate_params_schema(&[def(
            "_x9",
            ScriptParamType::String,
            crate::domain::ScriptParamPassAs::Env
        )])
        .is_ok());
        let long_key = "k".repeat(64);
        assert!(validate_params_schema(&[def(
            &long_key,
            ScriptParamType::String,
            crate::domain::ScriptParamPassAs::Env
        )])
        .is_ok());

        for bad in ["", "9a", "my-key", "my key", "键", "a$b"] {
            assert!(
                matches!(
                    validate_params_schema(&[def(
                        bad,
                        ScriptParamType::String,
                        crate::domain::ScriptParamPassAs::Env
                    )]),
                    Err(AppError::ValidationError { .. })
                ),
                "key {bad:?} should be rejected"
            );
        }
        let over = "k".repeat(65);
        assert!(matches!(
            validate_params_schema(&[def(
                &over,
                ScriptParamType::String,
                crate::domain::ScriptParamPassAs::Env
            )]),
            Err(AppError::ValidationError { .. })
        ));
    }

    #[test]
    fn duplicate_and_select_rules() {
        let d = def(
            "k",
            ScriptParamType::String,
            crate::domain::ScriptParamPassAs::Env,
        );
        assert!(matches!(
            validate_params_schema(&[d.clone(), d]),
            Err(AppError::ValidationError { .. })
        ));

        let mut s = def(
            "s",
            ScriptParamType::Select,
            crate::domain::ScriptParamPassAs::Env,
        );
        assert!(matches!(
            validate_params_schema(&[s.clone()]),
            Err(AppError::ValidationError { .. })
        ));
        s.options = vec!["a".into(), "b".into()];
        assert!(validate_params_schema(&[s]).is_ok());
    }

    #[test]
    fn default_must_match_type() {
        let mut n = def(
            "n",
            ScriptParamType::Number,
            crate::domain::ScriptParamPassAs::Env,
        );
        n.default_value = Some("abc".into());
        assert!(matches!(
            validate_params_schema(&[n.clone()]),
            Err(AppError::ValidationError { .. })
        ));
        n.default_value = Some("3.14".into());
        assert!(validate_params_schema(&[n]).is_ok());

        let mut s = def(
            "s",
            ScriptParamType::Select,
            crate::domain::ScriptParamPassAs::Env,
        );
        s.options = vec!["a".into(), "b".into()];
        s.default_value = Some("c".into());
        assert!(matches!(
            validate_params_schema(&[s.clone()]),
            Err(AppError::ValidationError { .. })
        ));
        s.default_value = Some("b".into());
        assert!(validate_params_schema(&[s]).is_ok());
    }

    #[test]
    fn run_param_rules() {
        let mut req = def(
            "r",
            ScriptParamType::String,
            crate::domain::ScriptParamPassAs::Env,
        );
        req.required = true;
        let mut num = def(
            "n",
            ScriptParamType::Number,
            crate::domain::ScriptParamPassAs::Env,
        );
        num.default_value = None;
        let mut sel = def(
            "s",
            ScriptParamType::Select,
            crate::domain::ScriptParamPassAs::Env,
        );
        sel.options = vec!["a".into(), "b".into()];
        let schema = vec![req, num, sel];

        let ok = HashMap::from([
            ("r".to_string(), "v".to_string()),
            ("n".to_string(), "10".to_string()),
            ("s".to_string(), "a".to_string()),
        ]);
        assert!(validate_run_params(&schema, &ok).is_ok());

        let missing = HashMap::from([("n".to_string(), "1".to_string())]);
        assert!(matches!(
            validate_run_params(&schema, &missing),
            Err(AppError::ValidationError { .. })
        ));

        let bad_num = HashMap::from([
            ("r".to_string(), "v".to_string()),
            ("n".to_string(), "x".to_string()),
        ]);
        assert!(matches!(
            validate_run_params(&schema, &bad_num),
            Err(AppError::ValidationError { .. })
        ));

        let bad_opt = HashMap::from([
            ("r".to_string(), "v".to_string()),
            ("s".to_string(), "z".to_string()),
        ]);
        assert!(matches!(
            validate_run_params(&schema, &bad_opt),
            Err(AppError::ValidationError { .. })
        ));
    }

    #[test]
    fn truthy_values() {
        for yes in ["1", "true", "TRUE", " yes ", "On"] {
            assert!(is_truthy(yes));
        }
        for no in ["", "0", "false", "no", "off", "2"] {
            assert!(!is_truthy(no));
        }
    }
}
