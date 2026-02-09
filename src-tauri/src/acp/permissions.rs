use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::acp::transport;
use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub agent_id: String,
    pub session_id: String,
    pub permission_id: String,
    pub description: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionResponse {
    pub permission_id: String,
    pub granted: bool,
}

/// Emit a permission request to the frontend and return when the user responds.
pub fn emit_permission_request(app: &AppHandle, request: &PermissionRequest) -> AppResult<()> {
    app.emit("acp:permission_request", request)
        .map_err(|e| crate::error::AppError::Internal(format!("Failed to emit permission request: {e}")))?;
    Ok(())
}

/// Build a JSON-RPC response for a permission request.
pub fn build_permission_response(
    request_id: serde_json::Value,
    granted: bool,
) -> transport::JsonRpcResponse {
    transport::JsonRpcResponse {
        jsonrpc: "2.0".into(),
        id: Some(request_id),
        result: Some(json!({ "granted": granted })),
        error: None,
    }
}
