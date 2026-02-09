use serde_json::json;

use crate::acp::transport::{self, JsonRpcResponse};
use crate::error::AppResult;

/// Handle fs/read_text_file request from agent.
pub async fn handle_read_text_file(
    request_id: serde_json::Value,
    params: &serde_json::Value,
) -> AppResult<JsonRpcResponse> {
    let path = params
        .get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("");

    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            result: Some(json!({ "content": content })),
            error: None,
        }),
        Err(e) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            result: None,
            error: Some(transport::JsonRpcError {
                code: -32000,
                message: format!("Failed to read file: {e}"),
                data: None,
            }),
        }),
    }
}

/// Handle fs/write_text_file request from agent.
pub async fn handle_write_text_file(
    request_id: serde_json::Value,
    params: &serde_json::Value,
) -> AppResult<JsonRpcResponse> {
    let path = params
        .get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("");
    let content = params
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    match tokio::fs::write(path, content).await {
        Ok(()) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            result: Some(json!({ "success": true })),
            error: None,
        }),
        Err(e) => Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            result: None,
            error: Some(transport::JsonRpcError {
                code: -32000,
                message: format!("Failed to write file: {e}"),
                data: None,
            }),
        }),
    }
}
