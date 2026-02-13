use serde::Serialize;
use tauri::Emitter;

use crate::acp::{client, discovery, manager, provisioner};
use crate::acp::builtin;
use crate::commands::settings_commands;
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
    // Deploy built-in agent (non-blocking on failure)
    builtin::ensure_builtin_deployed().await;

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

    let process = manager::spawn_agent_process(&agent_id, &command, &args, &std::collections::HashMap::new(), &command).await?;
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

#[derive(Debug, Clone, Serialize)]
pub struct CreateSessionResult {
    pub acp_session_id: String,
    pub models: Vec<crate::acp::client::AgentModel>,
}

#[tauri::command(rename_all = "camelCase")]
pub async fn create_acp_session(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    session_id: String,
) -> AppResult<CreateSessionResult> {
    let cwd = settings_commands::resolve_working_directory(state.inner());

    let mut processes = state.agent_processes.lock().await;
    let process = processes
        .get_mut(&agent_id)
        .ok_or_else(|| AppError::AgentNotRunning(agent_id.clone()))?;

    let (acp_session_id, models) = client::create_session(process, &cwd).await?;

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
        session_id.clone(),
        crate::state::AcpSessionInfo::new(
            session_id.clone(),
            agent_id,
            acp_session_id.clone(),
        ),
    );

    Ok(CreateSessionResult { acp_session_id, models })
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

