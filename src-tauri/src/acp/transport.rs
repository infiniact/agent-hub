use serde::{Deserialize, Serialize};

use crate::acp::manager::AgentProcess;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Send a JSON-RPC message (request or notification) to the agent via NDJSON over stdin.
pub async fn send_message(process: &mut AgentProcess, msg: &JsonRpcRequest) -> AppResult<()> {
    let json = serde_json::to_string(msg).map_err(AppError::Serde)?;

    // Lock the stdin for writing
    let mut stdin = process.stdin.lock().await;
    use tokio::io::AsyncWriteExt;

    stdin
        .write_all(json.as_bytes())
        .await
        .map_err(|e| AppError::Transport(format!("Failed to write to agent stdin: {e}")))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| AppError::Transport(format!("Failed to write newline: {e}")))?;
    stdin
        .flush()
        .await
        .map_err(|e| AppError::Transport(format!("Failed to flush agent stdin: {e}")))?;

    Ok(())
}

/// Receive the next JSON-RPC message from the agent's stdout.
pub async fn receive_message(process: &mut AgentProcess) -> AppResult<serde_json::Value> {
    process
        .message_rx
        .recv()
        .await
        .ok_or_else(|| AppError::Transport("Agent stdout channel closed".into()))
}

/// Receive a JSON-RPC response matching a specific request ID.
/// Skips over notifications (messages with a `method` field but no matching `id`).
/// Has a timeout to avoid hanging forever if the agent doesn't respond.
pub async fn receive_response(
    process: &mut AgentProcess,
    expected_id: &serde_json::Value,
) -> AppResult<serde_json::Value> {
    let timeout = std::time::Duration::from_secs(30);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        match tokio::time::timeout_at(deadline, process.message_rx.recv()).await {
            Ok(Some(msg)) => {
                // Check if this message has an `id` that matches our expected response
                if let Some(msg_id) = msg.get("id") {
                    if msg_id == expected_id {
                        return Ok(msg);
                    }
                    // Different ID — log and skip
                    log::debug!(
                        "Skipping response with id={}, waiting for id={}",
                        msg_id,
                        expected_id
                    );
                } else {
                    // No `id` field — this is a notification, skip it
                    let method = msg
                        .get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown");
                    log::debug!("Skipping notification '{}' while waiting for response", method);
                }
            }
            Ok(None) => {
                return Err(AppError::Transport(
                    "Agent stdout channel closed while waiting for response".into(),
                ));
            }
            Err(_) => {
                return Err(AppError::Transport(format!(
                    "Timeout ({}s) waiting for response with id={}",
                    timeout.as_secs(),
                    expected_id
                )));
            }
        }
    }
}

/// Build a JSON-RPC 2.0 request.
pub fn build_request(
    id: impl Into<serde_json::Value>,
    method: &str,
    params: Option<serde_json::Value>,
) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(id.into()),
        method: method.into(),
        params,
    }
}

/// Build a JSON-RPC 2.0 notification (no id).
pub fn build_notification(method: &str, params: Option<serde_json::Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: None,
        method: method.into(),
        params,
    }
}
