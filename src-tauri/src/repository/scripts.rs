use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{
    ScriptArgsTemplate, ScriptCreate, ScriptDto, ScriptParamDef, ScriptSummary, ScriptUpdate,
};
use crate::errors::AppError;

pub struct ScriptsRepo;

impl ScriptsRepo {
    pub fn list(conn: &Connection) -> Result<Vec<ScriptSummary>, AppError> {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, language, updated_at
                 FROM scripts ORDER BY updated_at DESC, id DESC",
            )
            .map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ScriptSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    language: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?);
        }
        Ok(out)
    }

    pub fn get(conn: &Connection, id: i64) -> Result<ScriptDto, AppError> {
        conn.query_row(
            "SELECT id, name, description, language, body, workspace_path, interpreter_path,
                    venv_name, env_json, params_schema_json, args_template_json, created_at, updated_at
             FROM scripts WHERE id = ?1",
            [id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                ))
            },
        )
        .optional()
        .map_err(|e| AppError::DbError {
            message: e.to_string(),
        })?
        .map(
            |(
                id,
                name,
                description,
                language,
                body,
                workspace_path,
                interpreter_path,
                venv_name,
                env_json,
                params_schema_json,
                args_template_json,
                created_at,
                updated_at,
            )| {
                Ok(ScriptDto {
                    id,
                    name,
                    description,
                    language,
                    body,
                    workspace_path,
                    interpreter_path,
                    venv_name,
                    env: parse_env_map(&env_json)?,
                    params_schema: parse_params_schema(&params_schema_json)?,
                    args_template: parse_args_template(&args_template_json)?,
                    created_at,
                    updated_at,
                })
            },
        )
        .ok_or_else(|| AppError::NotFound {
            message: format!("script not found: {id}"),
        })?
    }

    pub fn create(conn: &Connection, payload: &ScriptCreate) -> Result<ScriptDto, AppError> {
        // name/description 已由 service 层归一化校验
        conn.execute(
            "INSERT INTO scripts (name, description, body) VALUES (?1, ?2, ?3)",
            params![payload.name, payload.description, payload.body.as_str()],
        )
        .map_err(|e| {
            if is_unique_violation(&e) {
                AppError::ValidationError {
                    message: format!("script name already exists: {}", payload.name),
                }
            } else {
                AppError::DbError {
                    message: e.to_string(),
                }
            }
        })?;
        let id = conn.last_insert_rowid();
        Self::get(conn, id)
    }

    pub fn update(
        conn: &Connection,
        id: i64,
        payload: &ScriptUpdate,
    ) -> Result<ScriptDto, AppError> {
        // name/description 已由 service 层归一化校验
        let env_json =
            serde_json::to_string(&payload.env).map_err(|e| AppError::InternalError {
                message: format!("serialize env: {e}"),
            })?;
        let params_schema_json =
            serde_json::to_string(&payload.params_schema).map_err(|e| AppError::InternalError {
                message: format!("serialize params_schema: {e}"),
            })?;
        let args_template_json =
            serde_json::to_string(&payload.args_template).map_err(|e| AppError::InternalError {
                message: format!("serialize args_template: {e}"),
            })?;

        let workspace = normalize_opt_path(&payload.workspace_path);
        let interpreter = normalize_opt_path(&payload.interpreter_path);

        let changed = conn
            .execute(
                "UPDATE scripts SET
                    name = ?1,
                    description = ?2,
                    body = ?3,
                    workspace_path = ?4,
                    interpreter_path = ?5,
                    venv_name = ?6,
                    env_json = ?7,
                    params_schema_json = ?8,
                    args_template_json = ?9,
                    updated_at = datetime('now')
                 WHERE id = ?10",
                params![
                    payload.name,
                    payload.description,
                    payload.body.as_str(),
                    workspace,
                    interpreter,
                    normalize_opt_name(&payload.venv_name),
                    env_json,
                    params_schema_json,
                    args_template_json,
                    id
                ],
            )
            .map_err(|e| {
                if is_unique_violation(&e) {
                    AppError::ValidationError {
                        message: format!("script name already exists: {}", payload.name),
                    }
                } else {
                    AppError::DbError {
                        message: e.to_string(),
                    }
                }
            })?;
        if changed == 0 {
            return Err(AppError::NotFound {
                message: format!("script not found: {id}"),
            });
        }
        Self::get(conn, id)
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<(), AppError> {
        let changed = conn
            .execute("DELETE FROM scripts WHERE id = ?1", [id])
            .map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?;
        if changed == 0 {
            return Err(AppError::NotFound {
                message: format!("script not found: {id}"),
            });
        }
        Ok(())
    }

    /// 绑定了指定 venv 的脚本名（删除 venv 前的引用检查）。
    pub fn names_by_venv(conn: &Connection, venv: &str) -> Result<Vec<String>, AppError> {
        let mut stmt = conn
            .prepare("SELECT name FROM scripts WHERE venv_name = ?1 ORDER BY name")
            .map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?;
        let rows = stmt
            .query_map([venv], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| AppError::DbError {
                message: e.to_string(),
            })?);
        }
        Ok(out)
    }

    pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<ScriptDto, AppError> {
        // name 已由 service 层归一化校验
        let changed = conn
            .execute(
                "UPDATE scripts SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
                params![name, id],
            )
            .map_err(|e| {
                if is_unique_violation(&e) {
                    AppError::ValidationError {
                        message: format!("script name already exists: {name}"),
                    }
                } else {
                    AppError::DbError {
                        message: e.to_string(),
                    }
                }
            })?;
        if changed == 0 {
            return Err(AppError::NotFound {
                message: format!("script not found: {id}"),
            });
        }
        Self::get(conn, id)
    }
}

