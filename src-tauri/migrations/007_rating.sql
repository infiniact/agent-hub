-- Add rating column to task_runs table
ALTER TABLE task_runs ADD COLUMN rating INTEGER DEFAULT NULL;

-- Add index for rating queries
CREATE INDEX IF NOT EXISTS idx_task_runs_rating ON task_runs(rating);
