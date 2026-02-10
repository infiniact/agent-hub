use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::agent::{AgentConfig, CreateAgentRequest, DiscoveredAgent, UpdateAgentRequest};
use crate::state::AppState;

fn row_to_agent(row: &rusqlite::Row) -> rusqlite::Result<AgentConfig> {
    Ok(AgentConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        icon: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        execution_mode: row.get(5)?,
        model: row.get(6)?,
        temperature: row.get(7)?,
        max_tokens: row.get(8)?,
        system_prompt: row.get(9)?,
        capabilities_json: row.get(10)?,
        acp_command: row.get(11)?,
        acp_args_json: row.get(12)?,
        is_control_hub: row.get::<_, i32>(13)? != 0,
        md_file_path: row.get(14)?,
        max_concurrency: row.get(15)?,
        available_models_json: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

const SELECT_COLS: &str = "id, name, icon, description, status, execution_mode, model, temperature, max_tokens, system_prompt, capabilities_json, acp_command, acp_args_json, is_control_hub, md_file_path, max_concurrency, available_models_json, created_at, updated_at";

pub fn list_agents(state: &AppState) -> AppResult<Vec<AgentConfig>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare(&format!("SELECT {SELECT_COLS} FROM agents ORDER BY created_at DESC"))
        .map_err(|e| AppError::Database(e.to_string()))?;

    let agents = stmt
        .query_map([], |row| row_to_agent(row))
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(agents)
}

pub fn get_agent(state: &AppState, id: &str) -> AppResult<AgentConfig> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.query_row(
        &format!("SELECT {SELECT_COLS} FROM agents WHERE id = ?1"),
        params![id],
        |row| row_to_agent(row),
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("Agent {id} not found")),
        _ => AppError::Database(e.to_string()),
    })
}

pub fn create_agent(state: &AppState, req: CreateAgentRequest) -> AppResult<AgentConfig> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    db.execute(
        "INSERT INTO agents (id, name, icon, description, execution_mode, model, temperature, max_tokens, system_prompt, capabilities_json, acp_command, acp_args_json, is_control_hub, max_concurrency) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            id,
            req.name,
            req.icon,
            req.description,
            req.execution_mode,
            req.model,
            req.temperature,
            req.max_tokens,
            req.system_prompt,
            req.capabilities_json,
            req.acp_command,
            req.acp_args_json,
            req.is_control_hub as i32,
            req.max_concurrency,
        ],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    drop(db);
    get_agent(state, &id)
}

pub fn update_agent(state: &AppState, id: &str, req: UpdateAgentRequest) -> AppResult<AgentConfig> {
    let existing = get_agent(state, id)?;
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    let name = req.name.unwrap_or(existing.name);
    let icon = req.icon.unwrap_or(existing.icon);
    let description = req.description.unwrap_or(existing.description);
    let status = req.status.unwrap_or(existing.status);
    let execution_mode = req.execution_mode.unwrap_or(existing.execution_mode);
    let model = req.model.unwrap_or(existing.model);
    let temperature = req.temperature.unwrap_or(existing.temperature);
    let max_tokens = req.max_tokens.unwrap_or(existing.max_tokens);
    let system_prompt = req.system_prompt.unwrap_or(existing.system_prompt);
    let capabilities_json = req.capabilities_json.unwrap_or(existing.capabilities_json);
    let acp_command = req.acp_command.or(existing.acp_command);
    let acp_args_json = req.acp_args_json.or(existing.acp_args_json);
    let is_control_hub = req.is_control_hub.unwrap_or(existing.is_control_hub);
    let max_concurrency = req.max_concurrency.unwrap_or(existing.max_concurrency);
    let available_models_json = req.available_models_json.or(existing.available_models_json);

    db.execute(
        "UPDATE agents SET name=?1, icon=?2, description=?3, status=?4, execution_mode=?5, model=?6, temperature=?7, max_tokens=?8, system_prompt=?9, capabilities_json=?10, acp_command=?11, acp_args_json=?12, is_control_hub=?13, max_concurrency=?14, available_models_json=?15, updated_at=datetime('now') WHERE id=?16",
        params![name, icon, description, status, execution_mode, model, temperature, max_tokens, system_prompt, capabilities_json, acp_command, acp_args_json, is_control_hub as i32, max_concurrency, available_models_json, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    drop(db);
    get_agent(state, id)
}

pub fn set_control_hub(state: &AppState, id: &str) -> AppResult<AgentConfig> {
    // Verify agent exists
    let _ = get_agent(state, id)?;

    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;

    // Clear all control hub flags first
    db.execute("UPDATE agents SET is_control_hub = 0", [])
        .map_err(|e| AppError::Database(e.to_string()))?;

    // Set the specified agent as control hub
    db.execute(
        "UPDATE agents SET is_control_hub = 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;

    drop(db);
    get_agent(state, id)
}

pub fn get_control_hub(state: &AppState) -> AppResult<Option<AgentConfig>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let result = db.query_row(
        &format!("SELECT {SELECT_COLS} FROM agents WHERE is_control_hub = 1 LIMIT 1"),
        [],
        |row| row_to_agent(row),
    );

    match result {
        Ok(agent) => Ok(Some(agent)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e.to_string())),
    }
}

pub fn update_agent_md_path(state: &AppState, id: &str, md_path: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "UPDATE agents SET md_file_path = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![md_path, id],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn delete_agent(state: &AppState, id: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute("DELETE FROM agents WHERE id = ?1", params![id])
        .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn save_discovered_agent(state: &AppState, agent: &DiscoveredAgent) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "INSERT OR REPLACE INTO discovered_agents (id, name, command, args_json, env_json, source_path, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
        params![agent.id, agent.name, agent.command, agent.args_json, agent.env_json, agent.source_path],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn list_discovered_agents(state: &AppState) -> AppResult<Vec<DiscoveredAgent>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare("SELECT id, name, command, args_json, env_json, source_path, last_seen_at FROM discovered_agents ORDER BY name")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let agents = stmt
        .query_map([], |row| {
            Ok(DiscoveredAgent {
                id: row.get(0)?,
                name: row.get(1)?,
                command: row.get(2)?,
                args_json: row.get(3)?,
                env_json: row.get(4)?,
                source_path: row.get(5)?,
                last_seen_at: row.get(6)?,
                available: false,
                models: Vec::new(),
                registry_id: None,
                icon_url: None,
                description: String::new(),
            })
        })
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(agents)
}
