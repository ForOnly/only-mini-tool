-- 脚本库：正文存 SQLite；workspace 仅为执行 cwd
CREATE TABLE IF NOT EXISTS scripts (
  id                   INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  name                 TEXT    NOT NULL UNIQUE,
  description          TEXT    NOT NULL DEFAULT '',
  language             TEXT    NOT NULL DEFAULT 'python',
  body                 TEXT    NOT NULL DEFAULT '',
  workspace_path       TEXT,
  interpreter_path     TEXT,
  env_json             TEXT    NOT NULL DEFAULT '{}',
  params_schema_json   TEXT    NOT NULL DEFAULT '[]',
  args_template_json   TEXT    NOT NULL DEFAULT '{"before":[],"after":[]}',
  created_at           TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at           TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_scripts_updated_at ON scripts(updated_at);
