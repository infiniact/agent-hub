use std::collections::HashMap;
use tauri::Emitter;

use crate::acp::{client, discovery, manager, provisioner, transport, upgrade};
use crate::db::{agent_md, agent_repo, settings_repo, task_run_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::AgentConfig;
use crate::models::task_run::{TaskPlan, PlannedAssignment};
use crate::state::{AppState, ConfirmationAction};
use crate::db::migrations::{get_output_dir};
use tokio_util::sync::CancellationToken;

/// Result from sending a prompt to an agent, including metadata
struct AgentPromptResult {
    text: String,
    tokens_in: i64,
    tokens_out: i64,
    cache_creation_tokens: i64,
    cache_read_tokens: i64,
    acp_session_id: String,
}

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
        let error_msg = e.to_string();
        log::error!("Orchestration failed for {}: {}", task_run_id, error_msg);
        let error_payload = serde_json::json!({
            "taskRunId": task_run_id,
            "error": error_msg,
        });
        log::info!("Emitting orchestration:error payload: {}", error_payload);
        if let Err(emit_err) = app.emit("orchestration:error", error_payload) {
            log::error!("Failed to emit orchestration:error event: {}", emit_err);
        }
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

    // Filter to only enabled agents for orchestration
    let enabled_agents: Vec<&AgentConfig> = all_agents.iter().filter(|a| a.is_enabled).collect();

    let catalog = build_agent_catalog_refs(&enabled_agents);

    // Try to use agents registry file; fall back to inline catalog
    let registry_content = agent_md::read_agents_registry()
        .unwrap_or_else(|_| catalog.clone());

    // 4. Ensure hub agent process is running and get a plan
    ensure_agent_running(app, state, &hub_agent).await?;

    let plan_prompt = format!(
        r#"You are the orchestrator control hub. Analyze this task and assign it to the most appropriate agents.

Available agents (from registry):
{registry_content}

User request: {user_prompt}

Respond with a JSON object (and nothing else) in this exact format:
{{"analysis": "your analysis of the task", "assignments": [{{"agent_id": "uuid", "task_description": "what this agent should do", "sequence_order": 0, "depends_on": []}}]}}

Rules:
- Only use agent IDs from the available agents list above
- Set sequence_order starting from 0 for parallel tasks, increment for sequential ones
- depends_on should list agent_ids whose output is needed as input
- If only one agent is needed, still return the assignments array with one entry
- Respect each agent's max_concurrency limit when assigning parallel tasks
- Do not assign more concurrent instances of an agent than its max_concurrency allows"#
    );

    let plan_response = send_prompt_to_agent(app, state, &hub_agent.id, &plan_prompt, Some(task_run_id), None).await?;

    if is_cancelled(state, task_run_id).await {
        return Ok(());
    }

    // Parse the plan
    let plan = parse_task_plan(&plan_response.text)?;

    // Validate: warn if hub assigned any disabled agents
    for assignment in &plan.assignments {
        if let Some(agent) = all_agents.iter().find(|a| a.id == assignment.agent_id) {
            if !agent.is_enabled {
                log::warn!(
                    "Hub assigned disabled agent '{}' ({}). Skipping this assignment.",
                    agent.name, agent.id
                );
            }
        }
    }

    // Filter out assignments to disabled agents
    let plan = TaskPlan {
        analysis: plan.analysis,
        assignments: plan.assignments.into_iter().filter(|a| {
            all_agents.iter()
                .find(|ag| ag.id == a.agent_id)
                .map(|ag| ag.is_enabled)
                .unwrap_or(true)
        }).collect(),
    };

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
    let mut total_tokens_in: i64 = 0;
    let mut total_tokens_out: i64 = 0;
    let mut total_cache_creation_tokens: i64 = 0;
    let mut total_cache_read_tokens: i64 = 0;

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

    for order in &sorted_orders {
        let group = &sequence_groups[order];

        // Build concurrency map: agent_id -> max_concurrency
        let agent_concurrency: HashMap<String, i64> = all_agents
            .iter()
            .map(|a| (a.id.clone(), a.max_concurrency))
            .collect();

        // Split group into batches that respect max_concurrency per agent
        let mut remaining: Vec<&PlannedAssignment> = group.iter().copied().collect();

        while !remaining.is_empty() {
            let mut batch: Vec<&PlannedAssignment> = Vec::new();
            let mut batch_agent_count: HashMap<String, i64> = HashMap::new();
            let mut deferred: Vec<&PlannedAssignment> = Vec::new();

            for planned in remaining {
                let max_conc = agent_concurrency.get(&planned.agent_id).copied().unwrap_or(1);
                let current = batch_agent_count.get(&planned.agent_id).copied().unwrap_or(0);

                if current < max_conc {
                    *batch_agent_count.entry(planned.agent_id.clone()).or_insert(0) += 1;
                    batch.push(planned);
                } else {
                    deferred.push(planned);
                }
            }

            // Execute batch assignments in parallel using tokio::JoinSet
            let mut join_set = tokio::task::JoinSet::new();

            for planned in &batch {
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
                            &state_clone, &aid, "running", None, None, 0, 0, 0, 0, 0, None,
                        )
                    })
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))??;
                }

                // Get ACP session ID for this agent if it exists
                let agent_acp_session_id = {
                    let sessions = state.acp_sessions.lock().await;
                    let orch_key = format!("orch:{}", planned.agent_id);
                    sessions.get(&orch_key).map(|s| s.acp_session_id.clone())
                };

                let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "assignmentId": assignment_id,
                    "agentId": planned.agent_id,
                    "agentName": agent_name,
                    "model": agent_model,
                    "sequenceOrder": planned.sequence_order,
                    "acpSessionId": agent_acp_session_id,
                }));

                // Ensure agent is running
                let agent_config = all_agents
                    .iter()
                    .find(|a| a.id == planned.agent_id)
                    .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", planned.agent_id)))?
                    .clone();

                // Spawn parallel task
                let app_clone = app.clone();
                let state_clone = state.clone();
                let task_run_id_clone = task_run_id.to_string();
                let agent_id_clone = planned.agent_id.clone();
                let agent_name_clone = agent_name.clone();
                let agent_model_clone = agent_model.clone();
                let assignment_id_clone = assignment_id.clone();
                let input_clone = input_text.clone();

                // Create per-agent child token from the task-level token
                let agent_cancel_token = {
                    let task_tokens = state.active_task_runs.lock().await;
                    task_tokens.get(task_run_id)
                        .map(|t| t.child_token())
                };
                // Store the per-agent token
                if let Some(ref token) = agent_cancel_token {
                    let mut agent_cancels = state.agent_cancellations.lock().await;
                    agent_cancels.insert(
                        (task_run_id.to_string(), planned.agent_id.clone()),
                        token.clone(),
                    );
                }

                join_set.spawn(async move {
                    let assign_start = std::time::Instant::now();

                    let result = execute_agent_assignment_with_self_healing(
                        &app_clone,
                        &state_clone,
                        &agent_config,
                        &input_clone,
                        &task_run_id_clone,
                        agent_cancel_token.as_ref(),
                    ).await;

                    let duration_ms = assign_start.elapsed().as_millis() as i64;

                    match result {
                        Ok(prompt_result) => {
                            // Update assignment as completed
                            {
                                let state_clone2 = state_clone.clone();
                                let aid = assignment_id_clone.clone();
                                let out = prompt_result.text.clone();
                                let model = agent_model_clone.clone();
                                let ti = prompt_result.tokens_in;
                                let to = prompt_result.tokens_out;
                                let cct = prompt_result.cache_creation_tokens;
                                let crt = prompt_result.cache_read_tokens;
                                let _ = tokio::task::spawn_blocking(move || {
                                    task_run_repo::update_task_assignment(
                                        &state_clone2, &aid, "completed", Some(&out), Some(&model),
                                        ti, to, cct, crt, duration_ms, None,
                                    )
                                }).await;
                            }

                            let _ = app_clone.emit("orchestration:agent_completed", &serde_json::json!({
                                "taskRunId": task_run_id_clone,
                                "assignmentId": assignment_id_clone,
                                "agentId": agent_id_clone,
                                "agentName": agent_name_clone,
                                "durationMs": duration_ms,
                                "status": "completed",
                                "tokensIn": prompt_result.tokens_in,
                                "tokensOut": prompt_result.tokens_out,
                                "cacheCreationTokens": prompt_result.cache_creation_tokens,
                                "cacheReadTokens": prompt_result.cache_read_tokens,
                                "acpSessionId": prompt_result.acp_session_id,
                                "output": prompt_result.text.clone(),
                            }));

                            (agent_id_clone, Ok(prompt_result))
                        }
                        Err(e) => {
                            let err_msg = e.to_string();
                            let is_cancelled = err_msg.contains("Agent cancelled");
                            let status = if is_cancelled { "cancelled" } else { "failed" };

                            // Auto-disable agent on non-cancellation errors
                            if !is_cancelled {
                                let state_for_disable = state_clone.clone();
                                let agent_id_for_disable = agent_id_clone.clone();
                                let err_for_disable = err_msg.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    agent_repo::disable_agent(
                                        &state_for_disable,
                                        &agent_id_for_disable,
                                        &err_for_disable,
                                    )
                                }).await;

                                let _ = app_clone.emit("orchestration:agent_auto_disabled", &serde_json::json!({
                                    "taskRunId": task_run_id_clone,
                                    "agentId": agent_id_clone,
                                    "agentName": agent_name_clone,
                                    "reason": &err_msg,
                                }));
                            }

                            // Update assignment as failed/cancelled
                            {
                                let state_clone2 = state_clone.clone();
                                let aid = assignment_id_clone.clone();
                                let em = err_msg.clone();
                                let s = status.to_string();
                                let _ = tokio::task::spawn_blocking(move || {
                                    task_run_repo::update_task_assignment(
                                        &state_clone2, &aid, &s, None, None,
                                        0, 0, 0, 0, duration_ms, Some(&em),
                                    )
                                }).await;
                            }

                            let _ = app_clone.emit("orchestration:agent_completed", &serde_json::json!({
                                "taskRunId": task_run_id_clone,
                                "assignmentId": assignment_id_clone,
                                "agentId": agent_id_clone,
                                "agentName": agent_name_clone,
                                "durationMs": duration_ms,
                                "status": status,
                                "error": &err_msg,
                            }));

                            log::warn!("Agent assignment failed for {}: {}", agent_name_clone, err_msg);

                            (agent_id_clone, Err(err_msg))
                        }
                    }
                });
            }

            // Collect results from all parallel tasks
            while let Some(join_result) = join_set.join_next().await {
                match join_result {
                    Ok((agent_id, Ok(prompt_result))) => {
                        total_tokens_in += prompt_result.tokens_in;
                        total_tokens_out += prompt_result.tokens_out;
                        total_cache_creation_tokens += prompt_result.cache_creation_tokens;
                        total_cache_read_tokens += prompt_result.cache_read_tokens;
                        agent_outputs.insert(agent_id, prompt_result.text);
                    }
                    Ok((agent_id, Err(err_msg))) => {
                        // Store error as output so downstream tasks can see it
                        agent_outputs.insert(agent_id, format!("(Agent failed: {})", err_msg));
                    }
                    Err(e) => {
                        log::error!("Join error in parallel batch: {}", e);
                    }
                }
            }

            remaining = deferred;
        }

        // After each sequence group, send feedback to control hub
        if !agent_outputs.is_empty() {
            let feedback = build_feedback_prompt(&agent_outputs, &all_agents);
            let _ = app.emit("orchestration:feedback", &serde_json::json!({
                "taskRunId": task_run_id,
                "message": "Control Hub reviewing results...",
            }));

            // We don't need to act on the feedback for now, just log it
            if let Ok(response) = send_prompt_to_agent(app, state, &hub_agent.id, &feedback, Some(task_run_id), None).await {
                log::info!("Control Hub feedback: {}", response.text);
            }
        }
    }

    // 7. Await user confirmation before summarizing
    // Emit awaiting_confirmation event with all agent outputs
    let _ = app.emit("orchestration:awaiting_confirmation", &serde_json::json!({
        "taskRunId": task_run_id,
        "agentOutputs": &agent_outputs.iter().map(|(id, out)| {
            let name = all_agents.iter().find(|a| a.id == *id)
                .map(|a| a.name.as_str()).unwrap_or("Unknown");
            serde_json::json!({ "agentId": id, "agentName": name, "output": out })
        }).collect::<Vec<_>>(),
    }));

    // Update status to awaiting_confirmation
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id, "awaiting_confirmation")
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    // Confirmation + regeneration loop
    loop {
        if is_cancelled(state, task_run_id).await {
            return Ok(());
        }

        // Create a oneshot channel and store it
        let (tx, rx) = tokio::sync::oneshot::channel::<ConfirmationAction>();
        {
            let mut confirmations = state.pending_confirmations.lock().await;
            confirmations.insert(task_run_id.to_string(), tx);
        }

        // Wait for user action
        let action = match tokio::time::timeout(
            std::time::Duration::from_secs(3600), // 1 hour timeout
            rx,
        ).await {
            Ok(Ok(action)) => action,
            Ok(Err(_)) => ConfirmationAction::Confirm, // channel dropped
            Err(_) => ConfirmationAction::Confirm,      // timeout
        };

        match action {
            ConfirmationAction::Confirm => {
                break; // Proceed to summary
            }
            ConfirmationAction::RegenerateAgent(agent_id) => {
                // Re-run a single agent
                log::info!("Regenerating agent {} for task {}", agent_id, task_run_id);

                let agent_config = all_agents.iter()
                    .find(|a| a.id == agent_id)
                    .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", agent_id)))?
                    .clone();

                let agent_name = agent_config.name.clone();
                let agent_model = agent_config.model.clone();

                // Find the original input for this agent from plan
                let planned = plan.assignments.iter()
                    .find(|a| a.agent_id == agent_id);

                let input_text = if let Some(planned) = planned {
                    let mut parts = vec![planned.task_description.clone()];
                    for dep_id in &planned.depends_on {
                        if let Some(output) = agent_outputs.get(dep_id) {
                            let dep_name = all_agents.iter()
                                .find(|a| a.id == *dep_id)
                                .map(|a| a.name.clone())
                                .unwrap_or_else(|| "Previous agent".into());
                            parts.push(format!("\n--- Output from {dep_name} ---\n{output}"));
                        }
                    }
                    parts.join("\n")
                } else {
                    "(Regenerated)".to_string()
                };

                // Emit agent_started for the regeneration
                let regen_assignment_id = uuid::Uuid::new_v4().to_string();
                let acp_sid = {
                    let sessions = state.acp_sessions.lock().await;
                    let orch_key = format!("orch:{}", agent_id);
                    sessions.get(&orch_key).map(|s| s.acp_session_id.clone())
                };

                let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "assignmentId": regen_assignment_id,
                    "agentId": agent_id,
                    "agentName": agent_name,
                    "model": agent_model,
                    "sequenceOrder": 0,
                    "acpSessionId": acp_sid,
                    "isRegeneration": true,
                }));

                let assign_start = std::time::Instant::now();
                let result = execute_agent_assignment_with_self_healing(
                    app, state, &agent_config, &input_text, task_run_id, None,
                ).await;
                let duration_ms = assign_start.elapsed().as_millis() as i64;

                match result {
                    Ok(prompt_result) => {
                        total_tokens_in += prompt_result.tokens_in;
                        total_tokens_out += prompt_result.tokens_out;
                        total_cache_creation_tokens += prompt_result.cache_creation_tokens;
                        total_cache_read_tokens += prompt_result.cache_read_tokens;

                        let _ = app.emit("orchestration:agent_completed", &serde_json::json!({
                            "taskRunId": task_run_id,
                            "assignmentId": regen_assignment_id,
                            "agentId": agent_id,
                            "agentName": agent_name,
                            "durationMs": duration_ms,
                            "status": "completed",
                            "tokensIn": prompt_result.tokens_in,
                            "tokensOut": prompt_result.tokens_out,
                            "cacheCreationTokens": prompt_result.cache_creation_tokens,
                            "cacheReadTokens": prompt_result.cache_read_tokens,
                            "acpSessionId": prompt_result.acp_session_id,
                            "output": prompt_result.text.clone(),
                        }));

                        agent_outputs.insert(agent_id.clone(), prompt_result.text);
                    }
                    Err(e) => {
                        let err_msg = e.to_string();

                        // Auto-disable agent on regeneration failure
                        {
                            let state_for_disable = state.clone();
                            let agent_id_for_disable = agent_id.clone();
                            let err_for_disable = err_msg.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                agent_repo::disable_agent(
                                    &state_for_disable,
                                    &agent_id_for_disable,
                                    &err_for_disable,
                                )
                            }).await;

                            let _ = app.emit("orchestration:agent_auto_disabled", &serde_json::json!({
                                "taskRunId": task_run_id,
                                "agentId": agent_id,
                                "agentName": agent_name,
                                "reason": &err_msg,
                            }));
                        }

                        let _ = app.emit("orchestration:agent_completed", &serde_json::json!({
                            "taskRunId": task_run_id,
                            "assignmentId": regen_assignment_id,
                            "agentId": agent_id,
                            "agentName": agent_name,
                            "durationMs": duration_ms,
                            "status": "failed",
                            "error": &err_msg,
                        }));
                        agent_outputs.insert(agent_id.clone(), format!("(Agent failed: {})", err_msg));
                    }
                }

                // Re-emit awaiting_confirmation so UI updates
                let _ = app.emit("orchestration:awaiting_confirmation", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "agentOutputs": &agent_outputs.iter().map(|(id, out)| {
                        let name = all_agents.iter().find(|a| a.id == *id)
                            .map(|a| a.name.as_str()).unwrap_or("Unknown");
                        serde_json::json!({ "agentId": id, "agentName": name, "output": out })
                    }).collect::<Vec<_>>(),
                }));
            }
            ConfirmationAction::RegenerateAll => {
                // Re-run all agents
                log::info!("Regenerating all agents for task {}", task_run_id);

                // Clear existing outputs
                agent_outputs.clear();

                // Re-execute all assignments following the same sequence order
                for order in &sorted_orders {
                    let group = &sequence_groups[order];

                    for planned in group {
                        if is_cancelled(state, task_run_id).await {
                            return Ok(());
                        }

                        let agent_config = all_agents.iter()
                            .find(|a| a.id == planned.agent_id)
                            .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", planned.agent_id)))?
                            .clone();

                        let agent_name = agent_config.name.clone();
                        let agent_model = agent_config.model.clone();

                        let mut input_parts = vec![planned.task_description.clone()];
                        for dep_id in &planned.depends_on {
                            if let Some(output) = agent_outputs.get(dep_id) {
                                let dep_name = all_agents.iter()
                                    .find(|a| a.id == *dep_id)
                                    .map(|a| a.name.clone())
                                    .unwrap_or_else(|| "Previous agent".into());
                                input_parts.push(format!("\n--- Output from {dep_name} ---\n{output}"));
                            }
                        }
                        let input_text = input_parts.join("\n");

                        let regen_assignment_id = uuid::Uuid::new_v4().to_string();
                        let acp_sid = {
                            let sessions = state.acp_sessions.lock().await;
                            let orch_key = format!("orch:{}", planned.agent_id);
                            sessions.get(&orch_key).map(|s| s.acp_session_id.clone())
                        };

                        let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                            "taskRunId": task_run_id,
                            "assignmentId": regen_assignment_id,
                            "agentId": planned.agent_id,
                            "agentName": agent_name,
                            "model": agent_model,
                            "sequenceOrder": planned.sequence_order,
                            "acpSessionId": acp_sid,
                            "isRegeneration": true,
                        }));

                        let assign_start = std::time::Instant::now();
                        let result = execute_agent_assignment_with_self_healing(
                            app, state, &agent_config, &input_text, task_run_id, None,
                        ).await;
                        let duration_ms = assign_start.elapsed().as_millis() as i64;

                        match result {
                            Ok(prompt_result) => {
                                total_tokens_in += prompt_result.tokens_in;
                                total_tokens_out += prompt_result.tokens_out;
                                total_cache_creation_tokens += prompt_result.cache_creation_tokens;
                                total_cache_read_tokens += prompt_result.cache_read_tokens;

                                let _ = app.emit("orchestration:agent_completed", &serde_json::json!({
                                    "taskRunId": task_run_id,
                                    "assignmentId": regen_assignment_id,
                                    "agentId": planned.agent_id,
                                    "agentName": agent_name,
                                    "durationMs": duration_ms,
                                    "status": "completed",
                                    "tokensIn": prompt_result.tokens_in,
                                    "tokensOut": prompt_result.tokens_out,
                                    "cacheCreationTokens": prompt_result.cache_creation_tokens,
                                    "cacheReadTokens": prompt_result.cache_read_tokens,
                                    "acpSessionId": prompt_result.acp_session_id,
                                    "output": prompt_result.text.clone(),
                                }));

                                agent_outputs.insert(planned.agent_id.clone(), prompt_result.text);
                            }
                            Err(e) => {
                                let err_msg = e.to_string();

                                // Auto-disable agent on regenerate-all failure
                                {
                                    let state_for_disable = state.clone();
                                    let agent_id_for_disable = planned.agent_id.clone();
                                    let err_for_disable = err_msg.clone();
                                    let _ = tokio::task::spawn_blocking(move || {
                                        agent_repo::disable_agent(
                                            &state_for_disable,
                                            &agent_id_for_disable,
                                            &err_for_disable,
                                        )
                                    }).await;

                                    let _ = app.emit("orchestration:agent_auto_disabled", &serde_json::json!({
                                        "taskRunId": task_run_id,
                                        "agentId": planned.agent_id,
                                        "agentName": agent_name,
                                        "reason": &err_msg,
                                    }));
                                }

                                let _ = app.emit("orchestration:agent_completed", &serde_json::json!({
                                    "taskRunId": task_run_id,
                                    "assignmentId": regen_assignment_id,
                                    "agentId": planned.agent_id,
                                    "agentName": agent_name,
                                    "durationMs": duration_ms,
                                    "status": "failed",
                                    "error": &err_msg,
                                }));
                                agent_outputs.insert(planned.agent_id.clone(), format!("(Agent failed: {})", err_msg));
                            }
                        }
                    }
                }

                // Re-emit awaiting_confirmation
                let _ = app.emit("orchestration:awaiting_confirmation", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "agentOutputs": &agent_outputs.iter().map(|(id, out)| {
                        let name = all_agents.iter().find(|a| a.id == *id)
                            .map(|a| a.name.as_str()).unwrap_or("Unknown");
                        serde_json::json!({ "agentId": id, "agentName": name, "output": out })
                    }).collect::<Vec<_>>(),
                }));
            }
        }
    }

    // Clean up pending confirmation
    {
        let mut confirmations = state.pending_confirmations.lock().await;
        confirmations.remove(task_run_id);
    }

    // Clean up per-agent cancellation tokens for this task run
    {
        let mut agent_cancels = state.agent_cancellations.lock().await;
        agent_cancels.retain(|(trid, _), _| trid != task_run_id);
    }

    // 8. Finalize — ask control hub for a summary
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

    let summary = send_prompt_to_agent(app, state, &hub_agent.id, &summary_prompt, Some(task_run_id), None)
        .await
        .map(|r| r.text)
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
                &state_clone, &id, total_tokens_in, total_tokens_out, total_cache_creation_tokens, total_cache_read_tokens, total_duration_ms,
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
        "totalCacheCreationTokens": total_cache_creation_tokens,
        "totalCacheReadTokens": total_cache_read_tokens,
    }));

    Ok(())
}

