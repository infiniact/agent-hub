use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::task_run::{TaskAssignment, TaskRun};
use crate::state::AppState;

fn row_to_task_run(row: &rusqlite::Row) -> rusqlite::Result<TaskRun> {
    Ok(TaskRun {
        id: row.get(0)?,
        title: row.get(1)?,
        user_prompt: row.get(2)?,
        control_hub_agent_id: row.get(3)?,
        status: row.get(4)?,
        task_plan_json: row.get(5)?,
        result_summary: row.get(6)?,
        total_tokens_in: row.get(7)?,
        total_tokens_out: row.get(8)?,
        total_cache_creation_tokens: row.get(9)?,
        total_cache_read_tokens: row.get(10)?,
        total_duration_ms: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        rating: row.get(14)?,
        schedule_type: row.get(15)?,
        scheduled_time: row.get(16)?,
        recurrence_pattern_json: row.get(17)?,
        next_run_at: row.get(18)?,
        is_paused: row.get::<_, i32>(19)? != 0,
        workspace_id: row.get(20)?,
    })
}

fn row_to_assignment(row: &rusqlite::Row) -> rusqlite::Result<TaskAssignment> {
    Ok(TaskAssignment {
        id: row.get(0)?,
        task_run_id: row.get(1)?,
        agent_id: row.get(2)?,
        agent_name: row.get(3)?,
        sequence_order: row.get(4)?,
        input_text: row.get(5)?,
        output_text: row.get(6)?,
        status: row.get(7)?,
        model_used: row.get(8)?,
        tokens_in: row.get(9)?,
        tokens_out: row.get(10)?,
        cache_creation_tokens: row.get(11)?,
        cache_read_tokens: row.get(12)?,
        started_at: row.get(13)?,
        completed_at: row.get(14)?,
        duration_ms: row.get(15)?,
        error_message: row.get(16)?,
        created_at: row.get(17)?,
    })
}

const TASK_RUN_COLS: &str = "id, title, user_prompt, control_hub_agent_id, status, task_plan_json, result_summary, total_tokens_in, total_tokens_out, total_cache_creation_tokens, total_cache_read_tokens, total_duration_ms, created_at, updated_at, rating, schedule_type, scheduled_time, recurrence_pattern, next_run_at, is_paused, workspace_id";
const ASSIGNMENT_COLS: &str = "id, task_run_id, agent_id, agent_name, sequence_order, input_text, output_text, status, model_used, tokens_in, tokens_out, cache_creation_tokens, cache_read_tokens, started_at, completed_at, duration_ms, error_message, created_at";

pub fn create_task_run(
    state: &AppState,
    id: &str,
    title: &str,
    user_prompt: &str,
    control_hub_agent_id: &str,
    status: &str,
    workspace_id: Option<&str>,
) -> AppResult<TaskRun> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "INSERT INTO task_runs (id, title, user_prompt, control_hub_agent_id, status, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, title, user_prompt, control_hub_agent_id, status, workspace_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    db.query_row(
        &format!("SELECT {TASK_RUN_COLS} FROM task_runs WHERE id = ?1"),
        params![id],
        |row| row_to_task_run(row),
    )
    .map_err(|e| AppError::Database(e.to_string()))
}

