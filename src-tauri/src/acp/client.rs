use serde_json::json;

use crate::acp::manager::AgentProcess;
use crate::acp::transport;
use crate::error::{AppError, AppResult};

/// Maximum number of retries for agent initialization.
const INIT_MAX_RETRIES: usize = 2;
/// Timeout for the initialize handshake (seconds). Agents like npx-based ones
/// can take a long time to start, so this is intentionally generous.
const INIT_TIMEOUT_SECS: u64 = 120;

/// Send the ACP `initialize` handshake to the agent.
/// Retries up to INIT_MAX_RETRIES times on transport timeouts, with progressively
/// longer timeouts. Includes stderr output in error messages for diagnostics.
pub async fn initialize_agent(process: &mut AgentProcess) -> AppResult<serde_json::Value> {
    let mut last_error: Option<AppError> = None;

    for attempt in 0..=INIT_MAX_RETRIES {
        if attempt > 0 {
            log::info!(
                "Retrying agent initialization (attempt {}/{})",
                attempt + 1,
                INIT_MAX_RETRIES + 1,
            );
            // Brief pause before retry to let the agent catch up
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

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

        if let Err(e) = transport::send_message(process, &req).await {
            last_error = Some(e);
            continue;
        }

        // Use a generous timeout for initialization
        match transport::receive_response_with_timeout(process, &json!(1), INIT_TIMEOUT_SECS).await
        {
            Ok(response) => {
                // Check for error in response
                if let Some(error) = response.get("error") {
                    let msg = error
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    return Err(AppError::Acp(format!("Initialize failed: {}", msg)));
                }

                if attempt > 0 {
                    log::info!(
                        "Agent initialization succeeded on attempt {}",
                        attempt + 1
                    );
                }
                return Ok(response);
            }
            Err(e) => {
                let is_timeout = matches!(&e, AppError::Transport(msg) if msg.contains("Timeout"));
                if is_timeout && attempt < INIT_MAX_RETRIES {
                    log::warn!(
                        "Agent initialization timed out (attempt {}/{}): {}",
                        attempt + 1,
                        INIT_MAX_RETRIES + 1,
                        e,
                    );
                    last_error = Some(e);
                    continue;
                }
                last_error = Some(e);
                break;
            }
        }
    }

    // All retries exhausted — build a detailed error message with stderr output
    let base_err = last_error
        .map(|e| e.to_string())
        .unwrap_or_else(|| "Unknown error".into());

    let stderr_output = {
        let lines = process.stderr_lines.lock().await;
        if lines.is_empty() {
            "(no stderr output captured)".to_string()
        } else {
            lines.join("\n")
        }
    };

    Err(AppError::Acp(format!(
        "Agent initialization failed after {} attempts.\nLast error: {}\nAgent stderr:\n{}",
        INIT_MAX_RETRIES + 1,
        base_err,
        stderr_output,
    )))
}

/// Get available models from an agent without creating a session.
/// This is a lightweight method to query available models before session creation.
/// Note: ACP doesn't have a separate "list_models" method, so we create a temporary session
/// and immediately end it after extracting models.
pub async fn get_available_models(
    process: &mut AgentProcess,
    cwd: &str,
) -> AppResult<Vec<AgentModel>> {
    log::info!("get_available_models: Creating temporary session to query models");

    let (_session_id, models) = create_session(process, cwd).await?;

    log::info!(
        "get_available_models: Retrieved {} models, will end temporary session",
        models.len()
    );

    // Note: We don't end the session here because the session may be reused
    // The caller can decide whether to end it or keep it for later use

    Ok(models)
}

/// Authentication method reported by the agent in the initialize response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthMethod {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// Extract auth methods from an initialize response.
pub fn extract_auth_methods(init_response: &serde_json::Value) -> Vec<AuthMethod> {
    init_response
        .get("result")
        .and_then(|r| r.get("authMethods"))
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let id = m.get("id")?.as_str()?.to_string();
                    let name = m.get("name")?.as_str()?.to_string();
                    let description = m.get("description").and_then(|d| d.as_str()).map(String::from);
                    Some(AuthMethod { id, name, description })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Start the ACP auth flow with a given auth method.
/// Returns the auth result which typically contains a URL for the user to visit.
pub async fn start_auth(
    process: &mut AgentProcess,
    method_id: &str,
) -> AppResult<serde_json::Value> {
    log::info!("start_auth: Starting auth with method '{}'", method_id);
    let req = transport::build_request(
        200,
        "auth/start",
        Some(json!({
            "methodId": method_id
        })),
    );

    transport::send_message(process, &req).await?;
    let response = transport::receive_response(process, &json!(200)).await?;

    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::Acp(format!("auth/start failed: {}", msg)));
    }

    log::info!("auth/start response: {}", serde_json::to_string(&response).unwrap_or_default());
    Ok(response)
}

