use crate::db::session_repo;
use crate::error::AppResult;
use crate::models::session::{CreateSessionRequest, Session};
use crate::state::AppState;

#[tauri::command]
pub async fn create_session(
    state: tauri::State<'_, AppState>,
    request: CreateSessionRequest,
) -> AppResult<Session> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || session_repo::create_session(&state, request))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_sessions(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<Vec<Session>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || session_repo::list_sessions(&state, &agent_id))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn load_session(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Session> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || session_repo::get_session(&state, &id))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn delete_session(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || session_repo::delete_session(&state, &id))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}