pub fn update_task_run_status(
    state: &AppState,
    id: &str,
    status: &str,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![status, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn update_task_run_plan(
    state: &AppState,
    id: &str,
    plan_json: &str,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET task_plan_json = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![plan_json, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn update_task_run_summary(
    state: &AppState,
    id: &str,
    summary: &str,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET result_summary = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![summary, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Rate a completed task run (1-5 stars)
pub fn rate_task_run(
    state: &AppState,
    id: &str,
    rating: i32,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET rating = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![rating, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn update_task_run_totals(
    state: &AppState,
    id: &str,
    tokens_in: i64,
    tokens_out: i64,
    cache_creation_tokens: i64,
    cache_read_tokens: i64,
    duration_ms: i64,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET total_tokens_in = ?1, total_tokens_out = ?2, total_cache_creation_tokens = ?3, total_cache_read_tokens = ?4, total_duration_ms = ?5, updated_at = datetime('now') WHERE id = ?6",
        params![tokens_in, tokens_out, cache_creation_tokens, cache_read_tokens, duration_ms, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn get_task_run(state: &AppState, id: &str) -> AppResult<TaskRun> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.query_row(
        &format!("SELECT {TASK_RUN_COLS} FROM task_runs WHERE id = ?1"),
        params![id],
        |row| row_to_task_run(row),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("TaskRun {id} not found")),
        _ => AppError::Database(e.to_string()),
    })
}

pub fn list_task_runs(state: &AppState, workspace_id: Option<&str>) -> AppResult<Vec<TaskRun>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(ws_id) = workspace_id {
        (
            format!("SELECT {TASK_RUN_COLS} FROM task_runs WHERE workspace_id = ?1 ORDER BY created_at DESC"),
            vec![Box::new(ws_id.to_string())],
        )
    } else {
        (
            format!("SELECT {TASK_RUN_COLS} FROM task_runs ORDER BY created_at DESC"),
            vec![],
        )
    };

    let mut stmt = db.prepare(&sql).map_err(|e| AppError::Database(e.to_string()))?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();

    let runs = stmt
        .query_map(params_refs.as_slice(), |row| row_to_task_run(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(runs)
}

pub fn create_task_assignment(
    state: &AppState,
    id: &str,
    task_run_id: &str,
    agent_id: &str,
    agent_name: &str,
    sequence_order: i64,
    input_text: &str,
) -> AppResult<TaskAssignment> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "INSERT INTO task_assignments (id, task_run_id, agent_id, agent_name, sequence_order, input_text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, task_run_id, agent_id, agent_name, sequence_order, input_text],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    db.query_row(
        &format!("SELECT {ASSIGNMENT_COLS} FROM task_assignments WHERE id = ?1"),
        params![id],
        |row| row_to_assignment(row),
    )
    .map_err(|e| AppError::Database(e.to_string()))
}

pub fn update_task_assignment(
    state: &AppState,
    id: &str,
    status: &str,
    output_text: Option<&str>,
    model_used: Option<&str>,
    tokens_in: i64,
    tokens_out: i64,
    cache_creation_tokens: i64,
    cache_read_tokens: i64,
    duration_ms: i64,
    error_message: Option<&str>,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let (started_at, completed_at) = match status {
        "running" => (Some(now.clone()), None),
        "completed" | "failed" | "skipped" => (None, Some(now)),
        _ => (None, None),
    };

    if status == "running" {
        db.execute(
            "UPDATE task_assignments SET status = ?1, started_at = COALESCE(started_at, ?2) WHERE id = ?3",
            params![status, started_at, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    } else {
        db.execute(
            "UPDATE task_assignments SET status=?1, output_text=?2, model_used=?3, tokens_in=?4, tokens_out=?5, cache_creation_tokens=?6, cache_read_tokens=?7, duration_ms=?8, error_message=?9, completed_at=?10 WHERE id=?11",
            params![status, output_text, model_used, tokens_in, tokens_out, cache_creation_tokens, cache_read_tokens, duration_ms, error_message, completed_at, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    Ok(())
}

pub fn list_assignments_for_run(state: &AppState, task_run_id: &str) -> AppResult<Vec<TaskAssignment>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!("SELECT {ASSIGNMENT_COLS} FROM task_assignments WHERE task_run_id = ?1 ORDER BY sequence_order"))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let assignments = stmt
        .query_map(params![task_run_id], |row| row_to_assignment(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(assignments)
}

/// List all task runs that are in non-terminal states (pending, analyzing, running, awaiting_confirmation).
/// Used on startup to find orphaned tasks that need to be resumed.
pub fn list_incomplete_task_runs(state: &AppState) -> AppResult<Vec<TaskRun>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!(
            "SELECT {TASK_RUN_COLS} FROM task_runs \
             WHERE status IN ('pending', 'analyzing', 'running', 'awaiting_confirmation') \
             ORDER BY created_at ASC"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let runs = stmt
        .query_map([], |row| row_to_task_run(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(runs)
}

// ============== Scheduling functions ==============

/// Update the schedule configuration for a task run
pub fn update_schedule(
    state: &AppState,
    task_run_id: &str,
    schedule_type: &str,
    scheduled_time: Option<&str>,
    recurrence_pattern_json: Option<&str>,
    next_run_at: Option<&str>,
) -> AppResult<TaskRun> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET schedule_type = ?1, scheduled_time = ?2, recurrence_pattern = ?3, next_run_at = ?4, is_paused = 0, updated_at = datetime('now') WHERE id = ?5",
        params![schedule_type, scheduled_time, recurrence_pattern_json, next_run_at, task_run_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    db.query_row(
        &format!("SELECT {TASK_RUN_COLS} FROM task_runs WHERE id = ?1"),
        params![task_run_id],
        |row| row_to_task_run(row),
    )
    .map_err(|e| AppError::Database(e.to_string()))
}

/// Clear the schedule for a task run
pub fn clear_schedule(state: &AppState, task_run_id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET schedule_type = 'none', scheduled_time = NULL, recurrence_pattern = NULL, next_run_at = NULL, is_paused = 0, updated_at = datetime('now') WHERE id = ?1",
        params![task_run_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Pause a scheduled task
pub fn pause_scheduled_task(state: &AppState, task_run_id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET is_paused = 1, updated_at = datetime('now') WHERE id = ?1",
        params![task_run_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Resume a paused scheduled task
pub fn resume_scheduled_task(state: &AppState, task_run_id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET is_paused = 0, updated_at = datetime('now') WHERE id = ?1",
        params![task_run_id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

/// Get all scheduled tasks that are due for execution
/// Returns tasks where next_run_at <= now and is_paused = 0
pub fn list_due_scheduled_tasks(state: &AppState) -> AppResult<Vec<TaskRun>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!(
            "SELECT {TASK_RUN_COLS} FROM task_runs \
             WHERE schedule_type != 'none' \
             AND is_paused = 0 \
             AND next_run_at IS NOT NULL \
             AND datetime(next_run_at) <= datetime('now') \
             ORDER BY next_run_at ASC"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let runs = stmt
        .query_map([], |row| row_to_task_run(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(runs)
}

/// Calculate the next run time based on recurrence pattern
/// Returns ISO 8601 datetime string
pub fn calculate_next_run(
    frequency: &str,
    time_str: &str,
    interval: i32,
    days_of_week: Option<&Vec<i32>>,
    day_of_month: Option<i32>,
    month: Option<i32>,
) -> Option<String> {
    use chrono::{Duration, Datelike};

    let now = chrono::Utc::now();
    let (hour, minute) = parse_time(time_str)?;

    let next = match frequency {
        "daily" => {
            // Calculate next daily occurrence
            let mut candidate = now.date_naive().and_hms_opt(hour, minute, 0)?;
            if candidate <= now.naive_utc() {
                candidate += Duration::days(interval as i64);
            } else {
                // Already past today, start from tomorrow with interval
                candidate = (now + Duration::days(1)).date_naive().and_hms_opt(hour, minute, 0)?;
            }
            candidate
        }
        "weekly" => {
            // Calculate next weekly occurrence
            let target_days = days_of_week?;
            let current_weekday = now.weekday().num_days_from_sunday() as i32;

            // Find the next matching day
            let mut best_diff = 7i32;
            for &day in target_days {
                let diff = ((day - current_weekday + 7) % 7) as i32;
                if diff > 0 && diff < best_diff {
                    best_diff = diff;
                }
            }

            // If no day found this week, use interval * 7 days
            if best_diff == 7 {
                best_diff = (interval * 7) as i32;
            }

            let mut candidate = (now + Duration::days(best_diff as i64))
                .date_naive()
                .and_hms_opt(hour, minute, 0)?;

            // If the time has already passed today for the target day, add interval weeks
            if candidate <= now.naive_utc() {
                candidate += Duration::days((interval * 7) as i64);
            }
            candidate
        }
        "monthly" => {
            // Calculate next monthly occurrence
            let target_day = day_of_month.unwrap_or(1).min(28) as u32;

            let mut candidate = now
                .date_naive()
                .with_day(target_day)?
                .and_hms_opt(hour, minute, 0)?;

            if candidate <= now.naive_utc() {
                // Move to next month
                let next_month = if now.month() == 12 {
                    now.with_month(1)?.with_year(now.year() + 1)?
                } else {
                    now.with_month(now.month() + 1)?
                };
                candidate = next_month
                    .date_naive()
                    .with_day(target_day)?
                    .and_hms_opt(hour, minute, 0)?;
            }
            candidate
        }
        "yearly" => {
            // Calculate next yearly occurrence
            let target_month = month.unwrap_or(1) as u32;
            let target_day = day_of_month.unwrap_or(1).min(28) as u32;

            let mut candidate = now
                .date_naive()
                .with_month(target_month)?
                .with_day(target_day)?
                .and_hms_opt(hour, minute, 0)?;

            if candidate <= now.naive_utc() {
                // Move to next year
                candidate = (now.with_year(now.year() + 1)?)
                    .date_naive()
                    .with_month(target_month)?
                    .with_day(target_day)?
                    .and_hms_opt(hour, minute, 0)?;
            }
            candidate
        }
        _ => return None,
    };

    Some(next.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// Parse time string in HH:MM format
fn parse_time(time_str: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

/// Update next_run_at for a task after it has been executed
/// For one-time tasks, this clears the schedule
/// For recurring tasks, this calculates the next run time
pub fn update_next_run_after_execution(state: &AppState, task_run_id: &str) -> AppResult<()> {
    let task = get_task_run(state, task_run_id)?;

    if task.schedule_type == "none" {
        return Ok(());
    }

    if task.schedule_type == "once" {
        // One-time task completed, clear the schedule
        return clear_schedule(state, task_run_id);
    }

    // Recurring task - calculate next run
    if let Some(pattern_json) = &task.recurrence_pattern_json {
        let pattern: crate::models::task_run::RecurrencePattern =
            serde_json::from_str(pattern_json)
                .map_err(|e| AppError::Database(format!("Invalid recurrence pattern: {}", e)))?;

        let next_run = calculate_next_run(
            &pattern.frequency,
            &pattern.time,
            pattern.interval,
            pattern.days_of_week.as_ref(),
            pattern.day_of_month,
            pattern.month,
        );

        let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
        db.execute(
            "UPDATE task_runs SET next_run_at = ?1, status = 'completed', updated_at = datetime('now') WHERE id = ?2",
            params![next_run, task_run_id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    Ok(())
}
