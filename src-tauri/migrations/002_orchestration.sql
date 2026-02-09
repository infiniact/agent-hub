-- 002_orchestration.sql
-- Adds orchestration support: control hub flag, agent markdown paths, task runs, task assignments

ALTER TABLE agents ADD COLUMN is_control_hub INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agents ADD COLUMN md_file_path TEXT;

CREATE TABLE IF NOT EXISTS task_runs (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL,
    control_hub_agent_id TEXT NOT NULL REFERENCES agents(id),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending','analyzing','running','completed','failed','cancelled')),
    task_plan_json TEXT,
    result_summary TEXT,
    total_tokens_in INTEGER NOT NULL DEFAULT 0,
    total_tokens_out INTEGER NOT NULL DEFAULT 0,
    total_duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS task_assignments (
    id TEXT PRIMARY KEY,
    task_run_id TEXT NOT NULL REFERENCES task_runs(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    agent_name TEXT NOT NULL DEFAULT '',
    sequence_order INTEGER NOT NULL DEFAULT 0,
    input_text TEXT NOT NULL DEFAULT '',
    output_text TEXT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending','running','completed','failed','skipped')),
    model_used TEXT,
    tokens_in INTEGER NOT NULL DEFAULT 0,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_task_assignments_run ON task_assignments(task_run_id, sequence_order);
