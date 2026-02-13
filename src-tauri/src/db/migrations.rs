use std::path::PathBuf;
use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub fn get_base_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".iaagenthub")
}

pub fn get_db_path() -> PathBuf {
    get_base_dir().join("iagenthub.db")
}

pub fn get_agents_dir() -> PathBuf {
    get_base_dir().join("agents")
}

pub fn get_output_dir() -> PathBuf {
    get_base_dir().join("output")
}

pub fn init_db() -> AppResult<Connection> {
    let base_dir = get_base_dir();
    std::fs::create_dir_all(&base_dir).ok();
    std::fs::create_dir_all(get_agents_dir()).ok();
    std::fs::create_dir_all(get_output_dir()).ok();

    let path = get_db_path();
    let conn = Connection::open(&path)
        .map_err(|e| AppError::Database(format!("Failed to open database: {e}")))?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| AppError::Database(format!("Failed to set pragmas: {e}")))?;

    // Create migration tracking table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );"
    )
    .map_err(|e| AppError::Database(format!("Failed to create migrations table: {e}")))?;

    run_migrations(&conn)?;

    Ok(conn)
}

fn run_migrations(conn: &Connection) -> AppResult<()> {
    let migrations: Vec<(&str, &str)> = vec![
        ("001_initial", include_str!("../../migrations/001_initial.sql")),
        ("002_orchestration", include_str!("../../migrations/002_orchestration.sql")),
        ("003_agent_features", include_str!("../../migrations/003_agent_features.sql")),
        ("004_fix_status_constraints", include_str!("../../migrations/004_fix_status_constraints.sql")),
        ("005_agent_enabled", include_str!("../../migrations/005_agent_enabled.sql")),
        ("006_cache_tokens", include_str!("../../migrations/006_cache_tokens.sql")),
        ("007_rating", include_str!("../../migrations/007_rating.sql")),
        ("008_scheduling", include_str!("../../migrations/008_scheduling.sql")),
    ];

    for (name, sql) in migrations {
        let already_applied: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM _migrations WHERE name = ?1",
                rusqlite::params![name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !already_applied {
            conn.execute_batch(sql)
                .map_err(|e| AppError::Database(format!("Migration '{name}' failed: {e}")))?;

            conn.execute(
                "INSERT INTO _migrations (name) VALUES (?1)",
                rusqlite::params![name],
            )
            .map_err(|e| AppError::Database(format!("Failed to record migration '{name}': {e}")))?;

            log::info!("Applied migration: {}", name);
        }
    }

    Ok(())
}
