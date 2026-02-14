use serde_json::json;

use crate::acp::transport::{self, JsonRpcResponse};
use crate::error::AppResult;

/// Check if a path is allowed given the trusted working directory.
///
/// Allowed paths:
/// - Inside the trusted working directory (primary workspace)
/// - Inside `~/.iaagenthub/` (app data / agent metadata)
/// - Inside the system temp directory
/// - If no trusted directory is set, allow all paths (backwards compatible)
pub fn is_path_allowed(path: &str, trusted_dir: Option<&str>) -> bool {
    let Some(trusted) = trusted_dir else {
        return true;
    };

    if trusted.is_empty() {
        return true;
    }

    let canonical = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => {
            // For new files that don't exist yet, check the parent directory
            let parent = std::path::Path::new(path).parent();
            match parent.and_then(|p| std::fs::canonicalize(p).ok()) {
                Some(p) => p,
                None => return false,
            }
        }
    };

    // Check trusted working directory
    if let Ok(trusted_canonical) = std::fs::canonicalize(trusted) {
        if canonical.starts_with(&trusted_canonical) {
            return true;
        }
    }

    // Check app data directory (~/.iaagenthub/)
    if let Some(home) = dirs::home_dir() {
        let app_data = home.join(".iaagenthub");
        if let Ok(app_canonical) = std::fs::canonicalize(&app_data) {
            if canonical.starts_with(&app_canonical) {
                return true;
            }
        }
    }

    // Check system temp directory
    let temp = std::env::temp_dir();
    if let Ok(temp_canonical) = std::fs::canonicalize(&temp) {
        if canonical.starts_with(&temp_canonical) {
            return true;
        }
    }

    false
}

/// Handle fs/read_text_file request from agent.
pub async fn handle_read_text_file(
    request_id: serde_json::Value,
    params: &serde_json::Value,
    trusted_dir: Option<&str>,
) -> AppResult<JsonRpcResponse> {
    let path = params
        .get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("");

    if !is_path_allowed(path, trusted_dir) {
        return Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            result: None,
            error: Some(transport::JsonRpcError {
                code: -32000,
                message: "Access denied: path is outside the trusted working directory".into(),
                data: None,
            }),
        });
    }

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
    trusted_dir: Option<&str>,
) -> AppResult<JsonRpcResponse> {
    let path = params
        .get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("");
    let content = params
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");

    if !is_path_allowed(path, trusted_dir) {
        return Ok(JsonRpcResponse {
            jsonrpc: "2.0".into(),
            id: Some(request_id),
            result: None,
            error: Some(transport::JsonRpcError {
                code: -32000,
                message: "Access denied: path is outside the trusted working directory".into(),
                data: None,
            }),
        });
    }

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
