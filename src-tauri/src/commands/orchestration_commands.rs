use crate::acp::{orchestrator, skill_discovery};
use crate::db::{agent_repo, settings_repo, task_run_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::AgentConfig;
use crate::models::task_run::{CreateTaskRunRequest, ScheduleTaskRequest, TaskAssignment, TaskRun};
use crate::state::{AppState, ConfirmationAction};
use tokio_util::sync::CancellationToken;

#[tauri::command(rename_all = "camelCase")]
pub async fn start_orchestration(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: CreateTaskRunRequest,
) -> AppResult<TaskRun> {
    // Guard: reject if an orchestration is already running
    {
        let tokens = state.active_task_runs.lock().await;
        if !tokens.is_empty() {
            return Err(AppError::InvalidRequest(
                "An orchestration task is already running. Please wait for it to complete or cancel it before starting a new one.".into()
            ));
        }
    }

    // Verify control hub exists
    let hub: AgentConfig = {
        let state_clone = state.inner().clone();
        tokio::task::spawn_blocking(move || agent_repo::get_control_hub(&state_clone))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
            .ok_or_else(|| AppError::Internal("No Control Hub agent configured. Set an agent as Control Hub first.".into()))?
    };

    let task_run_id = uuid::Uuid::new_v4().to_string();
    let title = if request.title.is_empty() {
        request.user_prompt.chars().take(100).collect::<String>()
    } else {
        request.title.clone()
    };

    // Create task run record
    let task_run: TaskRun = {
        let state_clone = state.inner().clone();
        let trid = task_run_id.clone();
        let t = title.clone();
        let up = request.user_prompt.clone();
        let hub_id = hub.id.clone();
        tokio::task::spawn_blocking(move || {
            task_run_repo::create_task_run(&state_clone, &trid, &t, &up, &hub_id, "pending")
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // Create cancellation token
    let cancel_token = CancellationToken::new();
    {
        let mut tokens = state.active_task_runs.lock().await;
        tokens.insert(task_run_id.clone(), cancel_token);
    }

    // Spawn orchestration in background
    let state_clone = state.inner().clone();
    let trid = task_run_id.clone();
    let prompt = request.user_prompt.clone();
    tokio::spawn(async move {
        orchestrator::run_orchestration(app, state_clone, trid, prompt).await;
    });

    Ok(task_run)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_orchestration(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<()> {
    let mut tokens = state.active_task_runs.lock().await;
    if let Some(token) = tokens.remove(&task_run_id) {
        token.cancel();
    }

    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        task_run_repo::update_task_run_status(&state_clone, &task_run_id, "cancelled")
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(())
}

#[tauri::command]
pub async fn list_task_runs(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<TaskRun>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || task_run_repo::list_task_runs(&state))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_task_run(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<TaskRun> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || task_run_repo::get_task_run(&state, &task_run_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_task_assignments(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<Vec<TaskAssignment>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || task_run_repo::list_assignments_for_run(&state, &task_run_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

/// User confirms orchestration results — proceed to summary
#[tauri::command(rename_all = "camelCase")]
pub async fn confirm_orchestration(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<()> {
    let mut confirmations = state.pending_confirmations.lock().await;
    if let Some(tx) = confirmations.remove(&task_run_id) {
        let _ = tx.send(ConfirmationAction::Confirm);
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "No pending confirmation for task run {}",
            task_run_id
        )))
    }
}

/// Rate a completed task run (1-5 stars)
#[tauri::command(rename_all = "camelCase")]
pub async fn rate_task_run(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
    rating: i32,
) -> AppResult<()> {
    if rating < 1 || rating > 5 {
        return Err(AppError::InvalidRequest(
            "Rating must be between 1 and 5 stars".to_string()
        ));
    }

    task_run_repo::rate_task_run(&state, &task_run_id, rating)?;
    Ok(())
}

/// User requests re-running a single agent, or all agents if agent_id is "__all__"
#[tauri::command(rename_all = "camelCase")]
pub async fn regenerate_agent(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
    agent_id: String,
) -> AppResult<()> {
    let mut confirmations = state.pending_confirmations.lock().await;
    if let Some(tx) = confirmations.remove(&task_run_id) {
        let action = if agent_id == "__all__" {
            ConfirmationAction::RegenerateAll
        } else {
            ConfirmationAction::RegenerateAgent(agent_id)
        };
        let _ = tx.send(action);
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "No pending confirmation for task run {}",
            task_run_id
        )))
    }
}

/// User responds to a permission request during orchestration
#[tauri::command(rename_all = "camelCase")]
pub async fn respond_orch_permission(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
    _agent_id: String,
    request_id: String,
    option_id: String,
) -> AppResult<()> {
    let perm_key = (task_run_id.clone(), request_id.clone());
    let mut perms = state.pending_orch_permissions.lock().await;
    if let Some(tx) = perms.remove(&perm_key) {
        let _ = tx.send(option_id);
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "No pending permission for task run {}, request {}",
            task_run_id, request_id
        )))
    }
}

/// Cancel a single agent within an orchestration task run
#[tauri::command(rename_all = "camelCase")]
pub async fn cancel_agent(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
    agent_id: String,
) -> AppResult<()> {
    let agent_cancels = state.agent_cancellations.lock().await;
    if let Some(token) = agent_cancels.get(&(task_run_id.clone(), agent_id.clone())) {
        token.cancel();
        Ok(())
    } else {
        Err(AppError::NotFound("No active agent".into()))
    }
}

// ============== Scheduling Commands ==============

/// Schedule a task for future execution
#[tauri::command(rename_all = "camelCase")]
pub async fn schedule_task(
    state: tauri::State<'_, AppState>,
    request: ScheduleTaskRequest,
) -> AppResult<TaskRun> {
    // Validate schedule type
    if !["none", "once", "recurring"].contains(&request.schedule_type.as_str()) {
        return Err(AppError::InvalidRequest(
            "schedule_type must be 'none', 'once', or 'recurring'".to_string()
        ));
    }

    // Get the existing task run
    let task = {
        let state_clone = state.inner().clone();
        let trid = request.task_run_id.clone();
        tokio::task::spawn_blocking(move || task_run_repo::get_task_run(&state_clone, &trid))
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??
    };

    // Only completed tasks can be scheduled
    if task.status != "completed" {
        return Err(AppError::InvalidRequest(
            "Only completed tasks can be scheduled for re-execution".to_string()
        ));
    }

    // Calculate next run time based on schedule type
    let (scheduled_time, recurrence_pattern_json, next_run_at) = match request.schedule_type.as_str() {
        "none" => (None, None, None),
        "once" => {
            let time = request.scheduled_time.clone();
            // For one-time tasks, next_run_at is the scheduled time
            let next = time.clone();
            (time, None, next)
        }
        "recurring" => {
            let pattern = request.recurrence_pattern.as_ref()
                .ok_or_else(|| AppError::InvalidRequest(
                    "recurrence_pattern is required for recurring schedule".to_string()
                ))?;

            let pattern_json = serde_json::to_string(pattern)
                .map_err(|e| AppError::Internal(format!("Failed to serialize pattern: {}", e)))?;

            // Calculate first next_run_at
            let next_run = task_run_repo::calculate_next_run(
                &pattern.frequency,
                &pattern.time,
                pattern.interval,
                pattern.days_of_week.as_ref(),
                pattern.day_of_month,
                pattern.month,
            );

            (request.scheduled_time, Some(pattern_json), next_run)
        }
        _ => unreachable!(),
    };

    // Update the task schedule
    let state_clone = state.inner().clone();
    let task_run_id = request.task_run_id.clone();
    let st = scheduled_time.clone();
    let rpj = recurrence_pattern_json.clone();
    let nra = next_run_at.clone();
    let schedule_type = request.schedule_type.clone();

    let updated_task = tokio::task::spawn_blocking(move || {
        task_run_repo::update_schedule(
            &state_clone,
            &task_run_id,
            &schedule_type,
            st.as_deref(),
            rpj.as_deref(),
            nra.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(updated_task)
}

/// Pause a scheduled task
#[tauri::command(rename_all = "camelCase")]
pub async fn pause_scheduled_task(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<()> {
    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        task_run_repo::pause_scheduled_task(&state_clone, &task_run_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(())
}

/// Resume a paused scheduled task
#[tauri::command(rename_all = "camelCase")]
pub async fn resume_scheduled_task(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<()> {
    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        task_run_repo::resume_scheduled_task(&state_clone, &task_run_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(())
}

/// Clear the schedule for a task
#[tauri::command(rename_all = "camelCase")]
pub async fn clear_schedule(
    state: tauri::State<'_, AppState>,
    task_run_id: String,
) -> AppResult<()> {
    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        task_run_repo::clear_schedule(&state_clone, &task_run_id)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(())
}

/// Discover skills from the skills/ directories in the workspace and global config.
/// Results are cached; pass `force_refresh: true` to re-scan.
#[tauri::command(rename_all = "camelCase")]
pub async fn discover_workspace_skills(
    state: tauri::State<'_, AppState>,
    force_refresh: Option<bool>,
) -> AppResult<skill_discovery::SkillDiscoveryResult> {
    let force = force_refresh.unwrap_or(false);

    // Resolve working directory
    let cwd = if let Ok(Some(setting)) = settings_repo::get_setting(state.inner(), "working_directory") {
        if !setting.value.is_empty() {
            setting.value
        } else {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".into())
        }
    } else {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".into())
    };

    // Check cache
    if !force {
        let cache = state.discovered_skills.lock().await;
        if let Some(ref cached) = *cache {
            if cached.scanned_directories.iter().any(|d| d.contains(&cwd)) {
                return Ok(cached.clone());
            }
        }
    }

    // Run discovery (blocking I/O)
    let cwd_clone = cwd.clone();
    let result = tokio::task::spawn_blocking(move || {
        skill_discovery::discover_skills(&cwd_clone)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    // Cache the result
    {
        let mut cache = state.discovered_skills.lock().await;
        *cache = Some(result.clone());
    }

    Ok(result)
}