/// Get models from an ACP agent by creating a temporary session.
/// This can be called after an agent is initialized to discover available models.
/// Note: This creates a temporary ACP session to query models. The session may be
/// kept alive for reuse, or you can call discard_temp_session to clean it up.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_agent_models(
    state: tauri::State<'_, AppState>,
    agent_id: String,
) -> AppResult<GetModelsResult> {
    let cwd = settings_commands::resolve_working_directory(state.inner());

    let mut processes = state.agent_processes.lock().await;
    let process = processes
        .get_mut(&agent_id)
        .ok_or_else(|| AppError::AgentNotRunning(agent_id.clone()))?;

    let (session_id, models) = client::create_session(process, &cwd).await?;
    log::info!("get_agent_models: agent {} returned {} models, ACP session: {}", agent_id, models.len(), session_id);

    drop(processes);

    // Store the temporary session in a special "temp:" namespace
    // This allows the session to be reused for actual conversation later
    let temp_session_key = format!("temp:{}", agent_id);
    let mut acp_sessions = state.acp_sessions.lock().await;
    acp_sessions.insert(
        temp_session_key.clone(),
        crate::state::AcpSessionInfo::new(
            temp_session_key.clone(),
            agent_id.clone(),
            session_id.clone(),
        ),
    );

    Ok(GetModelsResult {
        models,
        temp_acp_session_id: session_id,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct GetModelsResult {
    pub models: Vec<crate::acp::client::AgentModel>,
    /// The ACP session ID that was created to query models.
    /// This can be reused for actual conversation by calling resume_acp_session.
    pub temp_acp_session_id: String,
}

/// End an active ACP session following the ACP protocol.
/// This sends a session/end notification to the agent.
#[tauri::command(rename_all = "camelCase")]
pub async fn end_acp_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> AppResult<()> {
    log::info!("end_acp_session: Ending session {}", session_id);

    // Find the session info
    let (agent_id, acp_session_id) = {
        let acp_sessions = state.acp_sessions.lock().await;
        if let Some(info) = acp_sessions.get(&session_id) {
            (info.agent_id.clone(), info.acp_session_id.clone())
        } else {
            return Err(AppError::Internal(format!("Session {} not found", session_id)));
        }
    };

    // Send session/end to the agent
    let mut processes = state.agent_processes.lock().await;
    if let Some(process) = processes.get_mut(&agent_id) {
        client::end_session(process, &acp_session_id).await?;
    }

    // Mark session as ended in state
    let mut acp_sessions = state.acp_sessions.lock().await;
    if let Some(info) = acp_sessions.get_mut(&session_id) {
        info.mark_ended();
    }

    log::info!("end_acp_session: Session {} ended successfully", session_id);
    Ok(())
}

/// Resume/load an existing ACP session for a given database session.
/// This tries to load the ACP session from the agent, or creates a new one if it doesn't exist.
/// If a temporary session exists (from get_agent_models), it will be reused.
#[tauri::command(rename_all = "camelCase")]
pub async fn resume_acp_session(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> AppResult<ResumeSessionResult> {
    log::info!("resume_acp_session: Resuming session {}", session_id);

    // Get the session from DB to find agent_id
    let session = {
        let state_clone = state.inner().clone();
        let sid = session_id.clone();
        tokio::task::spawn_blocking(move || {
            crate::db::session_repo::get_session(&state_clone, &sid)
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??
    };

    let agent_id = session.agent_id.clone();

    // Get agent config
    let agent_config = {
        let state_clone = state.inner().clone();
        let aid = agent_id.clone();
        tokio::task::spawn_blocking(move || {
            crate::db::agent_repo::get_agent(&state_clone, &aid)
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // Check if we have an ACP session in memory for this exact session_id
    let acp_session_info = {
        let acp_sessions = state.acp_sessions.lock().await;
        acp_sessions.get(&session_id).cloned()
    };

    if let Some(info) = acp_session_info {
        log::info!("Found existing ACP session in memory: {}", info.acp_session_id);

        // Try to load the session to verify it's still valid
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(&agent_id) {
            match client::load_session(process, &info.acp_session_id).await {
                Ok(_history) => {
                    log::info!("Session load successful, resuming existing ACP session");
                    // Update session state to active
                    let mut acp_sessions = state.acp_sessions.lock().await;
                    if let Some(s) = acp_sessions.get_mut(&session_id) {
                        s.mark_active();
                    }
                    drop(acp_sessions);
                    drop(processes);
                    return Ok(ResumeSessionResult {
                        acp_session_id: info.acp_session_id,
                        is_loaded: true,
                        models: Vec::new(),
                    });
                }
                Err(e) => {
                    log::warn!("Session load failed: {}, creating new session", e);
                    // Session is not valid anymore, create new
                    drop(processes);
                    let mut acp_sessions = state.acp_sessions.lock().await;
                    acp_sessions.remove(&session_id);
                    drop(acp_sessions);
                }
            }
        }
    }

    // Check if there's a temporary session we can reuse
    let temp_session_key = format!("temp:{}", agent_id);
    let temp_acp_id = {
        let acp_sessions = state.acp_sessions.lock().await;
        acp_sessions.get(&temp_session_key).map(|info| info.acp_session_id.clone())
    };

    let acp_session_id = if let Some(temp_id) = temp_acp_id {
        log::info!("Reusing temporary ACP session: {}", temp_id);

        // Move the temp session to the actual session
        let mut acp_sessions = state.acp_sessions.lock().await;
        if let Some(mut info) = acp_sessions.remove(&temp_session_key) {
            info.session_id = session_id.clone();
            info.mark_active();
            acp_sessions.insert(session_id.clone(), info);
            temp_id
        } else {
            drop(acp_sessions);
            // Create new session if temp was somehow removed
            create_new_acp_session_internal(&app, &state, &session_id, &agent_id, &agent_config).await?.0
        }
    } else {
        log::info!("No temporary session found, checking DB for existing session");

        // Check if DB has an acp_session_id
        if let Some(db_acp_id) = session.acp_session_id {
            log::info!("Found ACP session ID in DB: {}", db_acp_id);

            // Agent needs to be running first
            let process_running = {
                let processes = state.agent_processes.lock().await;
                processes.contains_key(&agent_id)
            };

            if !process_running {
                log::info!("Agent not running, cannot resume session");
                return Err(AppError::AgentNotRunning(agent_id));
            }

            // Try to load the existing session
            let mut processes = state.agent_processes.lock().await;
            if let Some(process) = processes.get_mut(&agent_id) {
                match client::load_session(process, &db_acp_id).await {
                    Ok(_history) => {
                        log::info!("Successfully loaded ACP session from DB");

                        // Track in state
                        let mut acp_sessions = state.acp_sessions.lock().await;
                        acp_sessions.insert(
                            session_id.clone(),
                            crate::state::AcpSessionInfo::new(
                                session_id.clone(),
                                agent_id.clone(),
                                db_acp_id.clone(),
                            ),
                        );
                        // Mark as active since we successfully loaded it
                        if let Some(info) = acp_sessions.get_mut(&session_id) {
                            info.mark_active();
                        }
                        drop(acp_sessions);
                        drop(processes);

                        return Ok(ResumeSessionResult {
                            acp_session_id: db_acp_id,
                            is_loaded: true,
                            models: Vec::new(),
                        });
                    }
                    Err(e) => {
                        log::warn!("Failed to load ACP session from DB: {}, creating new", e);
                        drop(processes);
                    }
                }
            }
        }

        // Create new session
        log::info!("Creating new ACP session");
        create_new_acp_session_internal(&app, &state, &session_id, &agent_id, &agent_config).await?.0
    };

    Ok(ResumeSessionResult {
        acp_session_id,
        is_loaded: false,
        models: Vec::new(),
    })
}

/// Result of resuming or creating a session
#[derive(Debug, Clone, Serialize)]
pub struct ResumeSessionResult {
    pub acp_session_id: String,
    pub is_loaded: bool,
    pub models: Vec<crate::acp::client::AgentModel>,
}

/// Helper function to create a new ACP session (shared logic)
async fn create_new_acp_session_internal(
    app: &tauri::AppHandle,
    state: &AppState,
    session_id: &str,
    agent_id: &str,
    agent_config: &crate::models::agent::AgentConfig,
) -> AppResult<(String, bool, Vec<crate::acp::client::AgentModel>)> {
    log::info!("Creating new ACP session for agent {} in session {}", agent_id, session_id);

    let mut processes = state.agent_processes.lock().await;
    let process = processes.get_mut(agent_id)
        .ok_or_else(|| AppError::Internal(format!("Agent {} process not found", agent_id)))?;

    let cwd = settings_commands::resolve_working_directory(state);

    let (acp_id, models) = client::create_session(process, &cwd).await?;
    log::info!("Created ACP session: {} with {} models", acp_id, models.len());

    drop(processes);

    // Emit models to frontend
    let command = agent_config.acp_command.clone().unwrap_or_default();
    let model_ids: Vec<String> = models.iter().map(|m| m.model_id.clone()).collect();
    let _ = app.emit("acp:models", &serde_json::json!({
        "command": command,
        "models": model_ids
    }));

    // Update session in DB
    let state_clone = state.clone();
    let session_id_owned = session_id.to_string();
    let acp_id_clone = acp_id.clone();
    tokio::task::spawn_blocking(move || {
        crate::db::session_repo::update_session_acp_id(&state_clone, &session_id_owned, &acp_id_clone)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // Track in state
    let mut acp_sessions = state.acp_sessions.lock().await;
    acp_sessions.insert(
        session_id.to_string(),
        crate::state::AcpSessionInfo::new(
            session_id.to_string(),
            agent_id.to_string(),
            acp_id.clone(),
        ),
    );

    Ok((acp_id, false, models))
}

/// Result of ensure_agent_ready
#[derive(Debug, Clone, Serialize)]
pub struct EnsureAgentReadyResult {
    pub status: String,
    pub models: Vec<crate::acp::client::AgentModel>,
}

/// Ensure an agent is fully ready: spawned, initialized, and models fetched.
/// This is a single command that handles the full agent startup lifecycle:
/// 1. Resolve the correct command + args (sync with discovery, npx auto-upgrade)
/// 2. Spawn the agent process with proper environment
/// 3. ACP initialize handshake
/// 4. session/new to fetch available models (stored as temp session for later reuse)
///
/// If the agent is already running and has cached models, returns cached models
/// unless force_refresh is true.
#[tauri::command(rename_all = "camelCase")]
pub async fn ensure_agent_ready(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    agent_id: String,
    force_refresh: Option<bool>,
) -> AppResult<EnsureAgentReadyResult> {
    let force_refresh = force_refresh.unwrap_or(false);
    log::info!("ensure_agent_ready: Starting for agent {}, force_refresh={}", agent_id, force_refresh);

    // Get agent config from DB
    let agent_config = {
        let state_clone = state.inner().clone();
        let aid = agent_id.clone();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &aid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    let acp_command = agent_config.acp_command.clone().ok_or_else(|| {
        AppError::Internal(format!("Agent {} has no ACP command configured", agent_id))
    })?;

    // Check if already running — and if agent type or CLI version changed
    let needs_restart = {
        let processes = state.agent_processes.lock().await;
        if let Some(existing) = processes.get(&agent_id) {
            // Use agent_type for comparison instead of spawn_command basename.
            // This correctly detects changes for npx agents that all share "npx" as command.
            let expected_basename = std::path::Path::new(&acp_command)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&acp_command);
            // Look up what agent_type this command maps to via dynamic registry
            let expected_type = expected_basename;
            if existing.agent_type != expected_type {
                log::info!(
                    "Agent type changed: {} -> {}, will restart process",
                    existing.agent_type, expected_type
                );
                true
            } else if !existing.cli_version.is_empty() {
                // Check if the on-disk CLI was upgraded since this process started
                if let Some(disk_ver) = builtin::get_cli_version() {
                    if disk_ver != existing.cli_version {
                        log::info!(
                            "CLI version changed on disk: {} -> {}, will restart process",
                            existing.cli_version, disk_ver
                        );
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    };

    // Stop old process if agent type changed
    if needs_restart {
        log::info!("Stopping old agent process for {}", agent_id);
        let mut processes = state.agent_processes.lock().await;
        if let Some(mut old_process) = processes.remove(&agent_id) {
            let _ = manager::stop_agent_process(&mut old_process).await;
        }
        drop(processes);
        let mut stdins = state.agent_stdins.lock().await;
        stdins.remove(&agent_id);
        drop(stdins);
        // Clean up temp ACP sessions for this agent
        let mut acp_sessions = state.acp_sessions.lock().await;
        acp_sessions.retain(|k, info| !(info.agent_id == agent_id || k == &format!("temp:{}", agent_id)));
        drop(acp_sessions);
    }

    let process_running = {
        let processes = state.agent_processes.lock().await;
        processes.contains_key(&agent_id)
    };

    // If agent is already running, not restarting, and we have cached models — return early
    if process_running && !needs_restart && !force_refresh {
        if let Some(ref models_json) = agent_config.available_models_json {
            if let Ok(model_ids) = serde_json::from_str::<Vec<String>>(models_json) {
                if !model_ids.is_empty() {
                    log::info!(
                        "ensure_agent_ready: agent {} already running with {} cached models, skipping session/new",
                        agent_id, model_ids.len()
                    );
                    let models: Vec<crate::acp::client::AgentModel> = model_ids.iter().map(|id| {
                        crate::acp::client::AgentModel {
                            model_id: id.clone(),
                            name: id.rsplit('/').next().unwrap_or(id).to_string(),
                            description: None,
                        }
                    }).collect();

                    return Ok(EnsureAgentReadyResult {
                        status: "Running".into(),
                        models,
                    });
                }
            }
        }
    }

    if !process_running {
        // --- Resolve command + args via provisioner ---
        let base_args: Vec<String> = agent_config
            .acp_args_json
            .as_ref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();

        let resolved = provisioner::resolve_agent_command(&acp_command, &base_args).await?;

        log::info!(
            "Provisioner resolved: command={}, args={:?}, agent_type={}",
            resolved.command, resolved.args, resolved.agent_type
        );

        // Build extra environment variables
        // 1. Registry env vars (e.g., disable auto-update for certain agents)
        let mut extra_env = discovery::get_agent_env_for_command(&resolved.agent_type).await;
        // 2. Env vars from discovered agent config (agents.json custom env)
        {
            let discovered = state.discovered_agents.lock().await;
            let cmd_basename = std::path::Path::new(&acp_command)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&acp_command);
            if let Some(matched) = discovered.iter().find(|d| {
                let d_basename = std::path::Path::new(&d.command)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&d.command);
                d_basename == cmd_basename
            }) {
                if let Ok(config_env) = serde_json::from_str::<std::collections::HashMap<String, String>>(&matched.env_json) {
                    extra_env.extend(config_env);
                }
            }
        }

        log::info!("Extra env for agent: {:?}", extra_env);

        // --- Spawn ---
        let mut process = manager::spawn_agent_process(
            &agent_id,
            &resolved.command,
            &resolved.args,
            &extra_env,
            &resolved.agent_type,
        ).await?;

        // Record the on-disk CLI version so we can detect upgrades later
        process.cli_version = builtin::get_cli_version().unwrap_or_default();

        let stdin_handle = process.stdin.clone();
        log::info!("Agent process spawned: {} (cli_version={})", agent_id, process.cli_version);

        {
            let mut processes = state.agent_processes.lock().await;
            processes.insert(agent_id.clone(), process);
        }
        {
            let mut stdins = state.agent_stdins.lock().await;
            stdins.insert(agent_id.clone(), stdin_handle);
        }

        // --- Initialize (with retry for npx agents that may still be starting) ---
        let is_npx = provisioner::is_npx_command(&resolved.command);
        let max_retries = if is_npx { 3 } else { 1 };
        let mut last_err = None;

        for attempt in 0..max_retries {
            if attempt > 0 {
                log::info!("Retrying ACP initialize (attempt {}/{})", attempt + 1, max_retries);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }

            let mut processes = state.agent_processes.lock().await;
            if let Some(process) = processes.get_mut(&agent_id) {
                // Check if process died
                match process.child.try_wait() {
                    Ok(Some(exit_status)) => {
                        let stderr_output = {
                            let lines = process.stderr_lines.lock().await;
                            if lines.is_empty() {
                                "(no stderr output)".to_string()
                            } else {
                                lines.join("\n")
                            }
                        };
                        let msg = format!(
                            "Agent process exited with {}\nStderr:\n{}",
                            exit_status, stderr_output
                        );
                        log::error!("{}", msg);
                        drop(processes);
                        // Clean up dead process
                        let mut procs = state.agent_processes.lock().await;
                        procs.remove(&agent_id);
                        drop(procs);
                        let mut stdins = state.agent_stdins.lock().await;
                        stdins.remove(&agent_id);
                        return Err(AppError::Acp(msg));
                    }
                    _ => {}
                }

                match client::initialize_agent(process).await {
                    Ok(init_response) => {
                        process.status = manager::AgentProcessStatus::Running;
                        // Extract auth methods for later use
                        let methods = client::extract_auth_methods(&init_response);
                        if !methods.is_empty() {
                            log::info!("Agent reported {} auth methods: {:?}",
                                methods.len(),
                                methods.iter().map(|m| &m.id).collect::<Vec<_>>()
                            );
                        }
                        log::info!(
                            "Agent initialized: {:?}",
                            serde_json::to_string(&init_response).unwrap_or_default()
                        );
                        last_err = None;
                        break;
                    }
                    Err(e) => {
                        log::warn!("Initialize attempt {} failed: {}", attempt + 1, e);
                        last_err = Some(e);
                    }
                }
            }
        }

        if let Some(e) = last_err {
            // Capture stderr for a more informative error message
            let stderr_output = {
                let processes = state.agent_processes.lock().await;
                if let Some(process) = processes.get(&agent_id) {
                    let lines = process.stderr_lines.lock().await;
                    if lines.is_empty() {
                        String::new()
                    } else {
                        format!("\nAgent stderr:\n{}", lines.join("\n"))
                    }
                } else {
                    String::new()
                }
            };
            // Clean up failed process
            let mut processes = state.agent_processes.lock().await;
            if let Some(mut proc) = processes.remove(&agent_id) {
                let _ = manager::stop_agent_process(&mut proc).await;
            }
            drop(processes);
            let mut stdins = state.agent_stdins.lock().await;
            stdins.remove(&agent_id);
            return Err(AppError::Acp(format!("{}{}", e, stderr_output)));
        }

        // Notify frontend that agent is running
        let _ = app.emit("acp:agent_started", &serde_json::json!({
            "agent_id": agent_id,
            "status": "Running"
        }));
    }

    // --- Fetch models via session/new (stored as temp session) ---
    log::info!("Fetching models for agent {}", agent_id);

    let cwd = settings_commands::resolve_working_directory(state.inner());

    let mut processes = state.agent_processes.lock().await;
    let process = processes
        .get_mut(&agent_id)
        .ok_or_else(|| AppError::AgentNotRunning(agent_id.clone()))?;

    let session_result = client::create_session(process, &cwd).await;

    // If session/new failed, try auth flow before giving up
    let (session_id, models) = match session_result {
        Ok(result) => result,
        Err(e) => {
            let err_msg = e.to_string().to_lowercase();
            let is_auth_err = err_msg.contains("auth")
                || err_msg.contains("login")
                || err_msg.contains("authenticate")
                || err_msg.contains("unauthorized")
                || err_msg.contains("api_key")
                || err_msg.contains("api key");

            if !is_auth_err {
                drop(processes);
                return Err(e);
            }

            log::info!("session/new failed with auth error, attempting auth/start flow");

            // Re-read init response authMethods from a fresh initialize
            // (the process is already initialized, so extract from stored info)
            // Try auth/start with the first available OAuth method
            let auth_methods = vec!["oauth-personal", "oauth", "login"];
            let mut auth_url: Option<String> = None;

            for method_id in &auth_methods {
                match client::start_auth(process, method_id).await {
                    Ok(auth_response) => {
                        if let Some(url) = client::extract_auth_url(&auth_response) {
                            log::info!("Auth URL obtained: {}", url);
                            auth_url = Some(url);
                            break;
                        }
                        log::info!("auth/start succeeded but no URL found in response");
                    }
                    Err(auth_err) => {
                        log::debug!("auth/start with method '{}' failed: {}", method_id, auth_err);
                    }
                }
            }

            drop(processes);

            if let Some(url) = auth_url {
                return Err(AppError::Acp(format!(
                    "Authentication required. Please login: {}",
                    url
                )));
            } else {
                return Err(AppError::Acp(format!(
                    "Authentication required: {}",
                    e
                )));
            }
        }
    };

    log::info!(
        "ensure_agent_ready: agent {} returned {} models, temp ACP session: {}",
        agent_id, models.len(), session_id
    );

    drop(processes);

    // Store as temp session for later reuse
    let temp_session_key = format!("temp:{}", agent_id);
    let mut acp_sessions = state.acp_sessions.lock().await;
    acp_sessions.insert(
        temp_session_key.clone(),
        crate::state::AcpSessionInfo::new(
            temp_session_key,
            agent_id.clone(),
            session_id,
        ),
    );
    drop(acp_sessions);

    // Emit models to frontend
    let command = agent_config.acp_command.unwrap_or_default();
    let model_ids: Vec<String> = models.iter().map(|m| m.model_id.clone()).collect();
    let _ = app.emit("acp:models", &serde_json::json!({
        "command": command,
        "models": model_ids
    }));

    log::info!("ensure_agent_ready: agent {} is fully ready", agent_id);

    Ok(EnsureAgentReadyResult {
        status: "Running".into(),
        models,
    })
}

// ---------------------------------------------------------------------------
// Registry install / uninstall commands
// ---------------------------------------------------------------------------

/// Internal helper: deploy built-in agent then run discovery.
/// Used by `install_registry_agent` and `uninstall_registry_agent` which don't
/// go through the tauri `discover_agents` command.
async fn discover_agents_inner() -> AppResult<Vec<DiscoveredAgent>> {
    builtin::ensure_builtin_deployed().await;
    discovery::discover_agents().await
}

/// Install a registry agent by its registry ID.
/// For binary-distributed agents: downloads and extracts the binary.
/// For npx-distributed agents: runs `npx -y <package> --version` to pre-cache.
/// After install, re-runs discovery so the frontend gets fresh availability data.
#[tauri::command(rename_all = "camelCase")]
pub async fn install_registry_agent(
    registry_id: String,
) -> AppResult<Vec<DiscoveredAgent>> {
    use crate::acp::discovery::{
        Distribution, fetch_registry, get_current_platform,
    };

    log::info!("install_registry_agent: {}", registry_id);

    // Built-in agents use their own deployment path (deps pinned to `latest`)
    if builtin::is_builtin_agent(&registry_id) {
        log::info!("install_registry_agent: '{}' is built-in, using builtin deploy", registry_id);
        builtin::ensure_builtin_deployed().await;
        return discover_agents_inner().await;
    }

    let registry = fetch_registry().await?;
    let entry = registry
        .agents
        .iter()
        .find(|e| e.id == registry_id)
        .ok_or_else(|| {
            AppError::Internal(format!("Registry entry '{}' not found", registry_id))
        })?
        .clone();

    match &entry.distribution {
        Distribution::Binary(platforms) => {
            let platform = get_current_platform();
            let target = platforms.get(platform).ok_or_else(|| {
                AppError::Internal(format!(
                    "No binary for platform '{}' in agent '{}'",
                    platform, registry_id
                ))
            })?;

            provisioner::download_and_extract_binary(target, &entry.id)
                .await
                .map_err(|e| AppError::Internal(format!("Download failed: {e}")))?;

            log::info!("install_registry_agent: binary installed for {}", registry_id);
        }
        Distribution::Npx(npx) => {
            // Install npx package locally with npm overrides for dependency fixes
            let adapter_dir = discovery::get_adapters_dir().join(&registry_id);
            tokio::fs::create_dir_all(&adapter_dir)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create adapter dir: {e}")))?;

            // Write package.json with overrides to fix outdated transitive dependencies
            // Use ^version range to allow compatible minor/patch updates via npm update
            let raw_version = npx.package.rsplit('@').next().unwrap_or("latest");
            let version_range = if raw_version == "latest" {
                "latest".to_string()
            } else {
                format!("^{}", raw_version)
            };
            let package_json = serde_json::json!({
                "name": format!("{}-local", registry_id),
                "private": true,
                "dependencies": {
                    npx.package.split('@').take(
                        if npx.package.starts_with('@') { 2 } else { 1 }
                    ).collect::<Vec<_>>().join("@"): version_range
                }
            });
            let pkg_path = adapter_dir.join("package.json");
            tokio::fs::write(&pkg_path, serde_json::to_string_pretty(&package_json).unwrap_or_default())
                .await
                .map_err(|e| AppError::Internal(format!("Failed to write package.json: {e}")))?;

            // Run npm install in the adapter directory
            let enriched_path = discovery::get_enriched_path();
            let npm_path = which_in_path("npm", &enriched_path).ok_or_else(|| {
                AppError::Internal("npm not found on PATH".to_string())
            })?;

            log::info!(
                "install_registry_agent: npm install in {:?} for {}",
                adapter_dir, npx.package
            );
            let output = tokio::process::Command::new(&npm_path)
                .arg("install")
                .current_dir(&adapter_dir)
                .env("PATH", &enriched_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
                .map_err(|e| AppError::Internal(format!("npm install spawn error: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::warn!("npm install failed: {}", stderr);
                return Err(AppError::Internal(format!(
                    "npm install failed for {}: {}", registry_id, stderr
                )));
            }
            log::info!("install_registry_agent: npm package installed for {}", registry_id);

            // Ensure the embedded claude-agent-sdk is up-to-date.
            // The adapter may pin an older SDK version whose bundled cli.js
            // is rejected by the remote API. Force-install latest SDK.
            upgrade_embedded_sdk(&adapter_dir, &enriched_path, &npm_path).await;
        }
    }

    // Record in installed manifest
    discovery::mark_installed(&registry_id);

    // Re-discover so the frontend gets updated availability
    discover_agents_inner().await
}

/// Uninstall a registry agent by its registry ID.
/// Removes cached binary from `~/.iaagenthub/adapters/<id>/`.
/// For npx agents there's nothing to remove on disk – this is a no-op but still
/// re-runs discovery to refresh state.
#[tauri::command(rename_all = "camelCase")]
pub async fn uninstall_registry_agent(
    registry_id: String,
) -> AppResult<Vec<DiscoveredAgent>> {
    log::info!("uninstall_registry_agent: {}", registry_id);

    // Remove cached binary if exists
    let adapters_dir = discovery::get_adapters_dir().join(&registry_id);
    if adapters_dir.exists() {
        tokio::fs::remove_dir_all(&adapters_dir)
            .await
            .map_err(|e| {
                AppError::Internal(format!("Failed to remove adapter dir: {e}"))
            })?;
        log::info!("Removed adapter dir: {:?}", adapters_dir);
    }

    // Remove from installed manifest
    discovery::mark_uninstalled(&registry_id);

    // Re-discover
    discover_agents_inner().await
}

/// Thin helper – resolve a command in a given PATH (used only in this module).
fn which_in_path(cmd: &str, path_env: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    let lookup = "where.exe";
    #[cfg(not(target_os = "windows"))]
    let lookup = "which";

    let output = std::process::Command::new(lookup)
        .arg(cmd)
        .env("PATH", path_env)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if output.status.success() {
        let s = String::from_utf8_lossy(&output.stdout);
        let first = s.lines().next().unwrap_or("").trim().to_string();
        if first.is_empty() { None } else { Some(first) }
    } else {
        None
    }
}

/// Upgrade the embedded `@anthropic-ai/claude-agent-sdk` inside a local adapter
/// to the latest version. The adapter may pin an older SDK whose bundled `cli.js`
/// (a complete Claude Code bundle) is rejected by the remote API when it mandates
/// a newer client version. This is non-fatal — if it fails the adapter still works
/// with whatever SDK version was installed by `npm install`.
async fn upgrade_embedded_sdk(
    adapter_dir: &std::path::Path,
    enriched_path: &str,
    npm_path: &str,
) {
    log::info!("upgrade_embedded_sdk: ensuring latest claude-agent-sdk in {:?}", adapter_dir);
    let output = tokio::process::Command::new(npm_path)
        .args(["install", "@anthropic-ai/claude-agent-sdk@latest"])
        .current_dir(adapter_dir)
        .env("PATH", enriched_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            // Log the version we ended up with
            let sdk_pkg = adapter_dir
                .join("node_modules/@anthropic-ai/claude-agent-sdk/package.json");
            if let Ok(content) = std::fs::read_to_string(&sdk_pkg) {
                if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                    let ver = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("?");
                    log::info!("upgrade_embedded_sdk: claude-agent-sdk now at {}", ver);
                }
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            log::warn!("upgrade_embedded_sdk: npm install failed (non-fatal): {}", stderr.trim());
        }
        Err(e) => {
            log::warn!("upgrade_embedded_sdk: spawn failed (non-fatal): {}", e);
        }
    }
}