use tauri::Emitter;
use tokio_util::sync::CancellationToken;

use crate::chat_tool::bridge;
use crate::chat_tool::manager;
use crate::db::chat_tool_repo;
use crate::error::{AppError, AppResult};
use crate::models::chat_tool::{
    BridgeCommand, ChatTool, ChatToolContact, ChatToolMessage, CreateChatToolRequest,
    UpdateChatToolRequest,
};
use crate::state::AppState;

#[tauri::command(rename_all = "camelCase")]
pub async fn list_chat_tools(
    state: tauri::State<'_, AppState>,
    workspace_id: Option<String>,
) -> AppResult<Vec<ChatTool>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        chat_tool_repo::list_chat_tools(&state, workspace_id.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_chat_tool(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<ChatTool> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || chat_tool_repo::get_chat_tool(&state, &id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn create_chat_tool(
    state: tauri::State<'_, AppState>,
    request: CreateChatToolRequest,
) -> AppResult<ChatTool> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || chat_tool_repo::create_chat_tool(&state, request))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn update_chat_tool(
    state: tauri::State<'_, AppState>,
    id: String,
    request: UpdateChatToolRequest,
) -> AppResult<ChatTool> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || chat_tool_repo::update_chat_tool(&state, &id, request))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn delete_chat_tool(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    // Stop bridge if running
    {
        let mut cancellations = state.chat_tool_cancellations.lock().await;
        if let Some(token) = cancellations.remove(&id) {
            token.cancel();
        }
        let mut processes = state.chat_tool_processes.lock().await;
        if let Some(mut process) = processes.remove(&id) {
            let _ = manager::stop_bridge_process(&mut process).await;
        }
    }

    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || chat_tool_repo::delete_chat_tool(&state, &id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn start_chat_tool(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    // Check if already running
    {
        let processes = state.chat_tool_processes.lock().await;
        if processes.contains_key(&id) {
            return Err(AppError::InvalidRequest(format!(
                "Chat tool {} is already running",
                id
            )));
        }
    }

    // Get chat tool config
    let state_clone = state.inner().clone();
    let id_clone = id.clone();
    let chat_tool = tokio::task::spawn_blocking(move || {
        chat_tool_repo::get_chat_tool(&state_clone, &id_clone)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // Update status to starting
    let state_clone = state.inner().clone();
    let id_clone = id.clone();
    tokio::task::spawn_blocking(move || {
        chat_tool_repo::update_chat_tool_status(&state_clone, &id_clone, "starting", None)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // Spawn bridge process
    let spawn_result =
        manager::spawn_bridge(&id, &chat_tool.plugin_type, &chat_tool.config_json).await;

    let (process, stdout) = match spawn_result {
        Ok(r) => r,
        Err(e) => {
            // Revert status to error
            let state_clone = state.inner().clone();
            let id_clone = id.clone();
            let err_msg = e.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(
                    &state_clone,
                    &id_clone,
                    "error",
                    Some(&err_msg),
                )
            })
            .await;
            return Err(e);
        }
    };

    // Create cancellation token
    let cancel_token = CancellationToken::new();

    // Store process and token
    {
        let mut processes = state.chat_tool_processes.lock().await;
        processes.insert(id.clone(), process);
        let mut cancellations = state.chat_tool_cancellations.lock().await;
        cancellations.insert(id.clone(), cancel_token.clone());
    }

    // Start event loop in background
    let state_clone = state.inner().clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        bridge::run_bridge_event_loop(app, state_clone, id_clone, stdout, cancel_token).await;
    });

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn stop_chat_tool(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    // Cancel event loop
    {
        let mut cancellations = state.chat_tool_cancellations.lock().await;
        if let Some(token) = cancellations.remove(&id) {
            token.cancel();
        }
    }

    // Stop bridge process
    {
        let mut processes = state.chat_tool_processes.lock().await;
        if let Some(mut process) = processes.remove(&id) {
            manager::stop_bridge_process(&mut process).await?;
        }
    }

    // Clean up cached ACP session and task run for this chat tool
    {
        let mut sessions = state.chat_tool_acp_sessions.lock().await;
        sessions.remove(&id);
    }
    {
        let mut runs = state.chat_tool_task_runs.lock().await;
        runs.remove(&id);
    }
    {
        let mut processing = state.chat_tool_processing.lock().await;
        processing.remove(&id);
    }

    // Clear cached QR code
    {
        let mut qr_codes = state.chat_tool_qr_codes.lock().await;
        qr_codes.remove(&id);
    }

    // Update status in DB
    let state_clone = state.inner().clone();
    let id_clone = id.clone();
    tokio::task::spawn_blocking(move || {
        chat_tool_repo::update_chat_tool_status(&state_clone, &id_clone, "stopped", None)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    // Emit event so frontend updates immediately
    let _ = app.emit(
        "chat_tool:status_changed",
        serde_json::json!({
            "chatToolId": id,
            "status": "stopped",
            "message": null
        }),
    );

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn logout_chat_tool(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    // Send logout command to bridge process
    let processes = state.chat_tool_processes.lock().await;
    let process = processes.get(&id).ok_or_else(|| {
        AppError::InvalidRequest(format!("Chat tool {} is not running", id))
    })?;
    manager::send_bridge_command(process, &BridgeCommand::Logout).await?;
    drop(processes);

    // Clear cached QR code so a new one will be received
    {
        let mut qr_codes = state.chat_tool_qr_codes.lock().await;
        qr_codes.remove(&id);
    }

    // Clear ACP session (will be recreated on next message with new user)
    {
        let mut sessions = state.chat_tool_acp_sessions.lock().await;
        sessions.remove(&id);
    }

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_chat_tool_qr_code(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Option<String>> {
    let qr_codes = state.chat_tool_qr_codes.lock().await;
    Ok(qr_codes.get(&id).cloned())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_chat_tool_messages(
    state: tauri::State<'_, AppState>,
    chat_tool_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Vec<ChatToolMessage>> {
    let state = state.inner().clone();
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    tokio::task::spawn_blocking(move || {
        chat_tool_repo::list_chat_tool_messages(&state, &chat_tool_id, limit, offset)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn send_chat_tool_message(
    state: tauri::State<'_, AppState>,
    chat_tool_id: String,
    to_id: String,
    content: String,
    content_type: Option<String>,
) -> AppResult<()> {
    let ct = content_type.unwrap_or_else(|| "text".to_string());

    // Send through bridge
    let processes = state.chat_tool_processes.lock().await;
    let process = processes.get(&chat_tool_id).ok_or_else(|| {
        AppError::InvalidRequest(format!("Chat tool {} is not running", chat_tool_id))
    })?;

    let cmd = BridgeCommand::SendMessage {
        to_id: to_id.clone(),
        content: content.clone(),
        content_type: ct.clone(),
    };
    manager::send_bridge_command(process, &cmd).await?;
    drop(processes);

    // Save outgoing message
    let state_clone = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        chat_tool_repo::save_chat_tool_message(
            &state_clone,
            &chat_tool_id,
            "outgoing",
            Some(&to_id),
            None,
            &content,
            &ct,
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn list_chat_tool_contacts(
    state: tauri::State<'_, AppState>,
    chat_tool_id: String,
) -> AppResult<Vec<ChatToolContact>> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || chat_tool_repo::list_contacts(&state, &chat_tool_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn set_chat_tool_contact_blocked(
    state: tauri::State<'_, AppState>,
    contact_id: String,
    blocked: bool,
) -> AppResult<()> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        chat_tool_repo::set_contact_blocked(&state, &contact_id, blocked)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
