-- 004_fix_status_constraints.sql
-- Add 'awaiting_confirmation' to task_runs status, and 'cancelled' to task_assignments status.
-- SQLite does not support ALTER CHECK, so we recreate the tables.

-- 1. Fix task_runs: add 'awaiting_confirmation'
CREATE TABLE task_runs_new (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    user_prompt TEXT NOT NULL,
    control_hub_agent_id TEXT NOT NULL REFERENCES agents(id),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending','analyzing','running','awaiting_confirmation','completed','failed','cancelled')),
    task_plan_json TEXT,
    result_summary TEXT,
    total_tokens_in INTEGER NOT NULL DEFAULT 0,
    total_tokens_out INTEGER NOT NULL DEFAULT 0,
    total_duration_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO task_runs_new SELECT * FROM task_runs;
DROP TABLE task_runs;
ALTER TABLE task_runs_new RENAME TO task_runs;

-- 2. Fix task_assignments: add 'cancelled'
CREATE TABLE task_assignments_new (
    id TEXT PRIMARY KEY,
    task_run_id TEXT NOT NULL REFERENCES task_runs(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    agent_name TEXT NOT NULL DEFAULT '',
    sequence_order INTEGER NOT NULL DEFAULT 0,
    input_text TEXT NOT NULL DEFAULT '',
    output_text TEXT,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending','running','completed','failed','skipped','cancelled')),
    model_used TEXT,
    tokens_in INTEGER NOT NULL DEFAULT 0,
    tokens_out INTEGER NOT NULL DEFAULT 0,
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
INSERT INTO task_assignments_new SELECT * FROM task_assignments;
DROP TABLE task_assignments;
ALTER TABLE task_assignments_new RENAME TO task_assignments;
CREATE INDEX IF NOT EXISTS idx_task_assignments_run ON task_assignments(task_run_id, sequence_order);
