use std::collections::HashMap;
use serde::Serialize;
use tauri::Emitter;

use crate::acp::{client, discovery, manager, provisioner, skill_discovery, upgrade};
use crate::db::{agent_md, agent_repo, settings_repo, task_run_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::{AgentConfig, AgentSkill};
use crate::models::task_run::{TaskPlan, TaskRun, PlannedAssignment};
use crate::state::{AppState, ConfirmationAction};
use crate::db::migrations::{get_output_dir};
use crate::acp::skill_discovery::SkillDiscoveryResult;
use tokio_util::sync::CancellationToken;

/// Clean up all agent processes spawned for a specific task run.
/// Uses the `orch:{task_run_id}:` prefix to find and kill all processes belonging to this task.
async fn cleanup_task_processes(state: &AppState, task_run_id: &str) {
    let prefix = format!("orch:{}:", task_run_id);

    // Kill and remove all agent processes for this task run
    {
        let mut processes = state.agent_processes.lock().await;
        let keys_to_remove: Vec<String> = processes.keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        for key in &keys_to_remove {
            if let Some(mut process) = processes.remove(key) {
                if let Err(e) = manager::stop_agent_process(&mut process).await {
                    log::warn!("Failed to stop process {} during task cleanup: {}", key, e);
                }
            }
        }
        if !keys_to_remove.is_empty() {
            log::info!("Cleaned up {} agent processes for task {}", keys_to_remove.len(), task_run_id);
        }
    }

    // Remove stdin handles
    {
        let mut stdins = state.agent_stdins.lock().await;
        stdins.retain(|k, _| !k.starts_with(&prefix));
    }

    // Remove ACP sessions
    {
        let session_prefix = format!("orch_session:{}", prefix);
        let mut sessions = state.acp_sessions.lock().await;
        sessions.retain(|k, _| !k.starts_with(&session_prefix));
    }
}

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
    workspace_id: Option<String>,
) {
    let result = run_orchestration_inner(&app, &state, &task_run_id, &user_prompt, workspace_id.as_deref()).await;

    // Clean up all agent processes spawned for this task run (success, error, or cancel)
    cleanup_task_processes(&state, &task_run_id).await;

    // Always clean up the active task run entry so new orchestrations can start
    {
        let mut tokens = state.active_task_runs.lock().await;
        tokens.remove(&task_run_id);
    }
    // Clean up per-agent cancellation tokens for this task run
    {
        let mut agent_cancels = state.agent_cancellations.lock().await;
        agent_cancels.retain(|(trid, _), _| trid != &task_run_id);
    }

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
    workspace_id: Option<&str>,
) -> AppResult<()> {
    let start_time = std::time::Instant::now();

    // Check cancellation
    if is_cancelled(state, task_run_id).await {
        return Ok(());
    }

    // 1. Get the control hub agent (workspace-scoped)
    let hub_agent: AgentConfig = {
        let state_clone = state.clone();
        let ws_id = workspace_id.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || agent_repo::get_control_hub(&state_clone, ws_id.as_deref()))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
            .ok_or_else(|| AppError::Internal("No Control Hub agent configured for this workspace".into()))?
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
        "workspaceId": workspace_id,
    }));

    // 3. Discover workspace skills (cached)
    let cwd = resolve_orchestrator_working_directory(state, workspace_id);
    let discovery_result = {
        let mut cache = state.discovered_skills.lock().await;
        let needs_scan = match cache.as_ref() {
            Some(cached) => !cached.scanned_directories.iter().any(|d| d.contains(&cwd)),
            None => true,
        };
        if needs_scan {
            let cwd_clone = cwd.clone();
            let result = tokio::task::spawn_blocking(move || {
                skill_discovery::discover_skills(&cwd_clone)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
            log::info!(
                "Skill discovery: found {} skills from {} directories",
                result.skills.len(),
                result.scanned_directories.len(),
            );
            *cache = Some(result.clone());
            let _ = app.emit("orchestration:skills_discovered", &serde_json::json!({
                "taskRunId": task_run_id,
                "skillsCount": result.skills.len(),
            }));
            Some(result)
        } else {
            cache.clone()
        }
    };

    // 4. Build agent catalog (scoped to the workspace if provided)
    let all_agents: Vec<AgentConfig> = {
        let state_clone = state.clone();
        let ws_id = workspace_id.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || agent_repo::list_agents(&state_clone, ws_id.as_deref()))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // Filter to only enabled agents for orchestration
    let enabled_agents: Vec<&AgentConfig> = all_agents.iter().filter(|a| a.is_enabled).collect();

    let catalog = build_agent_catalog_refs(&enabled_agents, discovery_result.as_ref());

    // When workspace-scoped, always use the workspace-scoped catalog so the LLM
    // only sees agents that belong to this workspace.  Fall back to the global
    // registry file only when there is no workspace filter.
    let registry_content = if workspace_id.is_some() {
        catalog.clone()
    } else {
        agent_md::read_agents_registry().unwrap_or_else(|_| catalog.clone())
    };

    // 4. Ensure hub agent process is running and get a plan
    let hub_process_key = orch_process_key(task_run_id, &hub_agent.id);
    ensure_agent_running(app, state, &hub_agent, &hub_process_key).await?;

    let plan_prompt = format!(
        r#"You are the orchestrator control hub. Decompose the user request into subtasks and assign each to the best-matching agent.

## Available Agents

{catalog}

## User Request

{user_prompt}

## Instructions

1. Analyze the request and identify subtasks based ONLY on the information above.
2. Match each subtask to the agent whose skills best fit.
3. Respect each agent's constraints.
4. If no agent has a matching skill, choose the most general-purpose agent.

CRITICAL: You MUST respond with ONLY a valid JSON object. No explanations, no preamble, no markdown, no thinking — ONLY the JSON object below. Do NOT attempt to explore, research, or use tools. Make your plan based solely on the agent catalog and user request provided above.

{{"analysis": "Brief reasoning about task decomposition and agent matching", "assignments": [{{"agent_id": "uuid-from-catalog", "task_description": "Detailed instruction for the agent", "sequence_order": 0, "depends_on": [], "matched_skills": ["skill_id"], "selection_reason": "Why this agent"}}]}}

Rules:
- Output ONLY the JSON object, nothing else
- agent_id must come from the catalog above
- matched_skills must reference skill IDs from the assigned agent
- sequence_order: 0 for parallel, increment for sequential
- depends_on: agent_ids whose output is needed first
- Always return at least one assignment"#,
        catalog = registry_content,
    );

    let plan_response = send_prompt_to_agent(app, state, &hub_agent.id, &plan_prompt, Some(task_run_id), None, workspace_id, &hub_process_key).await?;

    if is_cancelled(state, task_run_id).await {
        return Ok(());
    }

    // Parse the plan, with one retry on failure
    let plan = match parse_task_plan(&plan_response.text) {
        Ok(p) => p,
        Err(first_err) => {
            log::warn!("First plan parse failed, retrying with correction prompt: {}", first_err);

            let retry_prompt = format!(
                "Your previous response was not valid JSON. I need ONLY a raw JSON object, no text before or after it.\n\n\
                 The expected format is:\n\
                 {{\"analysis\": \"...\", \"assignments\": [{{\"agent_id\": \"...\", \"task_description\": \"...\", \"sequence_order\": 0, \"depends_on\": [], \"matched_skills\": [\"...\"], \"selection_reason\": \"...\"}}]}}\n\n\
                 Respond with ONLY the JSON object. No markdown code fences, no explanation."
            );

            let retry_response = send_prompt_to_agent(app, state, &hub_agent.id, &retry_prompt, Some(task_run_id), None, workspace_id, &hub_process_key).await?;

            parse_task_plan(&retry_response.text).map_err(|_| first_err)?
        }
    };

    // Auto-correct matched_skills before validation
    let plan = auto_correct_plan_skills(plan, &all_agents, discovery_result.as_ref());

    // Validate skill matching (soft validation — warnings only)
    let validation = validate_plan_skill_matching(&plan, &all_agents, discovery_result.as_ref());
    if !validation.is_valid {
        for av in &validation.assignment_validations {
            for warning in &av.warnings {
                log::warn!(
                    "Skill validation warning for agent '{}' ({}): {}",
                    av.agent_name, av.agent_id, warning
                );
            }
        }
    }
    let _ = app.emit("orchestration:plan_validated", &serde_json::json!({
        "taskRunId": task_run_id,
        "validation": &validation,
    }));

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

    // Filter out assignments to agents that are not in the workspace or are disabled
    let plan = TaskPlan {
        analysis: plan.analysis,
        assignments: plan.assignments.into_iter().filter(|a| {
            match all_agents.iter().find(|ag| ag.id == a.agent_id) {
                Some(ag) => ag.is_enabled,
                None => {
                    log::warn!(
                        "Dropping assignment to unknown agent '{}' (not in workspace)",
                        a.agent_id
                    );
                    false
                }
            }
        }).collect(),
    };

    if plan.assignments.is_empty() {
        return Err(AppError::Internal(
            "No valid assignments in plan — all referenced agents are outside this workspace or disabled".into()
        ));
    }

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

                // Add peer agent catalog for A2A discovery
                let peer_catalog = build_peer_agent_section(&all_agents, &planned.agent_id);
                if !peer_catalog.is_empty() {
                    input_parts.push(peer_catalog);
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
                    let orch_key = format!("orch_session:{}", orch_process_key(task_run_id, &planned.agent_id));
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

                let ws_id_clone = workspace_id.map(|s| s.to_string());
                let all_agents_clone = all_agents.clone();
                join_set.spawn(async move {
                    let assign_start = std::time::Instant::now();

                    let result = execute_with_a2a_routing(
                        &app_clone,
                        &state_clone,
                        &agent_config,
                        &input_clone,
                        &task_run_id_clone,
                        agent_cancel_token.as_ref(),
                        ws_id_clone.as_deref(),
                        &all_agents_clone,
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
            if let Ok(response) = send_prompt_to_agent(app, state, &hub_agent.id, &feedback, Some(task_run_id), None, workspace_id, &hub_process_key).await {
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
                    let orch_key = format!("orch_session:{}", orch_process_key(task_run_id, &agent_id));
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
                    app, state, &agent_config, &input_text, task_run_id, None, workspace_id,
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
                            let orch_key = format!("orch_session:{}", orch_process_key(task_run_id, &planned.agent_id));
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
                            app, state, &agent_config, &input_text, task_run_id, None, workspace_id,
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

    let summary = send_prompt_to_agent(app, state, &hub_agent.id, &summary_prompt, Some(task_run_id), None, workspace_id, &hub_process_key)
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

fn build_agent_catalog_refs(agents: &[&AgentConfig], discovery: Option<&SkillDiscoveryResult>) -> String {
    build_structured_agent_catalog(agents, discovery)
}

/// Build a structured agent catalog in XML format for the control hub prompt.
/// XML is recommended by the Agent Skills spec for Claude model injection.
fn build_structured_agent_catalog(agents: &[&AgentConfig], discovery: Option<&SkillDiscoveryResult>) -> String {
    let mut xml = String::from("<available_agents>\n");

    for a in agents {
        let skills = if a.is_control_hub {
            resolve_agent_skills(a)
        } else {
            resolve_agent_skills_with_discovery(a, discovery)
        };

        xml.push_str("  <agent>\n");
        xml.push_str(&format!("    <id>{}</id>\n", xml_escape(&a.id)));
        xml.push_str(&format!("    <name>{}</name>\n", xml_escape(&a.name)));
        xml.push_str(&format!(
            "    <description>{}</description>\n",
            xml_escape(if a.description.is_empty() { "N/A" } else { &a.description })
        ));
        xml.push_str(&format!("    <model>{}</model>\n", xml_escape(&a.model)));
        xml.push_str(&format!("    <max_concurrency>{}</max_concurrency>\n", a.max_concurrency));

        if !skills.is_empty() {
            xml.push_str("    <skills>\n");
            for skill in &skills {
                let discovered = skill.skill_source.starts_with("discovered:");
                xml.push_str(&format!(
                    "      <skill discovered=\"{}\">\n",
                    discovered
                ));
                xml.push_str(&format!("        <id>{}</id>\n", xml_escape(&skill.id)));
                xml.push_str(&format!("        <name>{}</name>\n", xml_escape(&skill.name)));
                xml.push_str(&format!("        <description>{}</description>\n", xml_escape(&skill.description)));
                if !skill.constraints.is_empty() {
                    xml.push_str(&format!(
                        "        <allowed_tools>{}</allowed_tools>\n",
                        xml_escape(&skill.constraints.join(" "))
                    ));
                }
                if let Some(ref lic) = skill.license {
                    xml.push_str(&format!("        <license>{}</license>\n", xml_escape(lic)));
                }
                if let Some(ref compat) = skill.compatibility {
                    xml.push_str(&format!("        <compatibility>{}</compatibility>\n", xml_escape(compat)));
                }
                xml.push_str("      </skill>\n");
            }
            xml.push_str("    </skills>\n");
        }

        xml.push_str("  </agent>\n");
    }

    xml.push_str("</available_agents>");
    xml
}

/// Escape special XML characters in text content and attribute values.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Resolve the effective skills for an agent.
/// If `skills_json` is populated, use it directly.
/// Otherwise, auto-convert `capabilities_json` entries into minimal AgentSkill structs.
fn resolve_agent_skills(agent: &AgentConfig) -> Vec<AgentSkill> {
    // Try parsing skills_json first
    if !agent.skills_json.is_empty() && agent.skills_json != "[]" {
        if let Ok(skills) = serde_json::from_str::<Vec<AgentSkill>>(&agent.skills_json) {
            if !skills.is_empty() {
                return skills;
            }
        }
    }

    // Fallback: convert capabilities_json to minimal skills
    let capabilities: Vec<String> = serde_json::from_str(&agent.capabilities_json)
        .unwrap_or_default();

    capabilities
        .into_iter()
        .map(|cap| {
            let id = cap.to_lowercase().replace(' ', "_");
            let keywords = vec![cap.to_lowercase()];
            AgentSkill {
                id,
                name: cap,
                skill_type: "skill".into(),
                description: String::new(),
                task_keywords: keywords,
                constraints: Vec::new(),
                skill_source: String::new(),
                license: None,
                compatibility: None,
                metadata: std::collections::HashMap::new(),
            }
        })
        .collect()
}

/// Resolve skills for a non-control-hub agent, merging manual skills with discovered skills.
/// Manual skills take priority (dedup by ID).
fn resolve_agent_skills_with_discovery(
    agent: &AgentConfig,
    discovery: Option<&SkillDiscoveryResult>,
) -> Vec<AgentSkill> {
    let mut skills = resolve_agent_skills(agent);

    if let Some(disc) = discovery {
        let existing_ids: std::collections::HashSet<String> =
            skills.iter().map(|s| s.id.clone()).collect();

        for entry in &disc.skills {
            if !existing_ids.contains(&entry.skill.id) {
                skills.push(entry.skill.clone());
            }
        }
    }

    skills
}

// ---------------------------------------------------------------------------
// Agent-to-Agent (A2A) Communication
// ---------------------------------------------------------------------------

const MAX_A2A_ITERATIONS: usize = 5;

struct A2aCall {
    target_agent_id: String,
    prompt: String,
}

/// Parse `<a2a_call agent_id="...">prompt</a2a_call>` from agent output.
/// Uses the last occurrence if multiple are present.
fn parse_a2a_call(text: &str) -> Option<A2aCall> {
    let start_tag_prefix = "<a2a_call agent_id=\"";
    let end_tag = "</a2a_call>";

    let start_idx = text.rfind(start_tag_prefix)?;
    let after_prefix = &text[start_idx + start_tag_prefix.len()..];
    let quote_end = after_prefix.find('"')?;
    let agent_id = after_prefix[..quote_end].to_string();
    let close_bracket = after_prefix.find('>')?;
    let content_start = start_idx + start_tag_prefix.len() + close_bracket + 1;
    if content_start >= text.len() {
        return None;
    }
    let end_idx = text[content_start..].find(end_tag)?;
    let prompt = text[content_start..content_start + end_idx].trim().to_string();
    if agent_id.is_empty() || prompt.is_empty() {
        return None;
    }
    Some(A2aCall {
        target_agent_id: agent_id,
        prompt,
    })
}

/// Execute an agent assignment with A2A routing support.
/// After each agent execution, checks the output for `<a2a_call>` markers.
/// If found, executes the target agent and sends a follow-up prompt with the result.
/// Loops until no more A2A calls or max iterations reached.
async fn execute_with_a2a_routing(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
    initial_input: &str,
    task_run_id: &str,
    cancel_token: Option<&CancellationToken>,
    workspace_id: Option<&str>,
    all_agents: &[AgentConfig],
) -> AppResult<AgentPromptResult> {
    let mut current_input = initial_input.to_string();
    let mut accumulated_text = String::new();
    let mut total_result: Option<AgentPromptResult> = None;

    for iteration in 0..MAX_A2A_ITERATIONS {
        let result = execute_agent_assignment_with_self_healing(
            app, state, agent, &current_input, task_run_id, cancel_token, workspace_id,
        )
        .await?;

        accumulated_text.push_str(&result.text);

        // Check for A2A call in the output
        if let Some(a2a_call) = parse_a2a_call(&result.text) {
            // Validate target agent exists in workspace
            let target = all_agents.iter().find(|a| a.id == a2a_call.target_agent_id);
            if target.is_none() {
                // Agent not found — append error and send follow-up
                current_input = format!(
                    "The A2A call to agent '{}' failed: agent not found in this workspace. Please proceed without it.",
                    a2a_call.target_agent_id
                );
                total_result = Some(result);
                continue;
            }

            // Emit A2A call event
            let _ = app.emit("orchestration:a2a_call", &serde_json::json!({
                "taskRunId": task_run_id,
                "callerAgentId": agent.id,
                "targetAgentId": a2a_call.target_agent_id,
                "prompt": a2a_call.prompt,
                "iteration": iteration,
            }));

            // Execute target agent
            let target_process_key = orch_process_key(task_run_id, &a2a_call.target_agent_id);
            let target_result = send_prompt_to_agent(
                app,
                state,
                &a2a_call.target_agent_id,
                &a2a_call.prompt,
                Some(task_run_id),
                cancel_token,
                workspace_id,
                &target_process_key,
            )
            .await;

            let a2a_response = match target_result {
                Ok(r) => r.text,
                Err(e) => format!("(A2A call failed: {})", e),
            };

            // Emit A2A result event
            let _ = app.emit("orchestration:a2a_result", &serde_json::json!({
                "taskRunId": task_run_id,
                "callerAgentId": agent.id,
                "targetAgentId": a2a_call.target_agent_id,
                "resultPreview": a2a_response.chars().take(200).collect::<String>(),
                "iteration": iteration,
            }));

            // Build follow-up prompt for the calling agent
            let target_name = target.map(|a| a.name.as_str()).unwrap_or("Unknown");
            current_input = format!(
                "## A2A Call Result\n\nAgent **{}** responded:\n\n{}\n\n---\n\nPlease continue your work with this result.",
                target_name, a2a_response
            );
            total_result = Some(result);
        } else {
            // No A2A call — we're done
            let mut final_result = result;
            final_result.text = accumulated_text;
            return Ok(final_result);
        }
    }

    // Max iterations reached — return what we have
    if let Some(mut r) = total_result {
        r.text = accumulated_text;
        Ok(r)
    } else {
        Err(AppError::Internal("A2A routing produced no result".into()))
    }
}

/// Build a "Peer Agents" section for A2A discovery.
/// Lists all enabled sibling agents in the workspace (excluding the current agent)
/// so the executing agent can discover and delegate to them at runtime.
fn build_peer_agent_section(all_agents: &[AgentConfig], current_agent_id: &str) -> String {
    let peers: Vec<&AgentConfig> = all_agents
        .iter()
        .filter(|a| a.id != current_agent_id && a.is_enabled)
        .collect();

    if peers.is_empty() {
        return String::new();
    }

    let mut section = String::from("\n\n---\n## Available Peer Agents\n");
    section.push_str("You can delegate subtasks to these agents. To call a peer agent, ");
    section.push_str("output an A2A call block at the end of your response:\n\n");
    section.push_str("```\n<a2a_call agent_id=\"AGENT_UUID\">\nDetailed task description for the agent\n</a2a_call>\n```\n\n");
    section.push_str("The orchestrator will execute the target agent and return the result in a follow-up prompt.\n\n");

    for peer in &peers {
        let caps = if peer.capabilities_json != "[]" {
            &peer.capabilities_json
        } else {
            ""
        };
        section.push_str(&format!(
            "- **{}** (`{}`): {} {}\n",
            peer.name, peer.id, peer.description, caps
        ));
    }
    section
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

#[derive(Debug, Clone, Serialize)]
struct AssignmentValidation {
    agent_id: String,
    agent_name: String,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PlanValidation {
    is_valid: bool,
    assignment_validations: Vec<AssignmentValidation>,
    total_warnings: usize,
}

/// Validate that the plan's skill matching is consistent with declared agent skills.
/// This is a soft validation — it only produces warnings, not errors.
fn validate_plan_skill_matching(
    plan: &TaskPlan,
    agents: &[AgentConfig],
    discovery: Option<&SkillDiscoveryResult>,
) -> PlanValidation {
    let mut assignment_validations = Vec::new();

    for assignment in &plan.assignments {
        let mut warnings = Vec::new();
        let agent_name;

        if let Some(agent) = agents.iter().find(|a| a.id == assignment.agent_id) {
            agent_name = agent.name.clone();
            let resolved_skills = if agent.is_control_hub {
                resolve_agent_skills(agent)
            } else {
                resolve_agent_skills_with_discovery(agent, discovery)
            };

            // Check: task description keywords hit a skill's constraints
            let desc_lower = assignment.task_description.to_lowercase();
            for skill in &resolved_skills {
                for constraint in &skill.constraints {
                    let constraint_lower = constraint.to_lowercase();
                    // Simple keyword overlap check
                    let constraint_words: Vec<&str> = constraint_lower.split_whitespace().collect();
                    let match_count = constraint_words.iter()
                        .filter(|w| w.len() > 3 && desc_lower.contains(**w))
                        .count();
                    if match_count >= 2 {
                        warnings.push(format!(
                            "Task may violate constraint on skill '{}': {}",
                            skill.id, constraint
                        ));
                    }
                }
            }
        } else {
            agent_name = "Unknown".into();
            warnings.push(format!(
                "Agent ID '{}' not found in registered agents",
                assignment.agent_id
            ));
        }

        if !warnings.is_empty() {
            assignment_validations.push(AssignmentValidation {
                agent_id: assignment.agent_id.clone(),
                agent_name,
                warnings,
            });
        }
    }

    let total_warnings: usize = assignment_validations.iter()
        .map(|v| v.warnings.len())
        .sum();

    PlanValidation {
        is_valid: total_warnings == 0,
        assignment_validations,
        total_warnings,
    }
}

/// Auto-correct `matched_skills` in a parsed plan to reference valid skill IDs.
///
/// For each assignment:
/// - Non-existent skill IDs are replaced with the closest match from the agent's skills
///   (using normalized string comparison: lowercase, hyphens/spaces → underscores).
/// - Empty `matched_skills` are inferred from the task description via keyword overlap
///   with skill names, IDs, descriptions, and task_keywords.
fn auto_correct_plan_skills(
    mut plan: TaskPlan,
    agents: &[AgentConfig],
    discovery: Option<&SkillDiscoveryResult>,
) -> TaskPlan {
    for assignment in &mut plan.assignments {
        let agent = match agents.iter().find(|a| a.id == assignment.agent_id) {
            Some(a) => a,
            None => continue,
        };

        let resolved_skills = if agent.is_control_hub {
            resolve_agent_skills(agent)
        } else {
            resolve_agent_skills_with_discovery(agent, discovery)
        };

        if resolved_skills.is_empty() {
            continue;
        }

        let skill_ids: Vec<&str> = resolved_skills.iter().map(|s| s.id.as_str()).collect();

        if assignment.matched_skills.is_empty() {
            // Infer skills from task description
            let desc_lower = assignment.task_description.to_lowercase();
            let mut matched = Vec::new();

            for skill in &resolved_skills {
                // Check if any keyword from the skill matches the task description
                let hit = skill.task_keywords.iter().any(|kw| {
                    kw.len() > 2 && desc_lower.contains(&kw.to_lowercase())
                }) || desc_lower.contains(&skill.name.to_lowercase())
                   || desc_lower.contains(&skill.id.to_lowercase())
                   || (!skill.description.is_empty()
                       && skill_description_overlaps(&desc_lower, &skill.description));

                if hit {
                    matched.push(skill.id.clone());
                }
            }

            if !matched.is_empty() {
                log::info!(
                    "Auto-corrected empty matched_skills for agent '{}': inferred {:?}",
                    agent.name, matched,
                );
                assignment.matched_skills = matched;
            }
        } else {
            // Fix non-existent skill IDs by finding closest match
            let mut corrected = Vec::new();
            for skill_id in &assignment.matched_skills {
                if skill_ids.contains(&skill_id.as_str()) {
                    corrected.push(skill_id.clone());
                } else if let Some(best) = find_closest_skill_id(skill_id, &skill_ids) {
                    log::info!(
                        "Auto-corrected skill '{}' → '{}' for agent '{}'",
                        skill_id, best, agent.name,
                    );
                    corrected.push(best);
                }
                // else: no close match found, drop it silently
            }

            if corrected.is_empty() {
                // All IDs were invalid and dropped — fall back to inference
                let desc_lower = assignment.task_description.to_lowercase();
                let mut inferred = Vec::new();

                for skill in &resolved_skills {
                    let hit = skill.task_keywords.iter().any(|kw| {
                        kw.len() > 2 && desc_lower.contains(&kw.to_lowercase())
                    }) || desc_lower.contains(&skill.name.to_lowercase())
                       || desc_lower.contains(&skill.id.to_lowercase())
                       || (!skill.description.is_empty()
                           && skill_description_overlaps(&desc_lower, &skill.description));

                    if hit {
                        inferred.push(skill.id.clone());
                    }
                }

                if !inferred.is_empty() {
                    log::info!(
                        "All matched_skills were invalid for agent '{}'; inferred {:?} from task description",
                        agent.name, inferred,
                    );
                }
                assignment.matched_skills = inferred;
            } else {
                assignment.matched_skills = corrected;
            }
        }
    }

    plan
}

/// Normalize a string for fuzzy skill-ID comparison: lowercase, replace hyphens/spaces with underscores.
fn normalize_skill_id(s: &str) -> String {
    s.to_lowercase().replace(['-', ' '], "_")
}

/// Find the closest matching skill ID using normalized comparison.
/// Returns `Some(matched_id)` if a reasonable match is found, `None` otherwise.
fn find_closest_skill_id(target: &str, candidates: &[&str]) -> Option<String> {
    let norm_target = normalize_skill_id(target);

    // Exact match after normalization
    for &cand in candidates {
        if normalize_skill_id(cand) == norm_target {
            return Some(cand.to_string());
        }
    }

    // Substring containment (either direction)
    for &cand in candidates {
        let norm_cand = normalize_skill_id(cand);
        if norm_cand.contains(&norm_target) || norm_target.contains(&norm_cand) {
            return Some(cand.to_string());
        }
    }

    None
}

/// Check if a task description has meaningful word overlap with a skill description.
/// Returns true if at least 2 words of length >3 from the skill description appear in the task.
fn skill_description_overlaps(task_lower: &str, skill_desc: &str) -> bool {
    let desc_lower = skill_desc.to_lowercase();
    let words: Vec<&str> = desc_lower.split_whitespace().collect();
    let hits = words.iter().filter(|w| w.len() > 3 && task_lower.contains(**w)).count();
    hits >= 2
}

fn parse_task_plan(response: &str) -> AppResult<TaskPlan> {
    let json_str = extract_json_from_response(response);
    let sanitized = sanitize_llm_json(&json_str);

    serde_json::from_str::<TaskPlan>(&sanitized)
        .map_err(|e| {
            // Truncate response preview — use char-aware slicing to avoid panics on multi-byte chars
            let preview = if response.chars().count() > 500 {
                let truncated: String = response.chars().take(500).collect();
                format!("{}...(truncated, {} chars total)", truncated, response.chars().count())
            } else {
                response.to_string()
            };
            AppError::Internal(format!(
                "Failed to parse task plan from Control Hub response: {e}\nResponse preview: {preview}"
            ))
        })
}

/// Sanitize JSON produced by LLMs — fix common issues that cause parse failures:
/// 1. Unescaped control characters (literal newlines, tabs, etc.) inside string values
/// 2. Unescaped double quotes inside string values (e.g. Chinese text like "重来又如何")
/// 3. Trailing commas before `}` or `]`
///
/// For unescaped quotes we use a look-ahead heuristic: a `"` inside a string is the
/// *real* closing quote only if the next non-whitespace byte is a JSON structural
/// character (`:`, `,`, `}`, `]`) or end-of-input.  Otherwise it is content and gets
/// escaped as `\"`.
fn sanitize_llm_json(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() + 64);
    let mut in_string = false;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            if b == b'\\' {
                // Escaped sequence — copy the backslash and the next byte verbatim
                out.push(b);
                if i + 1 < bytes.len() {
                    i += 1;
                    out.push(bytes[i]);
                }
            } else if b == b'"' {
                // Is this the real closing quote, or an unescaped content quote?
                // Look ahead past whitespace for a JSON structural character.
                let mut k = i + 1;
                while k < bytes.len()
                    && matches!(bytes[k], b' ' | b'\t' | b'\n' | b'\r')
                {
                    k += 1;
                }
                if k >= bytes.len()
                    || matches!(bytes[k], b':' | b',' | b'}' | b']')
                {
                    // Real closing quote
                    in_string = false;
                    out.push(b);
                } else {
                    // Content quote — escape it
                    out.extend_from_slice(b"\\\"");
                }
            } else if b < 0x20 {
                // Unescaped control character — escape it
                match b {
                    b'\n' => out.extend_from_slice(b"\\n"),
                    b'\r' => out.extend_from_slice(b"\\r"),
                    b'\t' => out.extend_from_slice(b"\\t"),
                    _ => {
                        out.extend_from_slice(format!("\\u{:04x}", b).as_bytes());
                    }
                }
            } else {
                out.push(b);
            }
        } else {
            if b == b'"' {
                in_string = true;
            }
            out.push(b);
        }
        i += 1;
    }

    let s = String::from_utf8(out).unwrap_or_else(|_| input.to_string());

    // Pass 2: remove trailing commas before } or ]
    let bytes2 = s.as_bytes();
    let mut result = Vec::with_capacity(bytes2.len());
    let mut in_str = false;
    let mut esc = false;
    let mut j = 0;
    while j < bytes2.len() {
        let b = bytes2[j];
        if esc {
            esc = false;
            result.push(b);
            j += 1;
            continue;
        }
        if in_str {
            if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            result.push(b);
            j += 1;
            continue;
        }
        if b == b'"' {
            in_str = true;
            result.push(b);
            j += 1;
            continue;
        }
        if b == b',' {
            let mut k = j + 1;
            while k < bytes2.len() && matches!(bytes2[k], b' ' | b'\t' | b'\n' | b'\r') {
                k += 1;
            }
            if k < bytes2.len() && (bytes2[k] == b'}' || bytes2[k] == b']') {
                j += 1;
                continue;
            }
        }
        result.push(b);
        j += 1;
    }

    String::from_utf8(result).unwrap_or(s)
}

fn extract_json_from_response(response: &str) -> String {
    // Strategy 1: Find the first '{' and use brace-depth matching to find its closing '}'.
    // This is the most robust approach as it handles embedded code fences in JSON strings.
    if let Some(start) = response.find('{') {
        let bytes = response.as_bytes();
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escape_next = false;
        let mut end = start;

        for i in start..bytes.len() {
            let ch = bytes[i] as char;
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' && in_string {
                escape_next = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                continue;
            }
            if in_string {
                continue;
            }
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
        }

        if depth == 0 && end > start {
            return response[start..=end].to_string();
        }
    }

    // Fallback: return as-is
    response.to_string()
}

/// Build a composite process key for orchestration: `orch:{task_run_id}:{agent_id}`.
/// Each task run gets its own agent process, preventing concurrent tasks from
/// stealing each other's messages on the shared `message_rx` channel.
fn orch_process_key(task_run_id: &str, agent_id: &str) -> String {
    format!("orch:{}:{}", task_run_id, agent_id)
}

async fn ensure_agent_running(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
    process_key: &str,
) -> AppResult<()> {
    let process_running = {
        let processes = state.agent_processes.lock().await;
        processes.contains_key(process_key)
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
        processes.insert(process_key.to_string(), process);
    }
    {
        let mut stdins = state.agent_stdins.lock().await;
        stdins.insert(process_key.to_string(), stdin_handle);
    }

    // Initialize using non-blocking pattern to avoid holding the lock during recv
    {
        use crate::acp::transport;

        let init_req = transport::build_request(
            1,
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": 1,
                "clientInfo": {
                    "name": "IAAgentHub",
                    "version": "0.1.0"
                },
                "clientCapabilities": {
                    "fs": {
                        "readTextFile": false,
                        "writeTextFile": false
                    },
                    "terminal": false
                }
            })),
        );

        // Send initialize request (brief lock)
        {
            let mut processes = state.agent_processes.lock().await;
            if let Some(process) = processes.get_mut(process_key) {
                transport::send_message(process, &init_req).await?;
            }
        }

        // Wait for initialize response using non-blocking try_recv
        let init_deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        let init_response = loop {
            let recv_result = {
                let mut processes = state.agent_processes.lock().await;
                match processes.get_mut(process_key) {
                    Some(process) => process.message_rx.try_recv(),
                    None => return Err(AppError::Internal(format!("Agent {} disappeared during init", agent.id))),
                }
            };
            match recv_result {
                Ok(msg) => {
                    if let Some(msg_id) = msg.get("id") {
                        if msg_id == &serde_json::json!(1) {
                            break msg;
                        }
                    }
                    // Skip non-matching messages during init
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                    if std::time::Instant::now() >= init_deadline {
                        return Err(AppError::Transport(
                            "Timeout (120s) waiting for agent initialization".into()
                        ));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    return Err(AppError::Transport(
                        "Agent message channel disconnected during initialization".into()
                    ));
                }
            }
        };

        // Check for error in initialize response
        if let Some(error) = init_response.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            return Err(AppError::Acp(format!("Initialize failed: {}", msg)));
        }

        log::info!("Agent {} initialized successfully", agent.id);
    }

    let _ = app.emit("acp:agent_started", &serde_json::json!({
        "agent_id": agent.id,
        "status": "Running"
    }));

    Ok(())
}

/// Stall detection: if no text chunk is received for this many seconds, send a continue nudge.
const STALL_TIMEOUT_SECS: u64 = 120;
/// Maximum number of continue nudges before giving up on a stalled agent.
const MAX_CONTINUE_NUDGES: usize = 3;

/// Create an ACP session using non-blocking try_recv to avoid holding the
/// agent_processes lock during the entire session creation handshake.
/// This is critical for parallel agent execution — holding the lock during
/// blocking recv() prevents all other agents from reading their messages.
async fn create_session_nonblocking(
    state: &AppState,
    process_key: &str,
    agent_id: &str,
    cwd: &str,
) -> AppResult<String> {
    use crate::acp::transport;

    log::info!("create_session_nonblocking: Starting for agent {} (key={})", agent_id, process_key);

    // Send session/new request (brief lock)
    let req = transport::build_request(
        2,
        "session/new",
        Some(serde_json::json!({
            "cwd": cwd,
            "mcpServers": []
        })),
    );
    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(process_key) {
            transport::send_message(process, &req).await?;
        } else {
            return Err(AppError::Internal(format!("Agent {} not found (key={})", agent_id, process_key)));
        }
    }
    // Lock released — other agents can now access their processes

    // Wait for session/new response using non-blocking try_recv
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    let response = loop {
        let recv_result = {
            let mut processes = state.agent_processes.lock().await;
            match processes.get_mut(process_key) {
                Some(process) => process.message_rx.try_recv(),
                None => return Err(AppError::Internal(format!("Agent {} disappeared", agent_id))),
            }
        };

        match recv_result {
            Ok(msg) => {
                // Check if this is the session/new response (id=2)
                if let Some(msg_id) = msg.get("id") {
                    if msg_id == &serde_json::json!(2) {
                        break msg;
                    }
                    log::debug!(
                        "create_session_nonblocking: skipping response with id={}, waiting for id=2",
                        msg_id
                    );
                } else {
                    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("unknown");
                    log::debug!(
                        "create_session_nonblocking: skipping notification '{}' while waiting for session/new",
                        method
                    );
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                if std::time::Instant::now() >= deadline {
                    return Err(AppError::Transport(
                        "Timeout (90s) waiting for session/new response".into()
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                return Err(AppError::Transport(
                    "Agent message channel disconnected during session creation".into()
                ));
            }
        }
    };

    // Parse the response
    if let Some(error) = response.get("error") {
        let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
        return Err(AppError::Acp(format!("session/new failed: {}", msg)));
    }

    let result = response.get("result").ok_or_else(|| {
        let resp_str = serde_json::to_string(&response).unwrap_or_default();
        AppError::Acp(format!("No result in session/new response: {}", resp_str))
    })?;

    let session_id = result
        .get("sessionId")
        .and_then(|s| s.as_str())
        .ok_or_else(|| {
            let resp_str = serde_json::to_string(&response).unwrap_or_default();
            AppError::Acp(format!("No sessionId in session/new response: {}", resp_str))
        })?
        .to_string();

    log::info!("create_session_nonblocking: Session created: {}", session_id);
    Ok(session_id)
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
    workspace_id: Option<&str>,
    process_key: &str,
) -> AppResult<AgentPromptResult> {
    // Ensure agent is running
    let agent: AgentConfig = {
        let state_clone = state.clone();
        let aid = agent_id.to_string();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &aid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };
    ensure_agent_running(app, state, &agent, process_key).await?;

    // Check if we have an orchestration ACP session for this process key
    let orch_session_key = format!("orch_session:{}", process_key);
    let acp_session_id = {
        let sessions = state.acp_sessions.lock().await;
        sessions.get(&orch_session_key).map(|s| s.acp_session_id.clone())
    };

    let acp_session_id = if let Some(id) = acp_session_id {
        id
    } else {
        // Create a new ACP session using non-blocking pattern to avoid holding
        // the agent_processes lock during the entire session creation handshake.
        let cwd = resolve_orchestrator_working_directory(state, workspace_id);
        let acp_id = create_session_nonblocking(state, process_key, agent_id, &cwd).await?;

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
    };

    // Send prompt
    let request_id = chrono::Utc::now().timestamp_millis();
    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(process_key) {
            client::send_prompt(process, &acp_session_id, prompt, request_id).await?;
        } else {
            return Err(AppError::Internal(format!("Agent {} process not found when sending prompt (key={})", agent_id, process_key)));
        }
    }

    // Collect response
    let mut collected_text = String::new();
    let mut tokens_in: i64 = 0;
    let mut tokens_out: i64 = 0;
    let mut cache_creation_tokens: i64 = 0;
    let mut cache_read_tokens: i64 = 0;
    let mut jsonrpc_error: Option<String> = None;

    // Stall detection state
    let mut last_text_chunk_at = std::time::Instant::now();
    let mut continue_nudges_sent: usize = 0;

    loop {
        // Check per-agent cancellation
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(AppError::Internal("Agent cancelled".into()));
            }
        }

        // Non-blocking receive: lock the HashMap briefly, try_recv, release immediately.
        // This prevents blocking other parallel agents from receiving their messages.
        let recv_result = {
            let mut processes = state.agent_processes.lock().await;
            match processes.get_mut(process_key) {
                Some(process) => process.message_rx.try_recv(),
                None => return Err(AppError::Internal(format!("Agent {} process disappeared (key={})", agent_id, process_key))),
            }
        };
        // HashMap lock is released here

        let msg = match recv_result {
            Ok(msg) => Some(msg),
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No message yet — check for stall, then yield
                if last_text_chunk_at.elapsed() >= std::time::Duration::from_secs(STALL_TIMEOUT_SECS) {
                    if continue_nudges_sent < MAX_CONTINUE_NUDGES {
                        log::info!(
                            "Agent {} stalled for {}s without text output, sending continue nudge ({}/{})",
                            agent_id,
                            last_text_chunk_at.elapsed().as_secs(),
                            continue_nudges_sent + 1,
                            MAX_CONTINUE_NUDGES,
                        );
                        let nudge_request_id = chrono::Utc::now().timestamp_millis();
                        let nudge_sent = {
                            let mut procs = state.agent_processes.lock().await;
                            if let Some(process) = procs.get_mut(process_key) {
                                client::send_prompt(
                                    process, &acp_session_id,
                                    "Please continue your work.",
                                    nudge_request_id,
                                ).await.is_ok()
                            } else { false }
                        };
                        if nudge_sent {
                            continue_nudges_sent += 1;
                            last_text_chunk_at = std::time::Instant::now();
                            let _ = app.emit("orchestration:agent_nudged", &serde_json::json!({
                                "taskRunId": task_run_id.unwrap_or(""),
                                "agentId": agent_id,
                                "nudgeCount": continue_nudges_sent,
                                "maxNudges": MAX_CONTINUE_NUDGES,
                            }));
                        }
                    } else {
                        log::warn!(
                            "Agent {} still stalled after {} continue nudges, giving up",
                            agent_id, continue_nudges_sent,
                        );
                        break;
                    }
                }
                // Yield briefly so other parallel agents can make progress
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                log::warn!("Agent {} message channel disconnected", agent_id);
                None
            }
        };

        match msg {
            Some(msg) => {
                let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
                log::debug!("Agent {} try_recv got message: method='{}'", agent_id, method);

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
                                    last_text_chunk_at = std::time::Instant::now();

                                    let _ = app.emit("orchestration:agent_chunk", &serde_json::json!({
                                        "taskRunId": task_run_id.unwrap_or(""),
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
                                    "taskRunId": task_run_id.unwrap_or(""),
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
                                        "taskRunId": task_run_id.unwrap_or(""),
                                        "agentId": agent_id,
                                        "text": text,
                                    }));
                                }
                            }
                            _ => {}
                        }
                    }
                    "session/requestPermission" | "session/request_permission" => {
                        log::info!("Agent {} received permission request", agent_id);
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
                            log::info!(
                                "Emitting orchestration:orch_permission for agent {} (task_run={}, request_id={})",
                                agent_id, trid, perm_request_id
                            );
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
                                if let Some(stdin) = stdins.get(process_key) {
                                    let response_json = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": perm_response_id,
                                        "result": {
                                            "outcome": {
                                                "outcome": "selected",
                                                "optionId": option_id,
                                            }
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
                        // JSON-RPC response — check if this is for the original prompt or a nudge
                        let response_id = msg.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                        let is_original_response = response_id == request_id;

                        // Extract token usage from any response (original or nudge)
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

                        if is_original_response {
                            // Original prompt completed — break out of the loop
                            if msg.get("result").is_some() || msg.get("error").is_some() {
                                break;
                            }
                        }
                        // Nudge response: don't break, keep collecting messages
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
    workspace_id: Option<&str>,
) -> AppResult<AgentPromptResult> {
    let process_key = orch_process_key(task_run_id, &agent.id);
    ensure_agent_running(app, state, agent, &process_key).await?;
    send_prompt_to_agent(app, state, &agent.id, input, Some(task_run_id), cancel_token, workspace_id, &process_key).await
}

/// Stop an agent process and clean up all associated state (sessions, stdin handles).
async fn stop_and_cleanup_agent(state: &AppState, process_key: &str, agent_id: &str) {
    // Stop and remove agent process
    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(mut process) = processes.remove(process_key) {
            if let Err(e) = manager::stop_agent_process(&mut process).await {
                log::warn!("Failed to stop agent {} (key={}) during cleanup: {}", agent_id, process_key, e);
            }
        }
    }

    // Remove stdin handle
    {
        let mut stdins = state.agent_stdins.lock().await;
        stdins.remove(process_key);
    }

    // Remove all ACP sessions belonging to this process key
    {
        let mut sessions = state.acp_sessions.lock().await;
        let session_key = format!("orch_session:{}", process_key);
        sessions.remove(&session_key);
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
    workspace_id: Option<&str>,
) -> AppResult<AgentPromptResult> {
    let mut retries = 0;

    loop {
        let result = execute_agent_assignment(app, state, agent, input, task_run_id, cancel_token, workspace_id).await;

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
                let process_key = orch_process_key(task_run_id, &agent.id);
                stop_and_cleanup_agent(state, &process_key, &agent.id).await;

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
/// When workspace_id is provided, uses the workspace's working_directory.
/// Falls back to the user-configured setting, then current_dir().
fn resolve_orchestrator_working_directory(state: &AppState, workspace_id: Option<&str>) -> String {
    if let Some(ws_id) = workspace_id {
        if let Ok(ws) = crate::db::workspace_repo::get_workspace(state, ws_id) {
            if !ws.working_directory.is_empty() {
                return ws.working_directory;
            }
        }
    }
    if let Ok(Some(setting)) = settings_repo::get_setting(state, "working_directory") {
        if !setting.value.is_empty() {
            return setting.value;
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into())
}

// ---------------------------------------------------------------------------
// Auto-resume on startup
// ---------------------------------------------------------------------------

/// Resume a single orchestration task run from its persisted state.
///
/// - `pending` / `analyzing`: Restart from scratch via `run_orchestration`
/// - `running`: Load plan + completed assignment outputs, skip completed, re-run the rest
/// - `awaiting_confirmation`: Load completed outputs, re-enter confirmation flow
pub async fn resume_orchestration(
    app: tauri::AppHandle,
    state: AppState,
    task_run: TaskRun,
) {
    let task_run_id = task_run.id.clone();
    let status = task_run.status.clone();

    log::info!(
        "Resuming orchestration task {} (status={})",
        task_run_id, status
    );

    let result = match status.as_str() {
        "pending" | "analyzing" => {
            // No usable plan — restart from scratch
            let user_prompt = task_run.user_prompt.clone();
            let workspace_id = task_run.workspace_id.clone();
            run_orchestration_inner(
                &app,
                &state,
                &task_run_id,
                &user_prompt,
                workspace_id.as_deref(),
            )
            .await
        }
        "running" => {
            resume_orchestration_running(&app, &state, &task_run).await
        }
        "awaiting_confirmation" => {
            resume_from_confirmation(&app, &state, &task_run).await
        }
        _ => {
            log::warn!("Unexpected status '{}' for resume, skipping task {}", status, task_run_id);
            Ok(())
        }
    };

    // Clean up (same as run_orchestration)
    cleanup_task_processes(&state, &task_run_id).await;

    {
        let mut tokens = state.active_task_runs.lock().await;
        tokens.remove(&task_run_id);
    }
    {
        let mut agent_cancels = state.agent_cancellations.lock().await;
        agent_cancels.retain(|(trid, _), _| trid != &task_run_id);
    }

    if let Err(e) = &result {
        let error_msg = e.to_string();
        log::error!("Resumed orchestration failed for {}: {}", task_run_id, error_msg);
        let _ = app.emit("orchestration:error", serde_json::json!({
            "taskRunId": task_run_id,
            "error": error_msg,
        }));
        let state_clone = state.clone();
        let id_clone = task_run_id.clone();
        let _ = tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id_clone, "failed")
        }).await;
    }
}

/// Resume an orchestration task that was previously in `running` state.
/// Loads the saved plan, skips completed assignments, and re-executes the rest.
async fn resume_orchestration_running(
    app: &tauri::AppHandle,
    state: &AppState,
    task_run: &TaskRun,
) -> AppResult<()> {
    let start_time = std::time::Instant::now();
    let task_run_id = &task_run.id;
    let user_prompt = &task_run.user_prompt;
    let workspace_id = task_run.workspace_id.as_deref();

    // 1. Parse the saved plan
    let plan_json = task_run.task_plan_json.as_deref().ok_or_else(|| {
        AppError::Internal(format!("Task {} is 'running' but has no plan — restarting from scratch is needed", task_run_id))
    })?;
    let plan: TaskPlan = serde_json::from_str(plan_json).map_err(|e| {
        AppError::Internal(format!("Failed to parse saved plan for task {}: {}", task_run_id, e))
    })?;

    // 2. Validate hub agent still exists
    let hub_agent: AgentConfig = {
        let state_clone = state.clone();
        let hub_id = task_run.control_hub_agent_id.clone();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &hub_id))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // 3. Load all agents (scoped to workspace)
    let all_agents: Vec<AgentConfig> = {
        let state_clone = state.clone();
        let ws_id: Option<String> = workspace_id.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || agent_repo::list_agents(&state_clone, ws_id.as_deref()))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // 4. Load existing assignments from DB
    let db_assignments: Vec<crate::models::task_run::TaskAssignment> = {
        let state_clone = state.clone();
        let trid = task_run_id.to_string();
        tokio::task::spawn_blocking(move || task_run_repo::list_assignments_for_run(&state_clone, &trid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // 5. Build agent_outputs from completed assignments + accumulate tokens
    let mut agent_outputs: HashMap<String, String> = HashMap::new();
    let mut total_tokens_in: i64 = 0;
    let mut total_tokens_out: i64 = 0;
    let mut total_cache_creation_tokens: i64 = 0;
    let mut total_cache_read_tokens: i64 = 0;

    // Track which (agent_id, sequence_order) pairs are already completed
    let mut completed_keys: std::collections::HashSet<(String, i64)> = std::collections::HashSet::new();

    for assignment in &db_assignments {
        if assignment.status == "completed" {
            if let Some(ref output) = assignment.output_text {
                agent_outputs.insert(assignment.agent_id.clone(), output.clone());
            }
            total_tokens_in += assignment.tokens_in;
            total_tokens_out += assignment.tokens_out;
            total_cache_creation_tokens += assignment.cache_creation_tokens;
            total_cache_read_tokens += assignment.cache_read_tokens;
            completed_keys.insert((assignment.agent_id.clone(), assignment.sequence_order));
        }
    }

    log::info!(
        "Task {} resume: {} completed assignments loaded, {} agent outputs recovered",
        task_run_id,
        completed_keys.len(),
        agent_outputs.len(),
    );

    // 6. Set status to running and emit started
    {
        let state_clone = state.clone();
        let id = task_run_id.to_string();
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &id, "running")
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    let _ = app.emit("orchestration:started", &serde_json::json!({
        "taskRunId": task_run_id,
        "status": "running",
        "resumed": true,
        "workspaceId": workspace_id,
    }));

    // 7. Group plan assignments by sequence_order and execute remaining
    let mut sequence_groups: HashMap<i64, Vec<&PlannedAssignment>> = HashMap::new();
    for assignment in &plan.assignments {
        // Skip assignments to agents that no longer exist or are disabled
        match all_agents.iter().find(|a| a.id == assignment.agent_id) {
            Some(ag) if ag.is_enabled => {
                sequence_groups
                    .entry(assignment.sequence_order)
                    .or_default()
                    .push(assignment);
            }
            Some(ag) => {
                log::warn!(
                    "Skipping assignment to disabled agent '{}' ({}) during resume",
                    ag.name, ag.id
                );
            }
            None => {
                log::warn!(
                    "Skipping assignment to deleted agent '{}' during resume",
                    assignment.agent_id
                );
            }
        }
    }

    let mut sorted_orders: Vec<i64> = sequence_groups.keys().copied().collect();
    sorted_orders.sort();

    let hub_process_key = orch_process_key(task_run_id, &hub_agent.id);

    for order in &sorted_orders {
        let group = &sequence_groups[order];

        // Filter to only assignments NOT already completed
        let remaining_in_group: Vec<&&PlannedAssignment> = group
            .iter()
            .filter(|planned| !completed_keys.contains(&(planned.agent_id.clone(), planned.sequence_order)))
            .collect();

        if remaining_in_group.is_empty() {
            log::info!("Sequence group {} fully completed, skipping", order);
            continue;
        }

        // Build concurrency map
        let agent_concurrency: HashMap<String, i64> = all_agents
            .iter()
            .map(|a| (a.id.clone(), a.max_concurrency))
            .collect();

        let mut remaining: Vec<&PlannedAssignment> = remaining_in_group.into_iter().copied().collect();

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

            let mut join_set = tokio::task::JoinSet::new();

            for planned in &batch {
                if is_cancelled(state, task_run_id).await {
                    return Ok(());
                }

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

                // Build input with dependency outputs
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

                let peer_catalog = build_peer_agent_section(&all_agents, &planned.agent_id);
                if !peer_catalog.is_empty() {
                    input_parts.push(peer_catalog);
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

                let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "assignmentId": assignment_id,
                    "agentId": planned.agent_id,
                    "agentName": agent_name,
                    "model": agent_model,
                    "sequenceOrder": planned.sequence_order,
                    "resumed": true,
                }));

                let agent_config = all_agents
                    .iter()
                    .find(|a| a.id == planned.agent_id)
                    .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", planned.agent_id)))?
                    .clone();

                let app_clone = app.clone();
                let state_clone = state.clone();
                let task_run_id_clone = task_run_id.to_string();
                let agent_id_clone = planned.agent_id.clone();
                let agent_name_clone = agent_name.clone();
                let agent_model_clone = agent_model.clone();
                let assignment_id_clone = assignment_id.clone();
                let input_clone = input_text.clone();

                let agent_cancel_token = {
                    let task_tokens = state.active_task_runs.lock().await;
                    task_tokens.get(task_run_id).map(|t| t.child_token())
                };
                if let Some(ref token) = agent_cancel_token {
                    let mut agent_cancels = state.agent_cancellations.lock().await;
                    agent_cancels.insert(
                        (task_run_id.to_string(), planned.agent_id.clone()),
                        token.clone(),
                    );
                }

                let ws_id_clone: Option<String> = workspace_id.map(|s| s.to_string());
                let all_agents_clone = all_agents.clone();
                join_set.spawn(async move {
                    let assign_start = std::time::Instant::now();

                    let result = execute_with_a2a_routing(
                        &app_clone,
                        &state_clone,
                        &agent_config,
                        &input_clone,
                        &task_run_id_clone,
                        agent_cancel_token.as_ref(),
                        ws_id_clone.as_deref(),
                        &all_agents_clone,
                    ).await;

                    let duration_ms = assign_start.elapsed().as_millis() as i64;

                    match result {
                        Ok(prompt_result) => {
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

                            (agent_id_clone, Err(err_msg))
                        }
                    }
                });
            }

            // Collect results
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
                        agent_outputs.insert(agent_id, format!("(Agent failed: {})", err_msg));
                    }
                    Err(e) => {
                        log::error!("Join error in parallel batch during resume: {}", e);
                    }
                }
            }

            remaining = deferred;
        }

        // Feedback to hub after each sequence group
        if !agent_outputs.is_empty() {
            ensure_agent_running(app, state, &hub_agent, &hub_process_key).await?;
            let feedback = build_feedback_prompt(&agent_outputs, &all_agents);
            let _ = app.emit("orchestration:feedback", &serde_json::json!({
                "taskRunId": task_run_id,
                "message": "Control Hub reviewing results...",
            }));
            if let Ok(response) = send_prompt_to_agent(app, state, &hub_agent.id, &feedback, Some(task_run_id), None, workspace_id, &hub_process_key).await {
                log::info!("Control Hub feedback (resume): {}", response.text);
            }
        }
    }

    // 8. Enter confirmation flow (same as normal orchestration)
    run_confirmation_and_summary(app, state, task_run_id, user_prompt, workspace_id, &hub_agent, &hub_process_key, &plan, &all_agents, &mut agent_outputs, &mut total_tokens_in, &mut total_tokens_out, &mut total_cache_creation_tokens, &mut total_cache_read_tokens, start_time).await
}

