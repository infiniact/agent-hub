use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::chat_tool::{
    ChatTool, ChatToolContact, ChatToolMessage, CreateChatToolRequest, UpdateChatToolRequest,
};
use crate::state::AppState;

const CHAT_TOOL_COLS: &str =
    "id, name, plugin_type, config_json, linked_agent_id, status, status_message, auto_reply_mode, workspace_id, messages_received, messages_sent, last_active_at, created_at, updated_at";

fn row_to_chat_tool(row: &rusqlite::Row) -> rusqlite::Result<ChatTool> {
    Ok(ChatTool {
        id: row.get(0)?,
        name: row.get(1)?,
        plugin_type: row.get(2)?,
        config_json: row.get(3)?,
        linked_agent_id: row.get(4)?,
        status: row.get(5)?,
        status_message: row.get(6)?,
        auto_reply_mode: row.get(7)?,
        workspace_id: row.get(8)?,
        messages_received: row.get(9)?,
        messages_sent: row.get(10)?,
        last_active_at: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub fn list_chat_tools(state: &AppState, workspace_id: Option<&str>) -> AppResult<Vec<ChatTool>> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) =
        if let Some(ws_id) = workspace_id {
            (
                format!(
                    "SELECT {CHAT_TOOL_COLS} FROM chat_tools WHERE workspace_id = ?1 ORDER BY created_at ASC"
                ),
                vec![Box::new(ws_id.to_string())],
            )
        } else {
            (
                format!("SELECT {CHAT_TOOL_COLS} FROM chat_tools ORDER BY created_at ASC"),
                vec![],
            )
        };

    let mut stmt = db
        .prepare(&sql)
        .map_err(|e| AppError::Database(e.to_string()))?;

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let tools = stmt
        .query_map(param_refs.as_slice(), |row| row_to_chat_tool(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(tools)
}

pub fn get_chat_tool(state: &AppState, id: &str) -> AppResult<ChatTool> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.query_row(
        &format!("SELECT {CHAT_TOOL_COLS} FROM chat_tools WHERE id = ?1"),
        params![id],
        |row| row_to_chat_tool(row),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("ChatTool {id} not found"))
        }
        _ => AppError::Database(e.to_string()),
    })
}

pub fn create_chat_tool(state: &AppState, req: CreateChatToolRequest) -> AppResult<ChatTool> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;

    db.execute(
        "INSERT INTO chat_tools (id, name, plugin_type, config_json, linked_agent_id, auto_reply_mode, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, req.name, req.plugin_type, req.config_json, req.linked_agent_id, req.auto_reply_mode, req.workspace_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    drop(db);
    get_chat_tool(state, &id)
}

