use std::collections::HashMap;
use tauri::Emitter;

use crate::acp::{client, manager, transport};
use crate::db::{agent_repo, task_run_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::AgentConfig;
use crate::models::task_run::{TaskPlan, PlannedAssignment};
use crate::state::AppState;
use crate::db::migrations::{get_output_dir};

/// Run a complete orchestration flow:
/// 1. Validate control hub exists
/// 2. Create TaskRun record
/// 3. Ask control hub to plan
/// 4. Execute assignments sequentially
/// 5. Finalize and write summary
pub async fn run_orchestration(
    app: tauri::AppHandle,
    state: AppState,
    task_run_id: String,
    user_prompt: String,
) {
    let result = run_orchestration_inner(&app, &state, &task_run_id, &user_prompt).await;

    if let Err(e) = &result {
        log::error!("Orchestration failed: {}", e);
        let _ = app.emit("orchestration:error", &serde_json::json!({
            "taskRunId": task_run_id,
            "error": e.to_string(),
        }));
        // Update status to failed
        let state_clone = state.clone();
        let id_clone = task_run_id.clone();
        let _ = tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id_clone, "failed")
        }).await;
    }
}

async fn run_orchestration_inner(
    app: &tauri::AppHandle,
    state: &AppState,
    task_run_id: &str,
    user_prompt: &str,
) -> AppResult<()> {
    let start_time = std::time::Instant::now();

    // Check cancellation
    if is_cancelled(state, task_run_id).await {
        return Ok(());
    }

    // 1. Get the control hub agent
    let hub_agent: AgentConfig = {
        let state_clone = state.clone();
        tokio::task::spawn_blocking(move || agent_repo::get_control_hub(&state_clone))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
            .ok_or_else(|| AppError::Internal("No Control Hub agent configured".into()))?
    };

    // 2. Update status to analyzing
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id, "analyzing")
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    let _ = app.emit("orchestration:started", &serde_json::json!({
        "taskRunId": task_run_id,
        "status": "analyzing",
    }));

    // 3. Build agent catalog
    let all_agents: Vec<AgentConfig> = {
        let state_clone = state.clone();
        tokio::task::spawn_blocking(move || agent_repo::list_agents(&state_clone))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    let catalog = build_agent_catalog(&all_agents);

    // 4. Ensure hub agent process is running and get a plan
    ensure_agent_running(app, state, &hub_agent).await?;

    let plan_prompt = format!(
        r#"You are the orchestrator control hub. Analyze this task and assign it to the most appropriate agents.

Available agents:
{catalog}

User request: {user_prompt}

Respond with a JSON object (and nothing else) in this exact format:
{{"analysis": "your analysis of the task", "assignments": [{{"agent_id": "uuid", "task_description": "what this agent should do", "sequence_order": 0, "depends_on": []}}]}}

Rules:
- Only use agent IDs from the available agents list above
- Set sequence_order starting from 0 for parallel tasks, increment for sequential ones
- depends_on should list agent_ids whose output is needed as input
- If only one agent is needed, still return the assignments array with one entry"#
    );

    let plan_response = send_prompt_to_agent(app, state, &hub_agent.id, &plan_prompt).await?;

    if is_cancelled(state, task_run_id).await {
        return Ok(());
    }

    // Parse the plan
    let plan = parse_task_plan(&plan_response)?;

    // Store plan in DB
    {
        let plan_json = serde_json::to_string(&plan)
            .map_err(|e| AppError::Internal(format!("Failed to serialize plan: {e}")))?;
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_plan(&state_clone, &id, &plan_json)
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    let _ = app.emit("orchestration:plan_ready", &serde_json::json!({
        "taskRunId": task_run_id,
        "plan": &plan,
    }));

    // 5. Update status to running
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id, "running")
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    // 6. Execute assignments in sequence order
    let mut agent_outputs: HashMap<String, String> = HashMap::new();
    let total_tokens_in: i64 = 0;
    let total_tokens_out: i64 = 0;

    // Group assignments by sequence_order
    let mut sequence_groups: HashMap<i64, Vec<&PlannedAssignment>> = HashMap::new();
    for assignment in &plan.assignments {
        sequence_groups
            .entry(assignment.sequence_order)
            .or_default()
            .push(assignment);
    }

    let mut sorted_orders: Vec<i64> = sequence_groups.keys().copied().collect();
    sorted_orders.sort();

    for order in sorted_orders {
        let group = &sequence_groups[&order];

        for planned in group {
            if is_cancelled(state, task_run_id).await {
                return Ok(());
            }

            // Look up agent name
            let agent_name = all_agents
                .iter()
                .find(|a| a.id == planned.agent_id)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| "Unknown".into());

            let agent_model = all_agents
                .iter()
                .find(|a| a.id == planned.agent_id)
                .map(|a| a.model.clone())
                .unwrap_or_default();

            // Build input: task description + outputs from dependencies
            let mut input_parts = vec![planned.task_description.clone()];
            for dep_id in &planned.depends_on {
                if let Some(output) = agent_outputs.get(dep_id) {
                    let dep_name = all_agents
                        .iter()
                        .find(|a| a.id == *dep_id)
                        .map(|a| a.name.clone())
                        .unwrap_or_else(|| "Previous agent".into());
                    input_parts.push(format!("\n--- Output from {dep_name} ---\n{output}"));
                }
            }
            let input_text = input_parts.join("\n");

            // Create assignment record
            let assignment_id = uuid::Uuid::new_v4().to_string();
            {
                let state_clone = state.clone();
                let aid = assignment_id.clone();
                let trid = task_run_id.to_string();
                let agid = planned.agent_id.clone();
                let aname = agent_name.clone();
                let seq = planned.sequence_order;
                let inp = input_text.clone();
                tokio::task::spawn_blocking(move || {
                    task_run_repo::create_task_assignment(
                        &state_clone, &aid, &trid, &agid, &aname, seq, &inp,
                    )
                })
                .await
                .map_err(|e| AppError::Internal(e.to_string()))??;
            }

            // Mark as running
            {
                let state_clone = state.clone();
                let aid = assignment_id.clone();
                tokio::task::spawn_blocking(move || {
                    task_run_repo::update_task_assignment(
                        &state_clone, &aid, "running", None, None, 0, 0, 0, None,
                    )
                })
                .await
                .map_err(|e| AppError::Internal(e.to_string()))??;
            }

            let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                "taskRunId": task_run_id,
                "assignmentId": assignment_id,
                "agentId": planned.agent_id,
                "agentName": agent_name,
                "model": agent_model,
                "sequenceOrder": planned.sequence_order,
            }));

            // Ensure agent is running
            let agent_config = all_agents
                .iter()
                .find(|a| a.id == planned.agent_id)
                .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", planned.agent_id)))?;

            let assign_start = std::time::Instant::now();

            match execute_agent_assignment(app, state, agent_config, &input_text, task_run_id).await {
                Ok(output) => {
                    let duration_ms = assign_start.elapsed().as_millis() as i64;
                    agent_outputs.insert(planned.agent_id.clone(), output.clone());

                    // Update assignment as completed
                    {
                        let state_clone = state.clone();
                        let aid = assignment_id.clone();
                        let out = output.clone();
                        let model = agent_model.clone();
                        tokio::task::spawn_blocking(move || {
                            task_run_repo::update_task_assignment(
                                &state_clone, &aid, "completed", Some(&out), Some(&model),
                                0, 0, duration_ms, None,
                            )
                        })
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))??;
                    }

                    let _ = app.emit("orchestration:agent_completed", &serde_json::json!({
                        "taskRunId": task_run_id,
                        "assignmentId": assignment_id,
                        "agentId": planned.agent_id,
                        "agentName": agent_name,
                        "durationMs": assign_start.elapsed().as_millis() as i64,
                        "status": "completed",
                    }));
                }
                Err(e) => {
                    let duration_ms = assign_start.elapsed().as_millis() as i64;
                    let err_msg = e.to_string();

                    // Update assignment as failed
                    {
                        let state_clone = state.clone();
                        let aid = assignment_id.clone();
                        let em = err_msg.clone();
                        tokio::task::spawn_blocking(move || {
                            task_run_repo::update_task_assignment(
                                &state_clone, &aid, "failed", None, None,
                                0, 0, duration_ms, Some(&em),
                            )
                        })
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))??;
                    }

                    let _ = app.emit("orchestration:agent_completed", &serde_json::json!({
                        "taskRunId": task_run_id,
                        "assignmentId": assignment_id,
                        "agentId": planned.agent_id,
                        "agentName": agent_name,
                        "durationMs": duration_ms,
                        "status": "failed",
                        "error": err_msg,
                    }));

                    log::warn!("Agent assignment failed: {}", e);
                }
            }
        }

        // After each sequence group, send feedback to control hub
        if !agent_outputs.is_empty() {
            let feedback = build_feedback_prompt(&agent_outputs, &all_agents);
            let _ = app.emit("orchestration:feedback", &serde_json::json!({
                "taskRunId": task_run_id,
                "message": "Control Hub reviewing results...",
            }));

            // We don't need to act on the feedback for now, just log it
            if let Ok(response) = send_prompt_to_agent(app, state, &hub_agent.id, &feedback).await {
                log::info!("Control Hub feedback: {}", response);
            }
        }
    }

    // 7. Finalize — ask control hub for a summary
    let summary_prompt = format!(
        "Summarize the results of the orchestration.\n\nOriginal request: {}\n\nAgent outputs:\n{}",
        user_prompt,
        agent_outputs
            .iter()
            .map(|(id, out)| {
                let name = all_agents
                    .iter()
                    .find(|a| a.id == *id)
                    .map(|a| a.name.as_str())
                    .unwrap_or("Unknown");
                format!("--- {} ---\n{}\n", name, out)
            })
            .collect::<String>()
    );

    let summary = send_prompt_to_agent(app, state, &hub_agent.id, &summary_prompt)
        .await
        .unwrap_or_else(|_| "Summary not available".into());

    let total_duration_ms = start_time.elapsed().as_millis() as i64;

    // Update task run with summary and totals
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        let sum = summary.clone();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_summary(&state_clone, &id, &sum)
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_totals(
                &state_clone, &id, total_tokens_in, total_tokens_out, total_duration_ms,
            )
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id, "completed")
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    // Write output summary file
    write_output_summary(state, task_run_id, user_prompt, &plan, &all_agents, &summary, total_duration_ms).await;

    let _ = app.emit("orchestration:completed", &serde_json::json!({
        "taskRunId": task_run_id,
        "summary": summary,
        "totalDurationMs": total_duration_ms,
        "totalTokensIn": total_tokens_in,
        "totalTokensOut": total_tokens_out,
    }));

    Ok(())
}

