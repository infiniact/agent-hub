use serde::Serialize;

use crate::acp::{client, discovery, manager};
use crate::db::agent_repo;
use crate::error::{AppError, AppResult};
use crate::models::agent::DiscoveredAgent;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub status: String,
}

#[tauri::command]
pub async fn discover_agents(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<DiscoveredAgent>> {
    let agents = discovery::discover_agents().await?;

    // Save discovered agents to DB
    let state_clone = state.inner().clone();
    let agents_clone = agents.clone();
    tokio::task::spawn_blocking(move || {
        for agent in &agents_clone {
            let _ = agent_repo::save_discovered_agent(&state_clone, agent);
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut discovered = state.discovered_agents.lock().await;
    *discovered = agents.clone();

    Ok(agents)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn spawn_agent(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    command: String,
    args: Vec<String>,
) -> AppResult<AgentStatus> {
    let processes = state.agent_processes.lock().await;
    if processes.contains_key(&agent_id) {
        return Err(AppError::AgentAlreadyRunning(agent_id));
    }
    drop(processes);

    let process = manager::spawn_agent_process(&agent_id, &command, &args).await?;
    let stdin_handle = process.stdin.clone();

    let mut processes = state.agent_processes.lock().await;
    processes.insert(agent_id.clone(), process);
    drop(processes);

    let mut stdins = state.agent_stdins.lock().await;
    stdins.insert(agent_id.clone(), stdin_handle);

    Ok(AgentStatus {
        agent_id,
        status: "Starting".into(),
    })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn initialize_agent(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<serde_json::Value> {
    let mut processes = state.agent_processes.lock().await;
    let process = processes
        .get_mut(&agent_id)
        .ok_or_else(|| AppError::AgentNotRunning(agent_id.clone()))?;

    let response = client::initialize_agent(process).await?;
    process.status = manager::AgentProcessStatus::Running;

    Ok(response)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn create_acp_session(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    session_id: String,
) -> AppResult<String> {
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into());

    let mut processes = state.agent_processes.lock().await;
    let process = processes
        .get_mut(&agent_id)
        .ok_or_else(|| AppError::AgentNotRunning(agent_id.clone()))?;

    let acp_session_id = client::create_session(process, &cwd).await?;

    drop(processes);

    // Update session in DB
    let state_clone = state.inner().clone();
    let session_id_clone = session_id.clone();
    let acp_sid_clone = acp_session_id.clone();
    tokio::task::spawn_blocking(move || {
        crate::db::session_repo::update_session_acp_id(
            &state_clone,
            &session_id_clone,
            &acp_sid_clone,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // Track in state
    let mut acp_sessions = state.acp_sessions.lock().await;
    acp_sessions.insert(
        session_id,
        crate::state::AcpSessionInfo {
            session_id: acp_session_id.clone(),
            agent_id,
            acp_session_id: acp_session_id.clone(),
        },
    );

    Ok(acp_session_id)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_agent_status(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<AgentStatus> {
    let processes = state.agent_processes.lock().await;
    let status = processes
        .get(&agent_id)
        .map(|p| p.status.to_string())
        .unwrap_or_else(|| "Stopped".into());

    Ok(AgentStatus { agent_id, status })
}

#[tauri::command(rename_all = "camelCase")]
pub async fn stop_agent(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<()> {
    let mut processes = state.agent_processes.lock().await;
    if let Some(mut process) = processes.remove(&agent_id) {
        manager::stop_agent_process(&mut process).await?;
    }

    // Clean up stdin handle
    let mut stdins = state.agent_stdins.lock().await;
    stdins.remove(&agent_id);

    // Clean up ACP sessions for this agent
    let mut acp_sessions = state.acp_sessions.lock().await;
    acp_sessions.retain(|_, info| info.agent_id != agent_id);

    Ok(())
}
