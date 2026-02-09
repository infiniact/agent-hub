use crate::db::{agent_md, agent_repo};
use crate::error::AppResult;
use crate::models::agent::{AgentConfig, CreateAgentRequest, UpdateAgentRequest};
use crate::state::AppState;

#[tauri::command]
pub async fn list_agents(state: tauri::State<'_, AppState>) -> AppResult<Vec<AgentConfig>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || agent_repo::list_agents(&state))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn get_agent(state: tauri::State<'_, AppState>, id: String) -> AppResult<AgentConfig> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || agent_repo::get_agent(&state, &id))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn create_agent(
    state: tauri::State<'_, AppState>,
    request: CreateAgentRequest,
) -> AppResult<AgentConfig> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let agent = agent_repo::create_agent(&state, request)?;
        // Write markdown file and update DB with path
        if let Ok(md_path) = agent_md::write_agent_md(&agent) {
            let path_str = md_path.to_string_lossy().to_string();
            let _ = agent_repo::update_agent_md_path(&state, &agent.id, &path_str);
        }
        agent_repo::get_agent(&state, &agent.id)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn update_agent(
    state: tauri::State<'_, AppState>,
    id: String,
    request: UpdateAgentRequest,
) -> AppResult<AgentConfig> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let agent = agent_repo::update_agent(&state, &id, request)?;
        // Write markdown file and update DB with path
        if let Ok(md_path) = agent_md::write_agent_md(&agent) {
            let path_str = md_path.to_string_lossy().to_string();
            let _ = agent_repo::update_agent_md_path(&state, &agent.id, &path_str);
        }
        agent_repo::get_agent(&state, &agent.id)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn delete_agent(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        agent_md::delete_agent_md(&id);
        agent_repo::delete_agent(&state, &id)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn set_control_hub(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<AgentConfig> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || agent_repo::set_control_hub(&state, &agent_id))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn get_control_hub(
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<AgentConfig>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || agent_repo::get_control_hub(&state))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}