fn build_agent_catalog(agents: &[AgentConfig]) -> String {
    agents
        .iter()
        .map(|a| {
            let caps: Vec<String> = serde_json::from_str(&a.capabilities_json).unwrap_or_default();
            format!(
                "- ID: {}\n  Name: {}\n  Description: {}\n  Model: {}\n  Capabilities: [{}]\n",
                a.id, a.name, a.description, a.model,
                caps.join(", ")
            )
        })
        .collect::<String>()
}

fn build_feedback_prompt(outputs: &HashMap<String, String>, agents: &[AgentConfig]) -> String {
    let mut parts = vec!["Here are the results from the agents so far:\n".to_string()];
    for (id, output) in outputs {
        let name = agents
            .iter()
            .find(|a| a.id == *id)
            .map(|a| a.name.as_str())
            .unwrap_or("Unknown");
        parts.push(format!("--- {} ---\n{}\n", name, output));
    }
    parts.push("Are these results satisfactory? Reply with a brief assessment.".into());
    parts.join("\n")
}

fn parse_task_plan(response: &str) -> AppResult<TaskPlan> {
    // Try to find JSON in the response
    let json_str = extract_json_from_response(response);

    serde_json::from_str::<TaskPlan>(&json_str)
        .map_err(|e| AppError::Internal(format!(
            "Failed to parse task plan from Control Hub response: {e}\nResponse: {response}"
        )))
}