fn normalize_opt_path(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn normalize_opt_name(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// UNIQUE 约束冲突的结构化判断（SQLITE_CONSTRAINT_UNIQUE = 2067）。
/// 字符串匹配旧实现脆弱：依赖错误文案，升级 rusqlite/sqlite 即可能失效。
fn is_unique_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(e, _) if e.extended_code == 2067
    )
}

fn parse_env_map(raw: &str) -> Result<std::collections::HashMap<String, String>, AppError> {
    if raw.trim().is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    serde_json::from_str(raw).map_err(|e| AppError::DbError {
        message: format!("invalid env_json: {e}"),
    })
}

fn parse_params_schema(raw: &str) -> Result<Vec<ScriptParamDef>, AppError> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(raw).map_err(|e| AppError::DbError {
        message: format!("invalid params_schema_json: {e}"),
    })
}

fn parse_args_template(raw: &str) -> Result<ScriptArgsTemplate, AppError> {
    if raw.trim().is_empty() {
        return Ok(ScriptArgsTemplate::default());
    }
    serde_json::from_str(raw).map_err(|e| AppError::DbError {
        message: format!("invalid args_template_json: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ScriptCreate;
    use crate::infrastructure::database::migrations::run_migrations;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        run_migrations(&conn).expect("run migrations");
        conn
    }

    fn create_payload(name: &str) -> ScriptCreate {
        ScriptCreate {
            name: name.into(),
            description: "d".into(),
            body: "print(1)".into(),
        }
    }

    #[test]
    fn create_get_roundtrip() {
        let conn = conn();
        let dto = ScriptsRepo::create(&conn, &create_payload("hello")).expect("create");
        assert_eq!(dto.name, "hello");
        assert_eq!(dto.body, "print(1)");
        assert_eq!(dto.language, "python");
        let fetched = ScriptsRepo::get(&conn, dto.id).expect("get");
        assert_eq!(fetched.id, dto.id);
    }

    #[test]
    fn duplicate_name_is_validation_error() {
        let conn = conn();
        ScriptsRepo::create(&conn, &create_payload("a")).expect("first create");
        let err = ScriptsRepo::create(&conn, &create_payload("a")).expect_err("dup rejected");
        assert!(
            matches!(err, AppError::ValidationError { .. }),
            "unique violation must translate to ValidationError, got: {err:?}"
        );
    }

    #[test]
    fn update_rename_delete_missing_is_not_found() {
        let conn = conn();
        let dto = ScriptsRepo::create(&conn, &create_payload("x")).expect("create");

        let err = ScriptsRepo::delete(&conn, dto.id + 100).expect_err("delete missing");
        assert!(matches!(err, AppError::NotFound { .. }));

        let err = ScriptsRepo::rename(&conn, dto.id + 100, "y").expect_err("rename missing");
        assert!(matches!(err, AppError::NotFound { .. }));

        ScriptsRepo::delete(&conn, dto.id).expect("delete");
        let err = ScriptsRepo::get(&conn, dto.id).expect_err("get deleted");
        assert!(matches!(err, AppError::NotFound { .. }));
    }
}
