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
        total_duration_ms: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
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
        started_at: row.get(11)?,
        completed_at: row.get(12)?,
        duration_ms: row.get(13)?,
        error_message: row.get(14)?,
        created_at: row.get(15)?,
    })
}

const TASK_RUN_COLS: &str = "id, title, user_prompt, control_hub_agent_id, status, task_plan_json, result_summary, total_tokens_in, total_tokens_out, total_duration_ms, created_at, updated_at";
const ASSIGNMENT_COLS: &str = "id, task_run_id, agent_id, agent_name, sequence_order, input_text, output_text, status, model_used, tokens_in, tokens_out, started_at, completed_at, duration_ms, error_message, created_at";

pub fn create_task_run(
    state: &AppState,
    id: &str,
    title: &str,
    user_prompt: &str,
    control_hub_agent_id: &str,
    status: &str,
) -> AppResult<TaskRun> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "INSERT INTO task_runs (id, title, user_prompt, control_hub_agent_id, status) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, user_prompt, control_hub_agent_id, status],
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

pub fn update_task_run_totals(
    state: &AppState,
    id: &str,
    tokens_in: i64,
    tokens_out: i64,
    duration_ms: i64,
) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE task_runs SET total_tokens_in = ?1, total_tokens_out = ?2, total_duration_ms = ?3, updated_at = datetime('now') WHERE id = ?4",
        params![tokens_in, tokens_out, duration_ms, id],
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

pub fn list_task_runs(state: &AppState) -> AppResult<Vec<TaskRun>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!("SELECT {TASK_RUN_COLS} FROM task_runs ORDER BY created_at DESC"))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let runs = stmt
        .query_map([], |row| row_to_task_run(row))
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
            "UPDATE task_assignments SET status=?1, output_text=?2, model_used=?3, tokens_in=?4, tokens_out=?5, duration_ms=?6, error_message=?7, completed_at=?8 WHERE id=?9",
            params![status, output_text, model_used, tokens_in, tokens_out, duration_ms, error_message, completed_at, id],
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
