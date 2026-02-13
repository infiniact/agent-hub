-- Migration 008: Add scheduling support for task runs
-- Enables time-based filtering and scheduled/recurring task execution

-- Add scheduling related columns
ALTER TABLE task_runs ADD COLUMN schedule_type TEXT NOT NULL DEFAULT 'none'
    CHECK(schedule_type IN ('none', 'once', 'recurring'));
ALTER TABLE task_runs ADD COLUMN scheduled_time TEXT;  -- ISO 8601 datetime for one-time execution
ALTER TABLE task_runs ADD COLUMN recurrence_pattern TEXT;  -- JSON: {"frequency":"daily","time":"09:00","interval":1}
ALTER TABLE task_runs ADD COLUMN next_run_at TEXT;  -- ISO 8601 datetime for next scheduled run
ALTER TABLE task_runs ADD COLUMN is_paused INTEGER NOT NULL DEFAULT 0;  -- 0 = active, 1 = paused

-- Create index for efficient scheduled task lookup
CREATE INDEX idx_task_runs_scheduled ON task_runs(next_run_at)
    WHERE schedule_type != 'none' AND is_paused = 0;
