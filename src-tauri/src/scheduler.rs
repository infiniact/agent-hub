//! Task scheduler for executing scheduled and recurring tasks
//!
//! This module provides a background scheduler that checks for due tasks
//! and executes them via the orchestration system.

use tauri::AppHandle;
use tokio_util::sync::CancellationToken;

use crate::acp::orchestrator;
use crate::db::task_run_repo;
use crate::error::AppResult;
use crate::state::AppState;

/// Scheduler state for managing the background task
pub struct SchedulerState {
    /// Cancellation token to stop the scheduler
    cancel_token: CancellationToken,
    /// Handle to the scheduler task (for joining on shutdown)
    #[allow(dead_code)]
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl SchedulerState {
    pub fn new(cancel_token: CancellationToken, task_handle: tokio::task::JoinHandle<()>) -> Self {
        Self {
            cancel_token,
            task_handle: Some(task_handle),
        }
    }

    /// Stop the scheduler
    pub fn stop(&mut self) {
        self.cancel_token.cancel();
    }
}

/// Start the background scheduler
///
/// The scheduler runs every minute to check for due scheduled tasks.
/// When a task is due, it executes it and updates the next_run_at for recurring tasks.
pub fn start_scheduler(app: AppHandle, state: AppState) -> SchedulerState {
    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    let task_handle = tokio::spawn(async move {
        log::info!("[Scheduler] Starting task scheduler");

        loop {
            // Check every 60 seconds
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(60)) => {
                    if let Err(e) = check_and_execute_scheduled_tasks(&app, &state).await {
                        log::error!("[Scheduler] Error checking scheduled tasks: {:?}", e);
                    }
                }
                _ = cancel_token_clone.cancelled() => {
                    log::info!("[Scheduler] Scheduler stopped");
                    break;
                }
            }
        }
    });

    SchedulerState::new(cancel_token, task_handle)
}

/// Check for and execute due scheduled tasks
async fn check_and_execute_scheduled_tasks(app: &AppHandle, state: &AppState) -> AppResult<()> {
    let state_clone = state.clone();
    let due_tasks = tokio::task::spawn_blocking(move || {
        task_run_repo::list_due_scheduled_tasks(&state_clone)
    })
    .await
    .map_err(|e| crate::error::AppError::Internal(e.to_string()))??;

    if !due_tasks.is_empty() {
        log::info!("[Scheduler] Found {} due scheduled tasks", due_tasks.len());
    }

    for task in due_tasks {
        log::info!(
            "[Scheduler] Executing scheduled task: {} ({})",
            task.title,
            task.id
        );

        // Execute the task via the orchestrator
        let app_clone = app.clone();
        let state_clone = state.clone();
        let task_id = task.id.clone();

        // Clone task data for the async block
        let task_clone = task.clone();

        tokio::spawn(async move {
            // Reset task status to pending before execution
            {
                let state = state_clone.clone();
                let tid = task_id.clone();
                if let Err(e) = tokio::task::spawn_blocking(move || {
                    task_run_repo::update_task_run_status(&state, &tid, "pending")
                })
                .await
                {
                    log::error!("[Scheduler] Failed to reset task status: {:?}", e);
                    return;
                }
            }

            // Run orchestration
            let ws_id = task_clone.workspace_id.clone();
            orchestrator::run_orchestration(app_clone, state_clone.clone(), task_id, task_clone.user_prompt, ws_id).await;

            // After completion, update next_run_at for recurring tasks
            let state = state_clone.clone();
            let tid = task_clone.id.clone();
            if let Err(e) = tokio::task::spawn_blocking(move || {
                task_run_repo::update_next_run_after_execution(&state, &tid)
            })
            .await
            {
                log::error!("[Scheduler] Failed to update next run time: {:?}", e);
            }
        });
    }

    Ok(())
}

/// Calculate the next run time for display purposes
pub fn calculate_next_run_display(
    frequency: &str,
    time_str: &str,
    interval: i32,
    days_of_week: Option<&Vec<i32>>,
    day_of_month: Option<i32>,
    month: Option<i32>,
) -> Option<String> {
    task_run_repo::calculate_next_run(frequency, time_str, interval, days_of_week, day_of_month, month)
}
