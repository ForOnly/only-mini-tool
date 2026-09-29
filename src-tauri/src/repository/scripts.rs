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
                    env_json, params_schema_json, args_template_json, created_at, updated_at
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
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
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
        let name = payload.name.trim();
        if name.is_empty() {
            return Err(AppError::ValidationError {
                message: "script name is required".into(),
            });
        }
        conn.execute(
            "INSERT INTO scripts (name, description, body) VALUES (?1, ?2, ?3)",
            params![name, payload.description.trim(), payload.body.as_str()],
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
        let id = conn.last_insert_rowid();
        Self::get(conn, id)
    }

    pub fn update(
        conn: &Connection,
        id: i64,
        payload: &ScriptUpdate,
    ) -> Result<ScriptDto, AppError> {
        let name = payload.name.trim();
        if name.is_empty() {
            return Err(AppError::ValidationError {
                message: "script name is required".into(),
            });
        }
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
                    env_json = ?6,
                    params_schema_json = ?7,
                    args_template_json = ?8,
                    updated_at = datetime('now')
                 WHERE id = ?9",
                params![
                    name,
                    payload.description.trim(),
                    payload.body.as_str(),
                    workspace,
                    interpreter,
                    env_json,
                    params_schema_json,
                    args_template_json,
                    id
                ],
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

    pub fn rename(conn: &Connection, id: i64, name: &str) -> Result<ScriptDto, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::ValidationError {
                message: "script name is required".into(),
            });
        }
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

fn is_unique_violation(err: &rusqlite::Error) -> bool {
    err.to_string().to_ascii_lowercase().contains("unique")
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