/// Resume an orchestration task that was previously in `awaiting_confirmation` state.
/// Loads completed outputs and re-enters the confirmation loop.
async fn resume_from_confirmation(
    app: &tauri::AppHandle,
    state: &AppState,
    task_run: &TaskRun,
) -> AppResult<()> {
    let start_time = std::time::Instant::now();
    let task_run_id = &task_run.id;
    let user_prompt = &task_run.user_prompt;
    let workspace_id = task_run.workspace_id.as_deref();

    // 1. Parse saved plan
    let plan_json = task_run.task_plan_json.as_deref().ok_or_else(|| {
        AppError::Internal(format!("Task {} is 'awaiting_confirmation' but has no plan", task_run_id))
    })?;
    let plan: TaskPlan = serde_json::from_str(plan_json).map_err(|e| {
        AppError::Internal(format!("Failed to parse saved plan for task {}: {}", task_run_id, e))
    })?;

    // 2. Validate hub agent
    let hub_agent: AgentConfig = {
        let state_clone = state.clone();
        let hub_id = task_run.control_hub_agent_id.clone();
        tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &hub_id))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // 3. Load all agents
    let all_agents: Vec<AgentConfig> = {
        let state_clone = state.clone();
        let ws_id: Option<String> = workspace_id.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || agent_repo::list_agents(&state_clone, ws_id.as_deref()))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // 4. Load completed assignment outputs + tokens
    let db_assignments = {
        let state_clone = state.clone();
        let trid = task_run_id.to_string();
        tokio::task::spawn_blocking(move || task_run_repo::list_assignments_for_run(&state_clone, &trid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    let mut agent_outputs: HashMap<String, String> = HashMap::new();
    let mut total_tokens_in: i64 = 0;
    let mut total_tokens_out: i64 = 0;
    let mut total_cache_creation_tokens: i64 = 0;
    let mut total_cache_read_tokens: i64 = 0;

    for assignment in &db_assignments {
        if assignment.status == "completed" {
            if let Some(ref output) = assignment.output_text {
                agent_outputs.insert(assignment.agent_id.clone(), output.clone());
            }
            total_tokens_in += assignment.tokens_in;
            total_tokens_out += assignment.tokens_out;
            total_cache_creation_tokens += assignment.cache_creation_tokens;
            total_cache_read_tokens += assignment.cache_read_tokens;
        }
    }

    log::info!(
        "Task {} resume from confirmation: {} agent outputs recovered",
        task_run_id,
        agent_outputs.len(),
    );

    let hub_process_key = orch_process_key(task_run_id, &hub_agent.id);

    // 5. Emit awaiting_confirmation and enter confirmation loop
    run_confirmation_and_summary(app, state, task_run_id, user_prompt, workspace_id, &hub_agent, &hub_process_key, &plan, &all_agents, &mut agent_outputs, &mut total_tokens_in, &mut total_tokens_out, &mut total_cache_creation_tokens, &mut total_cache_read_tokens, start_time).await
}

/// Shared confirmation + summary logic used by both normal orchestration and resume paths.
#[allow(clippy::too_many_arguments)]
async fn run_confirmation_and_summary(
    app: &tauri::AppHandle,
    state: &AppState,
    task_run_id: &str,
    user_prompt: &str,
    workspace_id: Option<&str>,
    hub_agent: &AgentConfig,
    hub_process_key: &str,
    plan: &TaskPlan,
    all_agents: &[AgentConfig],
    agent_outputs: &mut HashMap<String, String>,
    total_tokens_in: &mut i64,
    total_tokens_out: &mut i64,
    total_cache_creation_tokens: &mut i64,
    total_cache_read_tokens: &mut i64,
    start_time: std::time::Instant,
) -> AppResult<()> {
    // Emit awaiting_confirmation
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

        let (tx, rx) = tokio::sync::oneshot::channel::<ConfirmationAction>();
        {
            let mut confirmations = state.pending_confirmations.lock().await;
            confirmations.insert(task_run_id.to_string(), tx);
        }

        let action = match tokio::time::timeout(
            std::time::Duration::from_secs(3600),
            rx,
        ).await {
            Ok(Ok(action)) => action,
            Ok(Err(_)) => ConfirmationAction::Confirm,
            Err(_) => ConfirmationAction::Confirm,
        };

        match action {
            ConfirmationAction::Confirm => {
                break;
            }
            ConfirmationAction::RegenerateAgent(agent_id) => {
                log::info!("Regenerating agent {} for task {}", agent_id, task_run_id);

                let agent_config = all_agents.iter()
                    .find(|a| a.id == agent_id)
                    .ok_or_else(|| AppError::NotFound(format!("Agent {} not found", agent_id)))?
                    .clone();

                let agent_name = agent_config.name.clone();
                let agent_model = agent_config.model.clone();

                let planned = plan.assignments.iter().find(|a| a.agent_id == agent_id);

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

                let regen_assignment_id = uuid::Uuid::new_v4().to_string();
                let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                    "taskRunId": task_run_id,
                    "assignmentId": regen_assignment_id,
                    "agentId": agent_id,
                    "agentName": agent_name,
                    "model": agent_model,
                    "sequenceOrder": 0,
                    "isRegeneration": true,
                }));

                let assign_start = std::time::Instant::now();
                let result = execute_agent_assignment_with_self_healing(
                    app, state, &agent_config, &input_text, task_run_id, None, workspace_id,
                ).await;
                let duration_ms = assign_start.elapsed().as_millis() as i64;

                match result {
                    Ok(prompt_result) => {
                        *total_tokens_in += prompt_result.tokens_in;
                        *total_tokens_out += prompt_result.tokens_out;
                        *total_cache_creation_tokens += prompt_result.cache_creation_tokens;
                        *total_cache_read_tokens += prompt_result.cache_read_tokens;

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
            ConfirmationAction::RegenerateAll => {
                log::info!("Regenerating all agents for task {}", task_run_id);
                agent_outputs.clear();

                let mut sorted_orders: Vec<i64> = plan.assignments.iter()
                    .map(|a| a.sequence_order)
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                sorted_orders.sort();

                for order in &sorted_orders {
                    let group: Vec<&PlannedAssignment> = plan.assignments.iter()
                        .filter(|a| a.sequence_order == *order)
                        .collect();

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
                        let _ = app.emit("orchestration:agent_started", &serde_json::json!({
                            "taskRunId": task_run_id,
                            "assignmentId": regen_assignment_id,
                            "agentId": planned.agent_id,
                            "agentName": agent_name,
                            "model": agent_model,
                            "sequenceOrder": planned.sequence_order,
                            "isRegeneration": true,
                        }));

                        let assign_start = std::time::Instant::now();
                        let result = execute_agent_assignment_with_self_healing(
                            app, state, &agent_config, &input_text, task_run_id, None, workspace_id,
                        ).await;
                        let duration_ms = assign_start.elapsed().as_millis() as i64;

                        match result {
                            Ok(prompt_result) => {
                                *total_tokens_in += prompt_result.tokens_in;
                                *total_tokens_out += prompt_result.tokens_out;
                                *total_cache_creation_tokens += prompt_result.cache_creation_tokens;
                                *total_cache_read_tokens += prompt_result.cache_read_tokens;

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
    {
        let mut agent_cancels = state.agent_cancellations.lock().await;
        agent_cancels.retain(|(trid, _), _| trid != task_run_id);
    }

    // Generate summary
    ensure_agent_running(app, state, hub_agent, hub_process_key).await?;

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

    let summary = send_prompt_to_agent(app, state, &hub_agent.id, &summary_prompt, Some(task_run_id), None, workspace_id, hub_process_key)
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
        let ti = *total_tokens_in;
        let to = *total_tokens_out;
        let cc = *total_cache_creation_tokens;
        let cr = *total_cache_read_tokens;
        tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_totals(&state_clone, &id, ti, to, cc, cr, total_duration_ms)
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

    write_output_summary(state, task_run_id, user_prompt, plan, all_agents, &summary, total_duration_ms).await;

    let _ = app.emit("orchestration:completed", &serde_json::json!({
        "taskRunId": task_run_id,
        "summary": summary,
        "totalDurationMs": total_duration_ms,
        "totalTokensIn": *total_tokens_in,
        "totalTokensOut": *total_tokens_out,
        "totalCacheCreationTokens": *total_cache_creation_tokens,
        "totalCacheReadTokens": *total_cache_read_tokens,
    }));

    Ok(())
}

/// Resume all incomplete orchestration tasks found in the database.
/// Called once during app startup.
///
/// Each task is spawned independently since every task run uses its own agent
/// processes (keyed by `orch:{task_run_id}:{agent_id}`), so there is no
/// resource contention even within the same workspace.
pub async fn resume_incomplete_tasks(app: tauri::AppHandle, state: AppState) {
    let incomplete_tasks = {
        let state_clone = state.clone();
        match tokio::task::spawn_blocking(move || task_run_repo::list_incomplete_task_runs(&state_clone)).await {
            Ok(Ok(tasks)) => tasks,
            Ok(Err(e)) => {
                log::error!("Failed to query incomplete task runs on startup: {}", e);
                return;
            }
            Err(e) => {
                log::error!("Spawn blocking failed for incomplete task query: {}", e);
                return;
            }
        }
    };

    if incomplete_tasks.is_empty() {
        log::info!("No incomplete orchestration tasks to resume on startup");
        return;
    }

    log::info!(
        "Found {} incomplete orchestration task(s) to resume on startup",
        incomplete_tasks.len()
    );

    for task_run in incomplete_tasks {
        let task_run_id = task_run.id.clone();
        let status = task_run.status.clone();

        // Validate that the control hub agent still exists
        let hub_exists = {
            let state_clone = state.clone();
            let hub_id = task_run.control_hub_agent_id.clone();
            matches!(
                tokio::task::spawn_blocking(move || agent_repo::get_agent(&state_clone, &hub_id)).await,
                Ok(Ok(_))
            )
        };

        if !hub_exists {
            log::warn!(
                "Control hub agent '{}' no longer exists for task {} — marking as failed",
                task_run.control_hub_agent_id, task_run_id
            );
            let state_clone = state.clone();
            let id = task_run_id.clone();
            let _ = tokio::task::spawn_blocking(move || {
                task_run_repo::update_task_run_status(&state_clone, &id, "failed")
            }).await;
            continue;
        }

        // Create a cancellation token and register it
        let cancel_token = CancellationToken::new();
        {
            let mut tokens = state.active_task_runs.lock().await;
            tokens.insert(task_run_id.clone(), cancel_token);
        }

        // Emit resuming event to frontend
        let _ = app.emit("orchestration:resuming", &serde_json::json!({
            "taskRunId": task_run_id,
            "status": status,
        }));

        // Spawn each task independently — no contention since each task run
        // uses its own agent processes via orch:{task_run_id}:{agent_id} keys
        let app_clone = app.clone();
        let state_clone = state.clone();
        tauri::async_runtime::spawn(async move {
            resume_orchestration(app_clone, state_clone, task_run).await;
        });
    }
}
