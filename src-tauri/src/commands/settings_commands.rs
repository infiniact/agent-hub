use std::collections::HashMap;

use crate::db::settings_repo;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[tauri::command]
pub async fn get_settings(
    state: tauri::State<'_, AppState>,
) -> AppResult<HashMap<String, String>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let settings = settings_repo::get_all_settings(&state)?;
        Ok(settings.into_iter().map(|s| (s.key, s.value)).collect())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn update_settings(
    state: tauri::State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || settings_repo::set_setting(&state, &key, &value))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}
