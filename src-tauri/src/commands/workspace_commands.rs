use crate::db::{settings_repo, workspace_repo};
use crate::error::{AppError, AppResult};
use crate::models::workspace::{CreateWorkspaceRequest, UpdateWorkspaceRequest, Workspace};
use crate::state::AppState;

#[tauri::command]
pub async fn list_workspaces(state: tauri::State<'_, AppState>) -> AppResult<Vec<Workspace>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || workspace_repo::list_workspaces(&state))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn create_workspace(
    state: tauri::State<'_, AppState>,
    request: CreateWorkspaceRequest,
) -> AppResult<Workspace> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || workspace_repo::create_workspace(&state, request))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_workspace(
    state: tauri::State<'_, AppState>,
    id: String,
    request: UpdateWorkspaceRequest,
) -> AppResult<Workspace> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || workspace_repo::update_workspace(&state, &id, request))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
pub async fn delete_workspace(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || workspace_repo::delete_workspace(&state, &id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn select_workspace_directory(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    workspace_id: String,
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
        let ws_id = workspace_id.clone();
        let p = path.clone();
        tokio::task::spawn_blocking(move || {
            workspace_repo::update_workspace(
                &state_clone,
                &ws_id,
                UpdateWorkspaceRequest {
                    name: None,
                    icon: None,
                    working_directory: Some(p),
                },
            )
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;

        // Also update the global working_directory setting to keep it in sync
        let state_clone = state.inner().clone();
        let p2 = path.clone();
        tokio::task::spawn_blocking(move || {
            settings_repo::set_setting(&state_clone, "working_directory", &p2)
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    }

    Ok(selected)
}
