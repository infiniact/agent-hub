use rusqlite::params;

use crate::error::{AppError, AppResult};
use crate::models::settings::AppSettings;
use crate::state::AppState;

pub fn get_setting(state: &AppState, key: &str) -> AppResult<Option<AppSettings>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let result = db.query_row(
        "SELECT key, value, updated_at FROM settings WHERE key = ?1",
        params![key],
        |row| {
            Ok(AppSettings {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        },
    );

    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(AppError::Database(e.to_string())),
    }
}

pub fn set_setting(state: &AppState, key: &str, value: &str) -> AppResult<()> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    db.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, datetime('now'))",
        params![key, value],
    )
    .map_err(|e| AppError::Database(e.to_string()))?;
    Ok(())
}

pub fn get_all_settings(state: &AppState) -> AppResult<Vec<AppSettings>> {
    let db = state.db.lock().map_err(|e| AppError::Database(e.to_string()))?;
    let mut stmt = db
        .prepare("SELECT key, value, updated_at FROM settings ORDER BY key")
        .map_err(|e| AppError::Database(e.to_string()))?;

    let settings = stmt
        .query_map([], |row| {
            Ok(AppSettings {
                key: row.get(0)?,
                value: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })
        .map_err(|e| AppError::Database(e.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database(e.to_string()))?;

    Ok(settings)
}
