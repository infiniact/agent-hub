ALTER TABLE task_assignments ADD COLUMN cache_creation_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_assignments ADD COLUMN cache_read_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_runs ADD COLUMN total_cache_creation_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE task_runs ADD COLUMN total_cache_read_tokens INTEGER NOT NULL DEFAULT 0;