pub fn update_chat_tool(
    state: &AppState,
    id: &str,
    req: UpdateChatToolRequest,
) -> AppResult<ChatTool> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;

    if let Some(name) = &req.name {
        db.execute(
            "UPDATE chat_tools SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![name, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }
    if let Some(config_json) = &req.config_json {
        db.execute(
            "UPDATE chat_tools SET config_json = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![config_json, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }
    if let Some(linked_agent_id) = &req.linked_agent_id {
        db.execute(
            "UPDATE chat_tools SET linked_agent_id = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![linked_agent_id, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }
    if let Some(auto_reply_mode) = &req.auto_reply_mode {
        db.execute(
            "UPDATE chat_tools SET auto_reply_mode = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![auto_reply_mode, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    drop(db);
    get_chat_tool(state, id)
}

pub fn delete_chat_tool(state: &AppState, id: &str) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.execute("DELETE FROM chat_tools WHERE id = ?1", params![id])
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn update_chat_tool_status(
    state: &AppState,
    id: &str,
    status: &str,
    message: Option<&str>,
) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE chat_tools SET status = ?1, status_message = ?2, updated_at = datetime('now') WHERE id = ?3",
        params![status, message, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Reset any chat tools that are in non-stopped states back to "stopped".
/// Called on app startup to clear stale statuses from a previous session.
pub fn reset_stale_statuses(state: &AppState) -> AppResult<u64> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    let count = db
        .execute(
            "UPDATE chat_tools SET status = 'stopped', status_message = 'Reset on app restart' WHERE status NOT IN ('stopped')",
            [],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(count as u64)
}

pub fn increment_message_count(state: &AppState, id: &str, direction: &str) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    let col = if direction == "incoming" {
        "messages_received"
    } else {
        "messages_sent"
    };
    db.execute(
        &format!(
            "UPDATE chat_tools SET {col} = {col} + 1, last_active_at = datetime('now'), updated_at = datetime('now') WHERE id = ?1"
        ),
        params![id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

// ── Messages ──

const MESSAGE_COLS: &str =
    "id, chat_tool_id, direction, external_sender_id, external_sender_name, content, content_type, agent_response, is_processed, error_message, created_at";

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<ChatToolMessage> {
    Ok(ChatToolMessage {
        id: row.get(0)?,
        chat_tool_id: row.get(1)?,
        direction: row.get(2)?,
        external_sender_id: row.get(3)?,
        external_sender_name: row.get(4)?,
        content: row.get(5)?,
        content_type: row.get(6)?,
        agent_response: row.get(7)?,
        is_processed: row.get::<_, i32>(8)? != 0,
        error_message: row.get(9)?,
        created_at: row.get(10)?,
    })
}

pub fn save_chat_tool_message(
    state: &AppState,
    chat_tool_id: &str,
    direction: &str,
    external_sender_id: Option<&str>,
    external_sender_name: Option<&str>,
    content: &str,
    content_type: &str,
) -> AppResult<ChatToolMessage> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;

    db.execute(
        "INSERT INTO chat_tool_messages (id, chat_tool_id, direction, external_sender_id, external_sender_name, content, content_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, chat_tool_id, direction, external_sender_id, external_sender_name, content, content_type],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    db.query_row(
        &format!("SELECT {MESSAGE_COLS} FROM chat_tool_messages WHERE id = ?1"),
        params![id],
        |row| row_to_message(row),
    )
    .map_err(|e| AppError::Database(e.to_string()))
}

pub fn list_chat_tool_messages(
    state: &AppState,
    chat_tool_id: &str,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<ChatToolMessage>> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!(
            "SELECT {MESSAGE_COLS} FROM chat_tool_messages WHERE chat_tool_id = ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let messages = stmt
        .query_map(params![chat_tool_id, limit, offset], |row| {
            row_to_message(row)
        })
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(messages)
}

pub fn mark_message_processed(
    state: &AppState,
    message_id: &str,
    agent_response: &str,
) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE chat_tool_messages SET is_processed = 1, agent_response = ?1 WHERE id = ?2",
        params![agent_response, message_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn mark_message_error(state: &AppState, message_id: &str, error: &str) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE chat_tool_messages SET error_message = ?1 WHERE id = ?2",
        params![error, message_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// List unprocessed incoming messages for a chat tool, ordered oldest first.
pub fn list_unprocessed_messages(
    state: &AppState,
    chat_tool_id: &str,
) -> AppResult<Vec<ChatToolMessage>> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!(
            "SELECT {MESSAGE_COLS} FROM chat_tool_messages WHERE chat_tool_id = ?1 AND direction = 'incoming' AND is_processed = 0 AND error_message IS NULL ORDER BY created_at ASC"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let messages = stmt
        .query_map(params![chat_tool_id], |row| row_to_message(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(messages)
}

/// Mark multiple messages as processed in a single transaction.
pub fn mark_messages_processed_batch(
    state: &AppState,
    message_ids: &[String],
    agent_response: &str,
) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;

    for mid in message_ids {
        db.execute(
            "UPDATE chat_tool_messages SET is_processed = 1, agent_response = ?1 WHERE id = ?2",
            params![agent_response, mid],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    Ok(())
}

// ── Contacts ──

const CONTACT_COLS: &str =
    "id, chat_tool_id, external_id, name, avatar_url, contact_type, is_blocked, created_at, updated_at";

fn row_to_contact(row: &rusqlite::Row) -> rusqlite::Result<ChatToolContact> {
    Ok(ChatToolContact {
        id: row.get(0)?,
        chat_tool_id: row.get(1)?,
        external_id: row.get(2)?,
        name: row.get(3)?,
        avatar_url: row.get(4)?,
        contact_type: row.get(5)?,
        is_blocked: row.get::<_, i32>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub fn upsert_contacts(
    state: &AppState,
    chat_tool_id: &str,
    contacts: &[(String, String, Option<String>, String)], // (external_id, name, avatar_url, contact_type)
) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;

    for (external_id, name, avatar_url, contact_type) in contacts {
        let id = uuid::Uuid::new_v4().to_string();
        db.execute(
            "INSERT INTO chat_tool_contacts (id, chat_tool_id, external_id, name, avatar_url, contact_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chat_tool_id, external_id) DO UPDATE SET
               name = excluded.name,
               avatar_url = excluded.avatar_url,
               contact_type = excluded.contact_type,
               updated_at = datetime('now')",
            params![id, chat_tool_id, external_id, name, avatar_url, contact_type],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    Ok(())
}

pub fn list_contacts(
    state: &AppState,
    chat_tool_id: &str,
) -> AppResult<Vec<ChatToolContact>> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!(
            "SELECT {CONTACT_COLS} FROM chat_tool_contacts WHERE chat_tool_id = ?1 ORDER BY name ASC"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let contacts = stmt
        .query_map(params![chat_tool_id], |row| row_to_contact(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(contacts)
}

pub fn set_contact_blocked(
    state: &AppState,
    contact_id: &str,
    blocked: bool,
) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE chat_tool_contacts SET is_blocked = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![blocked as i32, contact_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn update_last_active(state: &AppState, id: &str) -> AppResult<()> {
    let db = state
        .db
        .lock()
        .map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE chat_tools SET last_active_at = datetime('now') WHERE id = ?1",
        params![id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}
