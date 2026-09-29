-- 004: scripts 表增加 venv 绑定列（命名 venv 局部绑定；NULL = 跟随全局链）
ALTER TABLE scripts ADD COLUMN venv_name TEXT;