fn extract_json_from_response(response: &str) -> String {
    // Try to find a JSON block between ```json and ```
    if let Some(start) = response.find("```json") {
        let after = &response[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    // Try to find JSON between ``` and ```
    if let Some(start) = response.find("```") {
        let after = &response[start + 3..];
        if let Some(end) = after.find("```") {
            let candidate = after[..end].trim();
            if candidate.starts_with('{') {
                return candidate.to_string();
            }
        }
    }
    // Try to find raw JSON object
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            return response[start..=end].to_string();
        }
    }
    response.to_string()
}

async fn ensure_agent_running(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
) -> AppResult<()> {
    let process_running = {
        let processes = state.agent_processes.lock().await;
        processes.contains_key(&agent.id)
    };

    if process_running {
        return Ok(());
    }

    let acp_command = agent.acp_command.clone().ok_or_else(|| {
        AppError::Internal(format!("Agent {} has no ACP command configured", agent.id))
    })?;

    let args: Vec<String> = agent
        .acp_args_json
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();

    let mut final_command = acp_command;
    let mut final_args = args;

    // Auto-upgrade npx/pnpx
    if final_command.contains("npx") || final_command.contains("pnpx") {
        let project_root = crate::acp::discovery::get_project_root();
        let adapter_path = project_root
            .join("node_modules")
            .join("@zed-industries")
            .join("claude-code-acp")
            .join("dist")
            .join("index.js");

        if adapter_path.exists() {
            let enriched_path = crate::acp::discovery::get_enriched_path();
            if let Some(node_path) = std::env::split_paths(&enriched_path)
                .map(|p| p.join("node"))
                .find(|p| p.exists())
            {
                final_command = node_path.to_string_lossy().to_string();
                final_args = vec![adapter_path.to_string_lossy().to_string()];
            }
        }
    }

    // Sync from discovered agents
    {
        let discovered = state.discovered_agents.lock().await;
        let cmd_basename = std::path::Path::new(&final_command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&final_command);

        if let Some(matched) = discovered.iter().find(|d| {
            d.available && {
                let d_basename = std::path::Path::new(&d.command)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&d.command);
                d_basename == cmd_basename || d.name == agent.name
            }
        }) {
            let discovered_args: Vec<String> =
                serde_json::from_str(&matched.args_json).unwrap_or_default();
            if final_command != matched.command || final_args != discovered_args {
                final_command = matched.command.clone();
                final_args = discovered_args;
            }
        }
    }

    log::info!("Orchestrator spawning agent: {}, command={}, args={:?}", agent.id, final_command, final_args);

    let process = manager::spawn_agent_process(&agent.id, &final_command, &final_args).await?;
    let stdin_handle = process.stdin.clone();

    {
        let mut processes = state.agent_processes.lock().await;
        processes.insert(agent.id.clone(), process);
    }
    {
        let mut stdins = state.agent_stdins.lock().await;
        stdins.insert(agent.id.clone(), stdin_handle);
    }

    // Initialize
    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(&agent.id) {
            client::initialize_agent(process).await?;
        }
    }

    let _ = app.emit("acp:agent_started", &serde_json::json!({
        "agent_id": agent.id,
        "status": "Running"
    }));

    Ok(())
}

