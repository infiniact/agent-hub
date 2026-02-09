use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use serde_json::json;
use tokio::sync::Mutex;

use crate::acp::transport::{self, JsonRpcResponse};
use crate::error::AppResult;

/// Tracks spawned terminal processes.
pub struct TerminalManager {
    processes: Arc<Mutex<HashMap<String, tokio::process::Child>>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Handle terminal/create request from agent.
    pub async fn handle_create(
        &self,
        request_id: serde_json::Value,
        params: &serde_json::Value,
    ) -> AppResult<JsonRpcResponse> {
        let command = params
            .get("command")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        let args: Vec<String> = params
            .get("args")
            .and_then(|a| serde_json::from_value(a.clone()).ok())
            .unwrap_or_default();
        let cwd = params.get("cwd").and_then(|c| c.as_str());

        let shell = if cfg!(target_os = "windows") {
            "cmd.exe"
        } else {
            "/bin/sh"
        };
        let shell_flag = if cfg!(target_os = "windows") {
            "/C"
        } else {
            "-c"
        };

        let full_command = if args.is_empty() {
            command.to_string()
        } else {
            format!("{} {}", command, args.join(" "))
        };

        let mut cmd = tokio::process::Command::new(shell);
        cmd.arg(shell_flag).arg(&full_command);

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        match cmd.spawn() {
            Ok(child) => {
                let terminal_id = uuid::Uuid::new_v4().to_string();
                self.processes
                    .lock()
                    .await
                    .insert(terminal_id.clone(), child);
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: Some(request_id),
                    result: Some(json!({ "terminalId": terminal_id })),
                    error: None,
                })
            }
            Err(e) => Ok(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: Some(request_id),
                result: None,
                error: Some(transport::JsonRpcError {
                    code: -32000,
                    message: format!("Failed to create terminal: {e}"),
                    data: None,
                }),
            }),
        }
    }

    /// Handle terminal/kill request from agent.
    pub async fn handle_kill(
        &self,
        request_id: serde_json::Value,
        params: &serde_json::Value,
    ) -> AppResult<JsonRpcResponse> {
        let terminal_id = params
            .get("terminalId")
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let mut processes = self.processes.lock().await;
        if let Some(child) = processes.get_mut(terminal_id) {
            let _ = child.kill().await;
            processes.remove(terminal_id);
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: Some(request_id),
                result: Some(json!({ "success": true })),
                error: None,
            })
        } else {
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: Some(request_id),
                result: None,
                error: Some(transport::JsonRpcError {
                    code: -32001,
                    message: format!("Terminal {terminal_id} not found"),
                    data: None,
                }),
            })
        }
    }

    /// Handle terminal/wait_for_exit request from agent.
    pub async fn handle_wait_for_exit(
        &self,
        request_id: serde_json::Value,
        params: &serde_json::Value,
    ) -> AppResult<JsonRpcResponse> {
        let terminal_id = params
            .get("terminalId")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let mut processes = self.processes.lock().await;
        if let Some(child) = processes.get_mut(&terminal_id) {
            match child.wait().await {
                Ok(status) => {
                    let exit_code = status.code().unwrap_or(-1);
                    processes.remove(&terminal_id);
                    Ok(JsonRpcResponse {
                        jsonrpc: "2.0".into(),
                        id: Some(request_id),
                        result: Some(json!({ "exitCode": exit_code })),
                        error: None,
                    })
                }
                Err(e) => Ok(JsonRpcResponse {
                    jsonrpc: "2.0".into(),
                    id: Some(request_id),
                    result: None,
                    error: Some(transport::JsonRpcError {
                        code: -32000,
                        message: format!("Failed to wait for terminal: {e}"),
                        data: None,
                    }),
                }),
            }
        } else {
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: Some(request_id),
                result: None,
                error: Some(transport::JsonRpcError {
                    code: -32001,
                    message: format!("Terminal {terminal_id} not found"),
                    data: None,
                }),
            })
        }
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}
