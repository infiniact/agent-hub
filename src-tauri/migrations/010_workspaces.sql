-- Workspace entity: isolated container with its own working directory and agent instances
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT NOT NULL DEFAULT 'folder',
    working_directory TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Seed a default workspace using the current working_directory setting
INSERT OR IGNORE INTO workspaces (id, name, icon, working_directory)
VALUES ('default-workspace', 'Default', 'home',
    COALESCE((SELECT value FROM settings WHERE key = 'working_directory'), ''));

-- Track the active workspace in settings
INSERT OR IGNORE INTO settings (key, value) VALUES ('active_workspace_id', 'default-workspace');