/// Send a prompt to an agent and collect the complete text response.
/// This creates a session if needed and waits for the full result.
async fn send_prompt_to_agent(
    app: &tauri::AppHandle,
    state: &AppState,
    agent_id: &str,
    prompt: &str,
) -> AppResult<String> {
    // Ensure agent is running
    let agent: AgentConfig = {
        let state_clone = state.clone();
        let aid = agent_id.to_string();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &aid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };
    ensure_agent_running(app, state, &agent).await?;

    // Check if we have an orchestration ACP session for this agent
    let orch_session_key = format!("orch:{}", agent_id);
    let acp_session_id = {
        let sessions = state.acp_sessions.lock().await;
        sessions.get(&orch_session_key).map(|s| s.acp_session_id.clone())
    };

    let acp_session_id = if let Some(id) = acp_session_id {
        id
    } else {
        // Create a new ACP session
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(agent_id) {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into());

            let acp_id = client::create_session(process, &cwd).await?;

            let mut sessions = state.acp_sessions.lock().await;
            sessions.insert(
                orch_session_key.clone(),
                crate::state::AcpSessionInfo {
                    session_id: orch_session_key.clone(),
                    agent_id: agent_id.to_string(),
                    acp_session_id: acp_id.clone(),
                },
            );

            acp_id
        } else {
            return Err(AppError::Internal(format!("Agent {} process not found", agent_id)));
        }
    };

    // Send prompt
    let request_id = chrono::Utc::now().timestamp_millis();
    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(agent_id) {
            client::send_prompt(process, &acp_session_id, prompt, request_id).await?;
        }
    }

    // Collect response
    let mut collected_text = String::new();

    loop {
        let msg = {
            let mut processes = state.agent_processes.lock().await;
            if let Some(process) = processes.get_mut(agent_id) {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(300),
                    transport::receive_message(process),
                )
                .await
                {
                    Ok(Ok(msg)) => Some(msg),
                    Ok(Err(e)) => {
                        log::error!("Error receiving orchestration message: {}", e);
                        None
                    }
                    Err(_) => {
                        log::warn!("Timeout receiving orchestration message");
                        None
                    }
                }
            } else {
                None
            }
        };

        match msg {
            Some(msg) => {
                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

                match method {
                    "session/update" => {
                        let update_type = msg
                            .get("params")
                            .and_then(|p| p.get("update"))
                            .and_then(|u| u.get("sessionUpdate"))
                            .and_then(|s| s.as_str())
                            .unwrap_or("");

                        if update_type == "agent_message_chunk" || update_type == "user_message_chunk" {
                            if let Some(text) = msg
                                .get("params")
                                .and_then(|p| p.get("update"))
                                .and_then(|u| u.get("content"))
                                .and_then(|c| c.get("text"))
                                .and_then(|t| t.as_str())
                            {
                                collected_text.push_str(text);

                                // Emit streaming chunk for UI
                                let _ = app.emit("orchestration:agent_chunk", &serde_json::json!({
                                    "agentId": agent_id,
                                    "text": text,
                                }));
                            }
                        }
                    }
                    "session/requestPermission" | "session/request_permission" => {
                        // Forward permission requests to frontend
                        let _ = app.emit("acp:permission_request", &msg);
                    }
                    "" => {
                        // JSON-RPC response — end of prompt
                        if msg.get("result").is_some() || msg.get("error").is_some() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            None => break,
        }
    }

    if collected_text.is_empty() {
        collected_text = "(No response from agent)".into();
    }

    Ok(collected_text)
}

async fn execute_agent_assignment(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
    input: &str,
    _task_run_id: &str,
) -> AppResult<String> {
    ensure_agent_running(app, state, agent).await?;
    send_prompt_to_agent(app, state, &agent.id, input).await
}

async fn is_cancelled(state: &AppState, task_run_id: &str) -> bool {
    let tokens = state.active_task_runs.lock().await;
    if let Some(token) = tokens.get(task_run_id) {
        token.is_cancelled()
    } else {
        false
    }
}

async fn write_output_summary(
    state: &AppState,
    task_run_id: &str,
    user_prompt: &str,
    plan: &TaskPlan,
    _agents: &[AgentConfig],
    summary: &str,
    total_duration_ms: i64,
) {
    let output_dir = get_output_dir().join(task_run_id);
    if std::fs::create_dir_all(&output_dir).is_err() {
        log::error!("Failed to create output dir: {:?}", output_dir);
        return;
    }

    // Get assignments from DB
    let assignments = {
        let state_clone = state.clone();
        let trid = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::list_assignments_for_run(&state_clone, &trid)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default()
    };

    let duration_str = format_duration(total_duration_ms);
    let total_in: i64 = assignments.iter().map(|a| a.tokens_in).sum();
    let total_out: i64 = assignments.iter().map(|a| a.tokens_out).sum();

    let mut md = format!(
        "# Task: {}\n**Duration**: {}\n**Total Tokens**: {} in / {} out\n\n## Plan\n{}\n\n## Agent Executions\n| # | Agent | Model | Tokens In | Tokens Out | Duration | Status |\n|---|-------|-------|-----------|------------|----------|--------|\n",
        user_prompt.lines().next().unwrap_or("Orchestration"),
        duration_str,
        total_in,
        total_out,
        plan.analysis,
    );

    for (i, assignment) in assignments.iter().enumerate() {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            i + 1,
            assignment.agent_name,
            assignment.model_used.as_deref().unwrap_or("--"),
            assignment.tokens_in,
            assignment.tokens_out,
            format_duration(assignment.duration_ms),
            assignment.status,
        ));
    }

    md.push_str(&format!("\n## Result\n{}\n", summary));

    let summary_path = output_dir.join("summary.md");
    if let Err(e) = std::fs::write(&summary_path, &md) {
        log::error!("Failed to write summary: {}", e);
    } else {
        log::info!("Orchestration summary written to: {:?}", summary_path);
    }
}

fn format_duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m {}s", mins, secs)
    }
}
