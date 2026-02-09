use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::session::{CreateSessionRequest, Session};
use crate::state::AppState;

pub fn create_session(state: &AppState, req: CreateSessionRequest) -> AppResult<Session> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    db.execute(
        "INSERT INTO sessions (id, agent_id, title, mode) VALUES (?1, ?2, ?3, ?4)",
        params![id, req.agent_id, req.title, req.mode],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    drop(db);
    get_session(state, &id)
}

pub fn get_session(state: &AppState, id: &str) -> AppResult<Session> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.query_row(
        "SELECT id, agent_id, title, mode, acp_session_id, created_at, updated_at FROM sessions WHERE id = ?1",
        params![id],
        |row| {
            Ok(Session {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                title: row.get(2)?,
                mode: row.get(3)?,
                acp_session_id: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Session {id} not found")),
        _ => AppError::Database(e.to_string()),
    })
}

pub fn list_sessions(state: &AppState, agent_id: &str) -> AppResult<Vec<Session>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare("SELECT id, agent_id, title, mode, acp_session_id, created_at, updated_at FROM sessions WHERE agent_id = ?1 ORDER BY updated_at DESC")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let sessions = stmt
        .query_map(params![agent_id], |row| {
            Ok(Session {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                title: row.get(2)?,
                mode: row.get(3)?,
                acp_session_id: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(sessions)
}

pub fn delete_session(state: &AppState, id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute("DELETE FROM sessions WHERE id = ?1", params![id])
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn update_session_acp_id(state: &AppState, id: &str, acp_session_id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE sessions SET acp_session_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![acp_session_id, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}
