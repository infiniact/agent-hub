use crate::acp::orchestrator;
use crate::db::{agent_repo, task_run_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::AgentConfig;
use crate::models::task_run::{CreateTaskRunRequest, TaskAssignment, TaskRun};
use crate::state::AppState;
use tokio_util::sync::CancellationToken;

#[tauri::command(rename_all = "camelCase")]
pub async fn start_orchestration(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    request: CreateTaskRunRequest,
) -> AppResult<TaskRun> {
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
