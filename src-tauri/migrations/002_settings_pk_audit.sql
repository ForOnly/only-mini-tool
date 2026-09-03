-- 将早期「key 作主键、无审计列」的 settings 升级为 id PK + 审计字段。
-- 仅拷贝 key/value；时间戳用默认值（旧库无审计列时也能跑）。

CREATE TABLE settings__new (
  id         INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
  key        TEXT    NOT NULL UNIQUE,
  value      TEXT    NOT NULL,
  created_at TEXT    NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO settings__new (key, value)
SELECT key, value FROM settings;

DROP TABLE settings;
ALTER TABLE settings__new RENAME TO settings;

CREATE INDEX IF NOT EXISTS idx_settings_key ON settings(key);