/// Extract the login URL from an auth/start response.
pub fn extract_auth_url(auth_response: &serde_json::Value) -> Option<String> {
    // Try result.url
    if let Some(url) = auth_response.get("result").and_then(|r| r.get("url")).and_then(|u| u.as_str()) {
        return Some(url.to_string());
    }
    // Try result.uri
    if let Some(url) = auth_response.get("result").and_then(|r| r.get("uri")).and_then(|u| u.as_str()) {
        return Some(url.to_string());
    }
    // Search entire result string for any https:// URL
    if let Some(result) = auth_response.get("result") {
        let s = serde_json::to_string(result).unwrap_or_default();
        if let Some(start) = s.find("https://") {
            let rest = &s[start..];
            let end = rest.find(|c: char| c == '"' || c == '\'' || c == ' ' || c == '}' || c == ']')
                .unwrap_or(rest.len());
            let url = &rest[..end];
            if url.len() > 10 {
                return Some(url.to_string());
            }
        }
    }
    None
}

/// Model information from ACP session/new response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentModel {
    pub model_id: String,
    pub name: String,
    pub description: Option<String>,
}

/// Session history entry from ACP session/load response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionHistoryEntry {
    pub role: String,
    pub content: serde_json::Value,
    pub timestamp: Option<String>,
}

/// ACP session state tracking whether a session is active or has ended.
#[derive(Debug, Clone, PartialEq)]
pub enum AcpSessionState {
    Active,
    Ended,
}

/// Create a new ACP session with the agent.
/// Returns (session_id, models) where models are extracted from the response.
pub async fn create_session(
    process: &mut AgentProcess,
    cwd: &str,
) -> AppResult<(String, Vec<AgentModel>)> {
    log::info!("create_session: Starting ACP session/new request");
    let req = transport::build_request(
        2,
        "session/new",
        Some(json!({
            "cwd": cwd,
            "mcpServers": []
        })),
    );

    log::info!("create_session: Sending request: {}", serde_json::to_string(&req).unwrap_or_default());
    transport::send_message(process, &req).await?;
    log::info!("create_session: Request sent, waiting for response...");
    let response = transport::receive_response_with_timeout(process, &json!(2), 90).await?;
    log::info!("create_session: Response received");

    // Check for error in response
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::Acp(format!("session/new failed: {}", msg)));
    }

    let result = response.get("result").ok_or_else(|| {
        let resp_str = serde_json::to_string(&response).unwrap_or_default();
        AppError::Acp(format!("No result in session/new response: {}", resp_str))
    })?;

    let session_id = result
        .get("sessionId")
        .and_then(|s| s.as_str())
        .ok_or_else(|| {
            let resp_str = serde_json::to_string(&response).unwrap_or_default();
            AppError::Acp(format!("No sessionId in session/new response: {}", resp_str))
        })?
        .to_string();

    // Log the full result for debugging
    log::info!("session/new result: {}", serde_json::to_string(&result).unwrap_or_else(|_| "<error>".into()));

    // Extract models from the unstable `models` field (preferred for claude-code-acp)
    let models = if let Some(models_state) = result.get("models") {
        log::info!("Found 'models' field in response");
        // Parse from models.availableModels array
        if let Some(available) = models_state.get("availableModels").and_then(|a| a.as_array()) {
            log::info!("Found availableModels array with {} items", available.len());
            available
                .iter()
                .filter_map(|m| {
                    let model_id = m.get("modelId")?.as_str()?.to_string();
                    let name = m.get("name")?.as_str()?.to_string();
                    let description = m.get("description").and_then(|d| d.as_str()).map(String::from);
                    Some(AgentModel { model_id, name, description })
                })
                .collect()
        } else {
            log::warn!("'models' field found but no 'availableModels' array");
            Vec::new()
        }
    } else {
        log::info!("No 'models' field, trying 'configOptions'");
        // Fallback: try to extract models from configOptions with category "model" (stable ACP spec)
        if let Some(opts) = result.get("configOptions").and_then(|o| o.as_array()) {
            log::info!("Found configOptions array with {} items", opts.len());
            let model_opts: Vec<_> = opts.iter()
                .filter(|opt| opt.get("category").and_then(|c| c.as_str()) == Some("model"))
                .collect();
            log::info!("Found {} configOptions with category='model'", model_opts.len());
            if model_opts.is_empty() {
                log::info!("Config options keys: {:?}", opts.iter().filter_map(|o| o.get("id")).collect::<Vec<_>>());
            }
            model_opts.iter()
                .filter_map(|opt| {
                    let _current_value = opt.get("currentValue")?.as_str()?.to_string();
                    let options = opt.get("options")?.as_array()?;
                    Some(options.iter().filter_map(move |o| {
                        let value = o.get("value")?.as_str()?.to_string();
                        let label = o.get("label")?.as_str()?.to_string();
                        Some(AgentModel {
                            model_id: value,
                            name: label,
                            description: None,
                        })
                    }))
                })
                .flatten()
                .collect()
        } else {
            Vec::new()
        }
    };

    log::info!(
        "Session created: {}, extracted {} models from response",
        session_id,
        models.len()
    );

    // Fallback: if no models from session/new, try config/list (ACP spec method)
    if models.is_empty() {
        log::info!("No models from session/new, trying config/list fallback");
        match fetch_config_models(process).await {
            Ok(config_models) if !config_models.is_empty() => {
                log::info!("config/list returned {} models", config_models.len());
                return Ok((session_id, config_models));
            }
            Ok(_) => {
                log::info!("config/list returned no models either");
            }
            Err(e) => {
                log::info!("config/list not supported or failed: {}", e);
            }
        }
    }

    Ok((session_id, models))
}

