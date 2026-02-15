-- Chat tool instances: each represents a configured chat platform integration
CREATE TABLE IF NOT EXISTS chat_tools (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    plugin_type TEXT NOT NULL DEFAULT 'wechat',
    config_json TEXT NOT NULL DEFAULT '{}',
    linked_agent_id TEXT DEFAULT NULL,
    status TEXT NOT NULL DEFAULT 'stopped',
    status_message TEXT DEFAULT NULL,
    auto_reply_mode TEXT NOT NULL DEFAULT 'all',
    workspace_id TEXT DEFAULT NULL,
    messages_received INTEGER NOT NULL DEFAULT 0,
    messages_sent INTEGER NOT NULL DEFAULT 0,
    last_active_at TEXT DEFAULT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_tools_workspace ON chat_tools(workspace_id);
CREATE INDEX IF NOT EXISTS idx_chat_tools_status ON chat_tools(status);

-- Chat tool message log: tracks all incoming/outgoing messages through the bridge
CREATE TABLE IF NOT EXISTS chat_tool_messages (
    id TEXT PRIMARY KEY,
    chat_tool_id TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT 'incoming',
    external_sender_id TEXT DEFAULT NULL,
    external_sender_name TEXT DEFAULT NULL,
    content TEXT NOT NULL DEFAULT '',
    content_type TEXT NOT NULL DEFAULT 'text',
    agent_response TEXT DEFAULT NULL,
    is_processed INTEGER NOT NULL DEFAULT 0,
    error_message TEXT DEFAULT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (chat_tool_id) REFERENCES chat_tools(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_tool_messages_tool ON chat_tool_messages(chat_tool_id);
CREATE INDEX IF NOT EXISTS idx_chat_tool_messages_created ON chat_tool_messages(created_at);

-- Chat tool contacts: cached contact list from external platform
CREATE TABLE IF NOT EXISTS chat_tool_contacts (
    id TEXT PRIMARY KEY,
    chat_tool_id TEXT NOT NULL,
    external_id TEXT NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    avatar_url TEXT DEFAULT NULL,
    contact_type TEXT NOT NULL DEFAULT 'personal',
    is_blocked INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (chat_tool_id) REFERENCES chat_tools(id) ON DELETE CASCADE,
    UNIQUE(chat_tool_id, external_id)
);

CREATE INDEX IF NOT EXISTS idx_chat_tool_contacts_tool ON chat_tool_contacts(chat_tool_id);
