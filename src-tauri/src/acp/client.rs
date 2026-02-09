use serde_json::json;

use crate::acp::manager::AgentProcess;
use crate::acp::transport;
use crate::error::{AppError, AppResult};

/// Send the ACP `initialize` handshake to the agent.
pub async fn initialize_agent(process: &mut AgentProcess) -> AppResult<serde_json::Value> {
    let req = transport::build_request(
        1,
        "initialize",
        Some(json!({
            "protocolVersion": 1,
            "clientInfo": {
                "name": "IAAgentHub",
                "version": "0.1.0"
            },
            "clientCapabilities": {
                "fs": {
                    "readTextFile": false,
                    "writeTextFile": false
                },
                "terminal": false
            }
        })),
    );

    transport::send_message(process, &req).await?;

    // Wait for the initialize response, matching by request id
    let response = transport::receive_response(process, &json!(1)).await?;

    // Check for error in response
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::Acp(format!("Initialize failed: {}", msg)));
    }

    Ok(response)
}

/// Create a new ACP session with the agent.
pub async fn create_session(
    process: &mut AgentProcess,
    cwd: &str,
) -> AppResult<String> {
    let req = transport::build_request(
        2,
        "session/new",
        Some(json!({
            "cwd": cwd,
            "mcpServers": []
        })),
    );

    transport::send_message(process, &req).await?;
    let response = transport::receive_response(process, &json!(2)).await?;

    // Check for error in response
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::Acp(format!("session/new failed: {}", msg)));
    }

    let session_id = response
        .get("result")
        .and_then(|r| r.get("sessionId"))
        .and_then(|s| s.as_str())
        .ok_or_else(|| {
            let resp_str = serde_json::to_string(&response).unwrap_or_default();
            AppError::Acp(format!("No sessionId in session/new response: {}", resp_str))
        })?
        .to_string();

    Ok(session_id)
}

/// Send a text prompt within an ACP session.
pub async fn send_prompt(
    process: &mut AgentProcess,
    acp_session_id: &str,
    text: &str,
    request_id: i64,
) -> AppResult<()> {
    let req = transport::build_request(
        request_id,
        "session/prompt",
        Some(json!({
            "sessionId": acp_session_id,
            "prompt": [{
                "type": "text",
                "text": text
            }]
        })),
    );

    transport::send_message(process, &req).await
}

/// Cancel an in-flight prompt.
pub async fn cancel_prompt(
    process: &mut AgentProcess,
    acp_session_id: &str,
) -> AppResult<()> {
    let req = transport::build_notification(
        "session/cancel",
        Some(json!({
            "sessionId": acp_session_id
        })),
    );

    transport::send_message(process, &req).await
}