fn build_agent_catalog_refs(agents: &[&AgentConfig]) -> String {
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

    // Use provisioner to resolve the command
    let resolved = provisioner::resolve_agent_command(&acp_command, &args).await?;

    log::info!(
        "Orchestrator spawning agent: {}, command={}, args={:?}, agent_type={}",
        agent.id, resolved.command, resolved.args, resolved.agent_type
    );

    // Build extra environment variables
    let extra_env = discovery::get_agent_env_for_command(&resolved.agent_type).await;

    let process = manager::spawn_agent_process(
        &agent.id,
        &resolved.command,
        &resolved.args,
        &extra_env,
        &resolved.agent_type,
    ).await?;
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
/// Also forwards tool_call, thought events and extracts token usage.
async fn send_prompt_to_agent(
    app: &tauri::AppHandle,
    state: &AppState,
    agent_id: &str,
    prompt: &str,
    task_run_id: Option<&str>,
    cancel_token: Option<&CancellationToken>,
) -> AppResult<AgentPromptResult> {
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
            let cwd = resolve_orchestrator_working_directory(state);

            let (acp_id, _models) = client::create_session(process, &cwd).await?;

            let mut sessions = state.acp_sessions.lock().await;
            sessions.insert(
                orch_session_key.clone(),
                crate::state::AcpSessionInfo::new(
                    orch_session_key.clone(),
                    agent_id.to_string(),
                    acp_id.clone(),
                ),
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
    let mut tokens_in: i64 = 0;
    let mut tokens_out: i64 = 0;
    let mut cache_creation_tokens: i64 = 0;
    let mut cache_read_tokens: i64 = 0;
    let mut jsonrpc_error: Option<String> = None;

    loop {
        // Check per-agent cancellation
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(AppError::Internal("Agent cancelled".into()));
            }
        }

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

                        match update_type {
                            "agent_message_chunk" | "user_message_chunk" => {
                                if let Some(text) = msg
                                    .get("params")
                                    .and_then(|p| p.get("update"))
                                    .and_then(|u| u.get("content"))
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                {
                                    collected_text.push_str(text);

                                    let _ = app.emit("orchestration:agent_chunk", &serde_json::json!({
                                        "agentId": agent_id,
                                        "text": text,
                                    }));
                                }
                            }
                            "tool_call" | "tool_call_update" => {
                                // Forward tool call events
                                let update = msg.get("params")
                                    .and_then(|p| p.get("update"));
                                let tool_call_id = update
                                    .and_then(|u| u.get("toolCallId"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let tool_name = update
                                    .and_then(|u| u.get("name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let tool_title = update
                                    .and_then(|u| u.get("title"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let tool_status = update
                                    .and_then(|u| u.get("status"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(update_type);
                                let raw_input = update
                                    .and_then(|u| u.get("rawInput"))
                                    .cloned();
                                let raw_output = update
                                    .and_then(|u| u.get("rawOutput"))
                                    .cloned();

                                let _ = app.emit("orchestration:agent_tool_call", &serde_json::json!({
                                    "agentId": agent_id,
                                    "toolCallId": tool_call_id,
                                    "name": tool_name,
                                    "title": tool_title,
                                    "status": tool_status,
                                    "rawInput": raw_input,
                                    "rawOutput": raw_output,
                                }));
                            }
                            "agent_thought_chunk" => {
                                // Forward agent thought events
                                if let Some(text) = msg
                                    .get("params")
                                    .and_then(|p| p.get("update"))
                                    .and_then(|u| u.get("content"))
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                {
                                    let _ = app.emit("orchestration:agent_thought", &serde_json::json!({
                                        "agentId": agent_id,
                                        "text": text,
                                    }));
                                }
                            }
                            _ => {}
                        }
                    }
                    "session/requestPermission" | "session/request_permission" => {
                        // Extract permission request details
                        let params = msg.get("params");
                        let perm_request_id = msg.get("id")
                            .and_then(|v| v.as_i64())
                            .map(|v| v.to_string())
                            .or_else(|| msg.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                            .unwrap_or_default();

                        let session_id_val = params
                            .and_then(|p| p.get("sessionId"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        let tool_call_info = params
                            .and_then(|p| p.get("toolCall"))
                            .cloned();

                        let options = params
                            .and_then(|p| p.get("options"))
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!([]));

                        if let Some(trid) = task_run_id {
                            // Emit orchestration-specific permission event
                            let _ = app.emit("orchestration:orch_permission", &serde_json::json!({
                                "taskRunId": trid,
                                "agentId": agent_id,
                                "requestId": perm_request_id,
                                "sessionId": session_id_val,
                                "toolCall": tool_call_info,
                                "options": options,
                            }));

                            // Wait for user response via oneshot channel
                            let (tx, rx) = tokio::sync::oneshot::channel::<String>();
                            {
                                let perm_key = (trid.to_string(), perm_request_id.clone());
                                let mut perms = state.pending_orch_permissions.lock().await;
                                perms.insert(perm_key, tx);
                            }

                            // Wait with timeout
                            let option_id = match tokio::time::timeout(
                                std::time::Duration::from_secs(600),
                                rx,
                            ).await {
                                Ok(Ok(id)) => id,
                                Ok(Err(_)) => "allow".to_string(), // channel dropped, default allow
                                Err(_) => "allow".to_string(),     // timeout, default allow
                            };

                            // Send permission response back to agent via stdin
                            let perm_response_id: serde_json::Value = perm_request_id.parse::<i64>()
                                .map(|v| serde_json::json!(v))
                                .unwrap_or_else(|_| serde_json::json!(perm_request_id));
                            {
                                let stdins = state.agent_stdins.lock().await;
                                if let Some(stdin) = stdins.get(agent_id) {
                                    let response_json = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": perm_response_id,
                                        "result": {
                                            "outcome": "selected",
                                            "optionId": option_id,
                                        }
                                    });
                                    use tokio::io::AsyncWriteExt;
                                    let json_str = serde_json::to_string(&response_json).unwrap_or_default();
                                    let mut stdin_writer = stdin.lock().await;
                                    let _ = stdin_writer.write_all(json_str.as_bytes()).await;
                                    let _ = stdin_writer.write_all(b"\n").await;
                                    let _ = stdin_writer.flush().await;
                                }
                            }
                        } else {
                            // Non-orchestration context: forward as before
                            let _ = app.emit("acp:permission_request", &msg);
                        }
                    }
                    "" => {
                        // JSON-RPC response — end of prompt
                        // Extract token usage if present
                        if let Some(result) = msg.get("result") {
                            if let Some(usage) = result.get("usage") {
                                log::info!("Token usage from agent: {}", serde_json::to_string(&usage).unwrap_or_default());
                                tokens_in = usage.get("tokensIn")
                                    .or_else(|| usage.get("input_tokens"))
                                    .or_else(|| usage.get("promptTokens"))
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                tokens_out = usage.get("tokensOut")
                                    .or_else(|| usage.get("output_tokens"))
                                    .or_else(|| usage.get("completionTokens"))
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                cache_creation_tokens = usage.get("cacheCreationInputTokens")
                                    .or_else(|| usage.get("cache_creation_input_tokens"))
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                                cache_read_tokens = usage.get("cacheReadInputTokens")
                                    .or_else(|| usage.get("cache_read_input_tokens"))
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0);
                            }
                        }
                        // Capture JSON-RPC error if present
                        if let Some(error) = msg.get("error") {
                            let err_msg = error.get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown agent error");
                            let err_code = error.get("code")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            jsonrpc_error = Some(format!("Agent error (code {}): {}", err_code, err_msg));
                        }
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

    // Return error if the agent returned a JSON-RPC error
    if let Some(err) = jsonrpc_error {
        if collected_text.is_empty() {
            if upgrade::detect_upgrade_error(&err).is_some() {
                return Err(AppError::VersionUpgradeRequired(err));
            }
            return Err(AppError::Internal(err));
        }
        // If we got both text and an error, log the error but return the text
        log::warn!("Agent returned error alongside text: {}", err);
    }

    if collected_text.is_empty() {
        return Err(AppError::Internal(
            "Agent returned no response. Check that the agent is running and configured correctly.".into()
        ));
    }

    Ok(AgentPromptResult {
        text: collected_text,
        tokens_in,
        tokens_out,
        cache_creation_tokens,
        cache_read_tokens,
        acp_session_id,
    })
}

async fn execute_agent_assignment(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
    input: &str,
    task_run_id: &str,
    cancel_token: Option<&CancellationToken>,
) -> AppResult<AgentPromptResult> {
    ensure_agent_running(app, state, agent).await?;
    send_prompt_to_agent(app, state, &agent.id, input, Some(task_run_id), cancel_token).await
}

/// Stop an agent process and clean up all associated state (sessions, stdin handles).
async fn stop_and_cleanup_agent(state: &AppState, agent_id: &str) {
    // Stop and remove agent process
    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(mut process) = processes.remove(agent_id) {
            if let Err(e) = manager::stop_agent_process(&mut process).await {
                log::warn!("Failed to stop agent {} during cleanup: {}", agent_id, e);
            }
        }
    }

    // Remove stdin handle
    {
        let mut stdins = state.agent_stdins.lock().await;
        stdins.remove(agent_id);
    }

    // Remove all ACP sessions belonging to this agent
    {
        let mut sessions = state.acp_sessions.lock().await;
        sessions.retain(|_, info| info.agent_id != agent_id);
    }
}

/// Maximum number of upgrade retries before giving up.
const MAX_UPGRADE_RETRIES: usize = 1;

/// Wraps `execute_agent_assignment()` with automatic self-healing on version-upgrade errors.
///
/// If the agent returns a `VersionUpgradeRequired` error:
/// 1. Detects and parses the upgrade command
/// 2. Runs `npm install -g <package>@<version>`
/// 3. Optionally updates the local adapter
/// 4. Kills the old agent process and clears sessions
/// 5. Retries the assignment (agent will be re-spawned by `ensure_agent_running`)
async fn execute_agent_assignment_with_self_healing(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
    input: &str,
    task_run_id: &str,
    cancel_token: Option<&CancellationToken>,
) -> AppResult<AgentPromptResult> {
    let mut retries = 0;

    loop {
        let result = execute_agent_assignment(app, state, agent, input, task_run_id, cancel_token).await;

        match result {
            Ok(prompt_result) => return Ok(prompt_result),
            Err(AppError::VersionUpgradeRequired(ref err_msg)) => {
                if retries >= MAX_UPGRADE_RETRIES {
                    log::error!(
                        "Agent {} still requires upgrade after {} retries, giving up",
                        agent.id,
                        retries
                    );
                    return Err(AppError::Internal(format!(
                        "Agent upgrade failed after {} retries: {}",
                        retries, err_msg
                    )));
                }
                retries += 1;

                let upgrade_info = match upgrade::detect_upgrade_error(err_msg) {
                    Some(info) => info,
                    None => {
                        // Should not happen since we already detected it, but be safe
                        return Err(AppError::Internal(err_msg.clone()));
                    }
                };

                log::info!(
                    "Agent {} requires upgrade: {} — attempting automatic upgrade (retry {}/{})",
                    agent.id,
                    upgrade_info.package,
                    retries,
                    MAX_UPGRADE_RETRIES
                );

                // Emit upgrading event
                let _ = app.emit("orchestration:agent_upgrading", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "agentId": agent.id,
                    "agentName": agent.name,
                    "package": upgrade_info.package,
                }));

                // Run npm upgrade
                if let Err(e) = upgrade::run_npm_upgrade(&upgrade_info).await {
                    log::error!("npm upgrade failed for {}: {}", upgrade_info.package, e);
                    let _ = app.emit("orchestration:agent_upgrade_failed", &serde_json::json!({
                        "taskRunId": task_run_id,
                        "agentId": agent.id,
                        "agentName": agent.name,
                        "error": e.to_string(),
                    }));
                    return Err(e);
                }

                // Update local adapter (non-fatal)
                if let Err(e) = upgrade::update_local_adapter(&upgrade_info.agent_type).await {
                    log::warn!("Local adapter update failed (non-fatal): {}", e);
                }

                // Refresh registry to pick up new versions (non-fatal)
                if let Err(e) = discovery::refresh_registry().await {
                    log::warn!("Registry refresh failed (non-fatal): {}", e);
                }

                // Kill old process and clear sessions
                stop_and_cleanup_agent(state, &agent.id).await;

                // Emit upgraded event
                let _ = app.emit("orchestration:agent_upgraded", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "agentId": agent.id,
                    "agentName": agent.name,
                    "package": upgrade_info.package,
                }));

                // Loop back — execute_agent_assignment will call ensure_agent_running
                // which re-spawns the agent with the upgraded binary
                continue;
            }
            Err(other) => return Err(other),
        }
    }
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

/// Resolve the effective working directory for orchestration.
/// Returns the user-configured trusted directory, or falls back to current_dir().
fn resolve_orchestrator_working_directory(state: &AppState) -> String {
    if let Ok(Some(setting)) = settings_repo::get_setting(state, "working_directory") {
        if !setting.value.is_empty() {
            return setting.value;
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into())
}
