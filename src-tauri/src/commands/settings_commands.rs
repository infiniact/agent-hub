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

/// Open native folder picker and persist the selected path as working directory.
#[tauri::command(rename_all = "camelCase")]
pub async fn select_working_directory(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;

    let (tx, rx) = tokio::sync::oneshot::channel();

    app.dialog()
        .file()
        .set_title("Select Workspace Folder")
        .pick_folder(move |folder| {
            let path = folder.map(|f| f.to_string());
            let _ = tx.send(path);
        });

    let selected = rx
        .await
        .map_err(|_| AppError::Internal("Dialog channel closed".into()))?;

    if let Some(ref path) = selected {
        let state_clone = state.inner().clone();
        let path_clone = path.clone();
        tokio::task::spawn_blocking(move || {
            settings_repo::set_setting(&state_clone, "working_directory", &path_clone)
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    Ok(selected)
}

/// Get the current working directory setting.
#[tauri::command(rename_all = "camelCase")]
pub async fn get_working_directory(
    state: tauri::State<'_, AppState>,
) -> AppResult<Option<String>> {
    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let setting = settings_repo::get_setting(&state_clone, "working_directory")?;
        Ok(setting.map(|s| s.value))
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Resolve the effective working directory.
/// Returns the user-configured trusted directory, or falls back to current_dir().
pub fn resolve_working_directory(state: &AppState) -> String {
    if let Ok(Some(setting)) = settings_repo::get_setting(state, "working_directory") {
        if !setting.value.is_empty() {
            return setting.value;
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| ".".into())
}
