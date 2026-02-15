use crate::db::{agent_md, agent_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::{AgentConfig, CreateAgentRequest, UpdateAgentRequest};
use crate::state::AppState;
use crate::acp::{client, discovery, manager, provisioner};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_agents(
    state: tauri::State<'_, AppState>,
    workspace_id: Option<String>,
) -> AppResult<Vec<AgentConfig>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || agent_repo::list_agents(&state, workspace_id.as_deref()))
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
        // Regenerate agents registry
        if let Ok(all_agents) = agent_repo::list_agents(&state, None) {
            let _ = agent_md::write_agents_registry(&all_agents);
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
        // Regenerate agents registry
        if let Ok(all_agents) = agent_repo::list_agents(&state, None) {
            let _ = agent_md::write_agents_registry(&all_agents);
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
        agent_repo::delete_agent(&state, &id)?;
        // Regenerate agents registry
        if let Ok(all_agents) = agent_repo::list_agents(&state, None) {
            let _ = agent_md::write_agents_registry(&all_agents);
        }
        Ok(())
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

#[tauri::command(rename_all = "camelCase")]
pub async fn get_control_hub(
    state: tauri::State<'_, AppState>,
    workspace_id: Option<String>,
) -> AppResult<Option<AgentConfig>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || agent_repo::get_control_hub(&state, workspace_id.as_deref()))
        .await
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))?
}

/// Enable a previously disabled agent, performing a health check before confirming.
/// If the health check fails, the agent is reverted to disabled with the new error.
#[tauri::command(rename_all = "camelCase")]
pub async fn enable_agent(
    _app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<AgentConfig> {
    // 1. Fetch agent, verify it exists
    let agent: AgentConfig = {
        let state_clone = state.inner().clone();
        let aid = agent_id.clone();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &aid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // 2. Temporarily mark as enabled
    {
        let state_clone = state.inner().clone();
        let aid = agent_id.clone();
        tokio::task::spawn_blocking(move || {
            agent_repo::update_agent(&state_clone, &aid, UpdateAgentRequest {
                is_enabled: Some(true),
                disabled_reason: None,
                ..Default::default()
            })
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    // 3. Health check: resolve command, spawn, initialize ACP
    let acp_command = match agent.acp_command.as_ref() {
        Some(cmd) if !cmd.is_empty() => cmd.clone(),
        _ => {
            // No ACP command — just enable without health check
            let state_clone = state.inner().clone();
            let aid = agent_id.clone();
            let updated = tokio::task::spawn_blocking(move || {
                let agent = agent_repo::get_agent(&state_clone, &aid)?;
                if let Ok(all) = agent_repo::list_agents(&state_clone, None) {
                    let _ = agent_md::write_agents_registry(&all);
                }
                Ok::<AgentConfig, AppError>(agent)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;
            return Ok(updated);
        }
    };

    let args: Vec<String> = agent
        .acp_args_json
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();

    let health_result = async {
        let resolved = provisioner::resolve_agent_command(&acp_command, &args).await?;
        let extra_env = discovery::get_agent_env_for_command(&resolved.agent_type).await;

        let mut process = manager::spawn_agent_process(
            &agent_id,
            &resolved.command,
            &resolved.args,
            &extra_env,
            &resolved.agent_type,
        ).await?;

        let init_result = client::initialize_agent(&mut process).await;
        let _ = manager::stop_agent_process(&mut process).await;
        init_result?;

        Ok::<(), AppError>(())
    }.await;

    match health_result {
        Ok(()) => {
            // Health check passed — regenerate registry and return updated agent
            let state_clone = state.inner().clone();
            let aid = agent_id.clone();
            let updated = tokio::task::spawn_blocking(move || {
                let agent = agent_repo::get_agent(&state_clone, &aid)?;
                if let Ok(md_path) = agent_md::write_agent_md(&agent) {
                    let path_str = md_path.to_string_lossy().to_string();
                    let _ = agent_repo::update_agent_md_path(&state_clone, &agent.id, &path_str);
                }
                if let Ok(all) = agent_repo::list_agents(&state_clone, None) {
                    let _ = agent_md::write_agents_registry(&all);
                }
                agent_repo::get_agent(&state_clone, &aid)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            Ok(updated)
        }
        Err(e) => {
            // Health check failed — revert to disabled with new error
            let err_msg = e.to_string();
            let state_clone = state.inner().clone();
            let aid = agent_id.clone();
            let reason = err_msg.clone();
            let _ = tokio::task::spawn_blocking(move || {
                agent_repo::disable_agent(&state_clone, &aid, &reason)
            }).await;

            Err(AppError::Internal(format!(
                "Health check failed, agent remains disabled: {}", err_msg
            )))
        }
    }
}
