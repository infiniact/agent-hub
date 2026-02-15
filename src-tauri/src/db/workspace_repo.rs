use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::workspace::{CreateWorkspaceRequest, UpdateWorkspaceRequest, Workspace};
use crate::state::AppState;

fn row_to_workspace(row: &rusqlite::Row) -> rusqlite::Result<Workspace> {
    Ok(Workspace {
        id: row.get(0)?,
        name: row.get(1)?,
        icon: row.get(2)?,
        working_directory: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

const WORKSPACE_COLS: &str = "id, name, icon, working_directory, created_at, updated_at";

pub fn list_workspaces(state: &AppState) -> AppResult<Vec<Workspace>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!(
            "SELECT {WORKSPACE_COLS} FROM workspaces ORDER BY created_at ASC"
        ))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let workspaces = stmt
        .query_map([], |row| row_to_workspace(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(workspaces)
}

pub fn get_workspace(state: &AppState, id: &str) -> AppResult<Workspace> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.query_row(
        &format!("SELECT {WORKSPACE_COLS} FROM workspaces WHERE id = ?1"),
        params![id],
        |row| row_to_workspace(row),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::NotFound(format!("Workspace {id} not found"))
        }
        _ => AppError::Database(e.to_string()),
    })
}

pub fn create_workspace(state: &AppState, req: CreateWorkspaceRequest) -> AppResult<Workspace> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    db.execute(
        "INSERT INTO workspaces (id, name, icon, working_directory) VALUES (?1, ?2, ?3, ?4)",
        params![id, req.name, req.icon, req.working_directory],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    // Clone agent templates into the new workspace as independent instances
    for template_id in &req.agent_ids {
        // Read the template agent
        let mut stmt = db
            .prepare("SELECT name, icon, description, execution_mode, model, temperature, max_tokens, system_prompt, capabilities_json, skills_json, acp_command, acp_args_json, is_control_hub, max_concurrency FROM agents WHERE id = ?1")
            .map_err(|e| AppError::Database(e.to_string()))?;

        let cloned = stmt.query_row(params![template_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, i32>(12)?,
                row.get::<_, i64>(13)?,
            ))
        });

        if let Ok((name, icon, description, exec_mode, model, temp, max_tok, sys_prompt, caps, skills, acp_cmd, acp_args, is_hub, max_conc)) = cloned {
            let new_id = uuid::Uuid::new_v4().to_string();
            db.execute(
                "INSERT INTO agents (id, name, icon, description, execution_mode, model, temperature, max_tokens, system_prompt, capabilities_json, skills_json, acp_command, acp_args_json, is_control_hub, max_concurrency, workspace_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![new_id, name, icon, description, exec_mode, model, temp, max_tok, sys_prompt, caps, skills, acp_cmd, acp_args, is_hub, max_conc, id],
            )
            .map_err(|e| AppError::Database(e.to_string()))?;
        }
    }

    drop(db);
    get_workspace(state, &id)
}

pub fn update_workspace(
    state: &AppState,
    id: &str,
    req: UpdateWorkspaceRequest,
) -> AppResult<Workspace> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    if let Some(name) = &req.name {
        db.execute(
            "UPDATE workspaces SET name = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![name, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }
    if let Some(icon) = &req.icon {
        db.execute(
            "UPDATE workspaces SET icon = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![icon, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }
    if let Some(wd) = &req.working_directory {
        db.execute(
            "UPDATE workspaces SET working_directory = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![wd, id],
        )
        .map_err(|e| AppError::Database(e.to_string()))?;
    }

    drop(db);
    get_workspace(state, id)
}

pub fn delete_workspace(state: &AppState, id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    // Prevent deleting the last workspace
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
        .map_err(|e| AppError::Database(e.to_string()))?;

    if count <= 1 {
        return Err(AppError::InvalidRequest(
            "Cannot delete the last workspace".into(),
        ));
    }

    // Agents with this workspace_id will be CASCADE-deleted by the FK
    db.execute("DELETE FROM workspaces WHERE id = ?1", params![id])
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(())
}
