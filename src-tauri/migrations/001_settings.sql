CREATE TABLE IF NOT EXISTS schema_version (
  version INTEGER PRIMARY KEY NOT NULL,
  applied_at TEXT NOT NULL
);

-- 业务表约定：代理主键 id + created_at / updated_at 审计字段
CREATE TABLE IF NOT EXISTS settings (
  id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  key        TEXT    NOT NULL UNIQUE,
  value      TEXT    NOT NULL,
  created_at TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_settings_key ON settings(key);
