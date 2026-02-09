use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::message::ChatMessage;
use crate::state::AppState;

pub fn save_message(state: &AppState, msg: &ChatMessage) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "INSERT INTO messages (id, session_id, role, content_json, tool_calls_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![msg.id, msg.session_id, msg.role, msg.content_json, msg.tool_calls_json],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn get_messages(state: &AppState, session_id: &str) -> AppResult<Vec<ChatMessage>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare("SELECT id, session_id, role, content_json, tool_calls_json, created_at FROM messages WHERE session_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let messages = stmt
        .query_map(params![session_id], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content_json: row.get(3)?,
                tool_calls_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(messages)
}

pub fn delete_messages_for_session(state: &AppState, session_id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute("DELETE FROM messages WHERE session_id = ?1", params![session_id])
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}