/// Fetch models via the ACP `config/list` method.
/// Some agents (e.g. Gemini CLI) don't return models in session/new but support config/list.
async fn fetch_config_models(process: &mut AgentProcess) -> AppResult<Vec<AgentModel>> {
    let req = transport::build_request(
        100,
        "config/list",
        None,
    );

    transport::send_message(process, &req).await?;
    let response = transport::receive_response(process, &json!(100)).await?;

    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::Acp(format!("config/list failed: {}", msg)));
    }

    let result = response.get("result").ok_or_else(|| {
        AppError::Acp("No result in config/list response".into())
    })?;

    log::info!("config/list result: {}", serde_json::to_string(&result).unwrap_or_else(|_| "<error>".into()));

    // config/list returns { configOptions: [...] }
    // Look for options with category "model"
    let mut models = Vec::new();

    if let Some(opts) = result.get("configOptions").and_then(|o| o.as_array()) {
        for opt in opts {
            let category = opt.get("category").and_then(|c| c.as_str()).unwrap_or("");
            if category != "model" {
                continue;
            }
            if let Some(options) = opt.get("options").and_then(|o| o.as_array()) {
                for o in options {
                    if let (Some(value), Some(label)) = (
                        o.get("value").and_then(|v| v.as_str()),
                        o.get("label").and_then(|l| l.as_str()),
                    ) {
                        models.push(AgentModel {
                            model_id: value.to_string(),
                            name: label.to_string(),
                            description: o.get("description").and_then(|d| d.as_str()).map(String::from),
                        });
                    }
                }
            }
        }
    }

    // Also check for a flat array of items with id/name pattern
    if models.is_empty() {
        if let Some(items) = result.as_array() {
            for item in items {
                if let (Some(id), Some(name)) = (
                    item.get("id").and_then(|v| v.as_str()),
                    item.get("name").and_then(|n| n.as_str()),
                ) {
                    models.push(AgentModel {
                        model_id: id.to_string(),
                        name: name.to_string(),
                        description: item.get("description").and_then(|d| d.as_str()).map(String::from),
                    });
                }
            }
        }
    }

    Ok(models)
}

/// Load an existing ACP session.
/// Returns the session history and state.
pub async fn load_session(
    process: &mut AgentProcess,
    acp_session_id: &str,
) -> AppResult<Vec<SessionHistoryEntry>> {
    log::info!("load_session: Starting ACP session/load request for {}", acp_session_id);
    let req = transport::build_request(
        3,
        "session/load",
        Some(json!({
            "sessionId": acp_session_id
        })),
    );

    log::info!("load_session: Sending request: {}", serde_json::to_string(&req).unwrap_or_default());
    transport::send_message(process, &req).await?;
    log::info!("load_session: Request sent, waiting for response...");
    let response = transport::receive_response(process, &json!(3)).await?;
    log::info!("load_session: Response received");

    // Check for error in response
    if let Some(error) = response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        // If session doesn't exist, treat as ended
        if error.get("code").and_then(|c| c.as_i64()) == Some(-32001) {
            return Err(AppError::Acp(format!("Session not found or expired: {}", msg)));
        }
        return Err(AppError::Acp(format!("session/load failed: {}", msg)));
    }

    let result = response.get("result").ok_or_else(|| {
        let resp_str = serde_json::to_string(&response).unwrap_or_default();
        AppError::Acp(format!("No result in session/load response: {}", resp_str))
    })?;

    log::info!("session/load result: {}", serde_json::to_string(&result).unwrap_or_else(|_| "<error>".into()));

    // Extract session history if available
    let history = if let Some(history_array) = result.get("history").and_then(|h| h.as_array()) {
        log::info!("Found history array with {} entries", history_array.len());
        history_array
            .iter()
            .filter_map(|entry| {
                let role = entry.get("role")?.as_str()?.to_string();
                let content = entry.get("content").cloned().unwrap_or(json!(null));
                let timestamp = entry.get("timestamp").and_then(|t| t.as_str()).map(String::from);
                Some(SessionHistoryEntry { role, content, timestamp })
            })
            .collect()
    } else {
        log::info!("No history field in session/load response");
        Vec::new()
    };

    log::info!("Session loaded: {} with {} history entries", acp_session_id, history.len());

    Ok(history)
}

/// End an active ACP session.
/// This sends a session/end notification to the agent.
pub async fn end_session(
    process: &mut AgentProcess,
    acp_session_id: &str,
) -> AppResult<()> {
    log::info!("end_session: Sending ACP session/end notification for {}", acp_session_id);

    // session/end is a notification (no response expected)
    let req = transport::build_notification(
        "session/end",
        Some(json!({
            "sessionId": acp_session_id
        })),
    );

    transport::send_message(process, &req).await?;
    log::info!("end_session: Session end notification sent for {}", acp_session_id);

    Ok(())
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
