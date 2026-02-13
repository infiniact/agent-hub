use tauri::Emitter;

use crate::db::agent_repo;
use crate::db::message_repo;
use crate::db::session_repo;
use crate::commands::settings_commands;
use crate::error::{AppError, AppResult};
use crate::models::agent::AgentConfig;
use crate::models::message::ChatMessage;
use crate::models::session::Session;
use crate::state::AppState;

#[tauri::command(rename_all = "camelCase")]
pub async fn send_prompt(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    session_id: String,
    content: String,
) -> AppResult<ChatMessage> {
    log::info!("send_prompt called: session_id={}, content_len={}", session_id, content.len());

    // Save user message to DB
    let user_msg = ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.clone(),
        role: "User".into(),
        content_json: serde_json::to_string(&[serde_json::json!({
            "type": "text",
            "text": content
        })])
        .unwrap_or_else(|_| "[]".into()),
        tool_calls_json: None,
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    let state_clone = state.inner().clone();
    let msg_clone = user_msg.clone();
    tokio::task::spawn_blocking(move || message_repo::save_message(&state_clone, &msg_clone))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;

    // Get the session to find agent_id
    let session: Session = {
        let state_clone = state.inner().clone();
        let session_id_clone = session_id.clone();
        tokio::task::spawn_blocking(move || session_repo::get_session(&state_clone, &session_id_clone))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };
    log::info!("Session found: agent_id={}", session.agent_id);
    let agent_id = session.agent_id.clone();

    // Get agent config to check for ACP command
    let agent_config: AgentConfig = {
        let state_clone = state.inner().clone();
        let agent_id_clone = agent_id.clone();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &agent_id_clone))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };
    log::info!("Agent config found: acp_command={:?}", agent_config.acp_command);

    // Ensure agent process is running
    let process_running = {
        let processes = state.agent_processes.lock().await;
        processes.contains_key(&agent_id)
    };

    if !process_running {
        // Auto-start the agent
        log::info!("Agent not running, attempting to auto-start...");

        let mut acp_command = agent_config.acp_command.clone().ok_or_else(|| {
            let msg = format!("Agent {} has no ACP command configured", agent_id);
            log::warn!("{}", msg);
            app.emit("acp:error", &serde_json::json!({
                "error": "NoAcpCommand",
                "message": msg
            })).ok();
            AppError::Internal(msg)
        })?;

        let mut args: Vec<String> = if let Some(args_json) = agent_config.acp_args_json.as_ref() {
            serde_json::from_str(args_json).unwrap_or_default()
        } else {
            vec![]
        };

        // Sync with discovered agents: use the latest command + args from discovery
        // This ensures DB entries always get the correct ACP flags
        {
            let discovered = state.discovered_agents.lock().await;
            // Match by command name (basename comparison)
            let cmd_basename = std::path::Path::new(&acp_command)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&acp_command);

            if let Some(matched) = discovered.iter().find(|d| {
                d.available && {
                    let d_basename = std::path::Path::new(&d.command)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&d.command);
                    d_basename == cmd_basename || d.name == agent_config.name
                }
            }) {
                let discovered_args: Vec<String> =
                    serde_json::from_str(&matched.args_json).unwrap_or_default();
                if acp_command != matched.command || args != discovered_args {
                    log::info!(
                        "Syncing agent args from discovery: {} {:?} -> {} {:?}",
                        acp_command, args, matched.command, discovered_args
                    );
                    acp_command = matched.command.clone();
                    args = discovered_args;
                }
            }
        }

        // Auto-upgrade: if using npx/pnpx, switch to built-in adapter
        if acp_command.contains("npx") || acp_command.contains("pnpx") {
            let project_root = crate::acp::discovery::get_project_root();
            let adapter_path = project_root
                .join("node_modules")
                .join("@zed-industries")
                .join("claude-code-acp")
                .join("dist")
                .join("index.js");

            if adapter_path.exists() {
                let enriched_path = crate::acp::discovery::get_enriched_path();
                // Find node in enriched PATH
                if let Some(node_path) = std::env::split_paths(&enriched_path)
                    .map(|p| p.join("node"))
                    .find(|p| p.exists())
                {
                    log::info!("Auto-upgrading from npx/pnpx to built-in adapter");
                    acp_command = node_path.to_string_lossy().to_string();
                    args = vec![adapter_path.to_string_lossy().to_string()];
                }
            }
        }

        log::info!("Spawning agent: command={}, args={:?}", acp_command, args);

        // Build extra environment variables from dynamic registry
        let extra_env = crate::acp::discovery::get_agent_env_for_command(&acp_command).await;

        // Spawn the agent process
        let process = crate::acp::manager::spawn_agent_process(&agent_id, &acp_command, &args, &extra_env, &acp_command).await?;
        let stdin_handle = process.stdin.clone();
        log::info!("Agent process spawned: {}", agent_id);

        // Add to state (both process and stdin)
        {
            let mut processes = state.agent_processes.lock().await;
            processes.insert(agent_id.clone(), process);
        }
        {
            let mut stdins = state.agent_stdins.lock().await;
            stdins.insert(agent_id.clone(), stdin_handle);
        }

        // Initialize the agent
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(&agent_id) {
            log::info!("Initializing agent...");
            let init_response = crate::acp::client::initialize_agent(process).await?;
            log::info!("Agent initialized: {:?}", serde_json::to_string(&init_response).unwrap_or_default());
        }
        drop(processes);

        // Notify frontend that agent is running
        let _ = app.emit("acp:agent_started", &serde_json::json!({
            "agent_id": agent_id,
            "status": "Running"
        }));
    }

    // Check if there's an active ACP session
    let acp_session_info = {
        let acp_sessions = state.acp_sessions.lock().await;
        acp_sessions.get(&session_id).cloned()
    };

    let acp_session_id = if let Some(info) = acp_session_info {
        // Session exists in our state - check if it's usable
        if info.is_usable() {
            log::info!("Found existing ACP session: {} (state: {})", info.acp_session_id, info.state);
            // Mark as active on use
            let mut acp_sessions = state.acp_sessions.lock().await;
            if let Some(s) = acp_sessions.get_mut(&session_id) {
                s.mark_active();
            }
            info.acp_session_id.clone()
        } else {
            log::info!("Existing session state is {}, creating new session", info.state);
            // Session is ended, remove and create new
            let mut acp_sessions = state.acp_sessions.lock().await;
            acp_sessions.remove(&session_id);
            drop(acp_sessions);
            create_new_acp_session(
                &app,
                &state,
                &agent_id,
                &agent_config,
                &session_id,
            ).await?
        }
    } else {
        // No ACP session for this session_id — check for a temp session from get_agent_models
        let temp_key = format!("temp:{}", agent_id);
        let temp_acp_id = {
            let acp_sessions = state.acp_sessions.lock().await;
            acp_sessions.get(&temp_key).map(|info| info.acp_session_id.clone())
        };

        if let Some(temp_id) = temp_acp_id {
            log::info!("Reusing temporary ACP session from get_agent_models: {}", temp_id);

            // Move the temp session to the actual session
            let mut acp_sessions = state.acp_sessions.lock().await;
            if let Some(mut info) = acp_sessions.remove(&temp_key) {
                info.session_id = session_id.clone();
                info.mark_active();
                acp_sessions.insert(session_id.clone(), info);
            }
            drop(acp_sessions);

            // Update session in DB
            let state_clone = state.inner().clone();
            let session_id_clone = session_id.clone();
            let temp_id_clone = temp_id.clone();
            tokio::task::spawn_blocking(move || {
                session_repo::update_session_acp_id(&state_clone, &session_id_clone, &temp_id_clone)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            temp_id
        } else {
            // No temp session either, create a brand new one
            log::info!("No ACP session found, creating new session");
            create_new_acp_session(
                &app,
                &state,
                &agent_id,
                &agent_config,
                &session_id,
            ).await?
        }
    };

    // Send prompt to agent
    let mut processes = state.agent_processes.lock().await;
    if let Some(process) = processes.get_mut(&agent_id) {
        let request_id = chrono::Utc::now().timestamp();
        log::info!("Sending prompt to agent: acp_session_id={}, request_id={}", acp_session_id, request_id);
        crate::acp::client::send_prompt(process, &acp_session_id, &content, request_id)
            .await?;
    }
    drop(processes);

    // Spawn a task to read streaming responses
    let app_clone = app.clone();
    let state_clone = state.inner().clone();
    let session_id_clone = session_id.clone();
    let agent_id_clone = agent_id.clone();

    log::info!("Spawning handle_agent_responses task");
    tokio::spawn(async move {
        log::info!("handle_agent_responses task started");
        handle_agent_responses(app_clone, state_clone, agent_id_clone, session_id_clone).await;
        log::info!("handle_agent_responses task completed");
    });

    log::info!("send_prompt completed successfully");
    Ok(user_msg)
}

async fn handle_agent_responses(
    app: tauri::AppHandle,
    state: AppState,
    agent_id: String,
    session_id: String,
) {
    log::info!("handle_agent_responses started: agent_id={}, session_id={}", agent_id, session_id);

    loop {
        let msg = {
            let mut processes = state.agent_processes.lock().await;
            if let Some(process) = processes.get_mut(&agent_id) {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(300),
                    crate::acp::transport::receive_message(process),
                )
                .await
                {
                    Ok(Ok(msg)) => {
                        log::debug!("Received message from agent: {}", serde_json::to_string(&msg).unwrap_or_else(|_| "unknown".into()));
                        Some(msg)
                    }
                    Ok(Err(e)) => {
                        log::error!("Error receiving message: {}", e);
                        None
                    }
                    Err(_) => {
                        log::warn!("Timeout receiving message from agent");
                        None
                    }
                }
            } else {
                log::warn!("Agent process not found: {}", agent_id);
                None
            }
        };

        match msg {
            Some(msg) => {
                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                log::debug!("Processing message with method: '{}'", method);

                match method {
                    "session/update" => {
                        // ACP sends all updates via session/update with nested update type
                        let update_type = msg
                            .get("params")
                            .and_then(|p| p.get("update"))
                            .and_then(|u| u.get("sessionUpdate"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");

                        log::debug!("session/update type: '{}'", update_type);

                        match update_type {
                            "agent_message_chunk" | "user_message_chunk" => {
                                let _ = app.emit("acp:agent_message_chunk", &msg);
                            }
                            "agent_thought_chunk" => {
                                let _ = app.emit("acp:agent_thought_chunk", &msg);
                            }
                            "tool_call" => {
                                let _ = app.emit("acp:tool_call", &msg);
                            }
                            "tool_call_update" => {
                                let _ = app.emit("acp:tool_call_update", &msg);
                            }
                            "plan" => {
                                let _ = app.emit("acp:plan", &msg);
                            }
                            "current_mode_update" => {
                                let _ = app.emit("acp:mode_update", &msg);
                            }
                            "available_commands_update" => {
                                let _ = app.emit("acp:commands_update", &msg);
                            }
                            _ => {
                                log::debug!("Unhandled session/update type: '{}'", update_type);
                                let _ = app.emit("acp:agent_message_chunk", &msg);
                            }
                        }
                    }
                    "session/requestPermission" | "session/request_permission" => {
                        log::info!("Permission request from agent: {:?}", serde_json::to_string(&msg).unwrap_or_default());
                        // Emit permission request to frontend - user will decide
                        let _ = app.emit("acp:permission_request", &msg);
                        // Don't auto-approve - wait for user response via respond_permission command
                    }
                    "" => {
                        // No method field - this is a JSON-RPC response to one of our requests
                        if let Some(result) = msg.get("result") {
                            log::info!("Agent response completed, result: {:?}", result);
                            let agent_msg = ChatMessage {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: "Agent".into(),
                                content_json: serde_json::to_string(result)
                                    .unwrap_or_else(|_| "[]".into()),
                                tool_calls_json: None,
                                created_at: chrono::Utc::now()
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string(),
                            };
                            let state_clone = state.clone();
                            let msg_clone = agent_msg.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                message_repo::save_message(&state_clone, &msg_clone)
                            })
                            .await;
                            let _ = app.emit("acp:message_complete", &agent_msg);
                            log::info!("handle_agent_responses ending (result received)");
                            break;
                        } else if let Some(error) = msg.get("error") {
                            log::error!("Agent returned error: {:?}", error);
                            let _ = app.emit("acp:error", &error);
                            log::info!("handle_agent_responses ending (error received)");
                            break;
                        }
                    }
                    _ => {
                        log::debug!("Unhandled method: '{}'", method);
                    }
                }
            }
            None => {
                log::info!("No more messages, ending handle_agent_responses");
                break;
            }
        }
    }
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_prompt(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> AppResult<()> {
    let acp_sessions = state.acp_sessions.lock().await;
    if let Some(acp_info) = acp_sessions.get(&session_id) {
        let agent_id = acp_info.agent_id.clone();
        let acp_session_id = acp_info.acp_session_id.clone();
        drop(acp_sessions);

        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(&agent_id) {
            crate::acp::client::cancel_prompt(process, &acp_session_id).await?;
        }
    }
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_messages(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> AppResult<Vec<ChatMessage>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || message_repo::get_messages(&state, &session_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Respond to a permission request from the agent
#[tauri::command(rename_all = "camelCase")]
pub async fn respond_permission(
    state: tauri::State<'_, AppState>,
    agent_id: String,
    request_id: serde_json::Value,
    option_id: String,
    user_message: Option<String>,
) -> AppResult<()> {
    log::info!("Responding to permission request: agent_id={}, option_id={}, user_message={:?}", agent_id, option_id, user_message);

    // Get the stdin handle for this agent
    let stdins = state.agent_stdins.lock().await;
    let stdin = stdins.get(&agent_id)
        .ok_or_else(|| AppError::Internal(format!("Agent stdin not found: {}", agent_id)))?;

    // Build the response
    let mut outcome = serde_json::json!({
        "outcome": "selected",
        "optionId": option_id
    });

    // Add user message if provided
    if let Some(msg) = user_message {
        if let Some(obj) = outcome.as_object_mut() {
            obj.insert("userMessage".into(), serde_json::json!(msg));
        }
    }

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {
            "outcome": outcome
        }
    });

    // Send response to agent
    use tokio::io::AsyncWriteExt;
    let json = serde_json::to_string(&response).unwrap_or_default();
    log::debug!("Sending permission response: {}", json);

    let mut stdin_writer = stdin.lock().await;
    stdin_writer.write_all(json.as_bytes()).await
        .map_err(|e| AppError::Acp(format!("Failed to write permission response: {e}")))?;
    stdin_writer.write_all(b"\n").await
        .map_err(|e| AppError::Acp(format!("Failed to write newline: {e}")))?;
    stdin_writer.flush().await
        .map_err(|e| AppError::Acp(format!("Failed to flush stdin: {e}")))?;

    log::info!("Permission response sent successfully");
    Ok(())
}

/// Helper function to create a new ACP session following the ACP protocol.
/// This creates a session/new request and tracks the session in state.
async fn create_new_acp_session(
    app: &tauri::AppHandle,
    state: &AppState,
    agent_id: &str,
    agent_config: &AgentConfig,
    session_id: &str,
) -> AppResult<String> {
    log::info!("Creating new ACP session for agent {}", agent_id);

    let mut processes = state.agent_processes.lock().await;
    let process = processes.get_mut(agent_id)
        .ok_or_else(|| AppError::Internal(format!("Agent {} process not found", agent_id)))?;

    let cwd = settings_commands::resolve_working_directory(state);

    log::info!("Calling create_session for agent {}", agent_id);
    let (acp_id, models) = crate::acp::client::create_session(process, &cwd).await?;
    log::info!("Created ACP session: {} with {} models", acp_id, models.len());

    drop(processes);

    // Emit models to frontend for updating the discovered agents list
    let command = agent_config.acp_command.clone().unwrap_or_default();
    let model_ids: Vec<String> = models.iter().map(|m| m.model_id.clone()).collect();
    let _ = app.emit("acp:models", &serde_json::json!({
        "command": command,
        "models": model_ids
    }));

    // Update session in DB - clone the session_id for the spawned task
    let session_id_owned = session_id.to_string();
    let state_clone = state.clone();
    let acp_id_clone = acp_id.clone();
    tokio::task::spawn_blocking(move || {
        session_repo::update_session_acp_id(&state_clone, &session_id_owned, &acp_id_clone)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // Track in state with proper AcpSessionInfo
    let mut acp_sessions = state.acp_sessions.lock().await;
    acp_sessions.insert(
        session_id.to_string(),
        crate::state::AcpSessionInfo::new(
            session_id.to_string(),
            agent_id.to_string(),
            acp_id.clone(),
        ),
    );

    Ok(acp_id)
}
