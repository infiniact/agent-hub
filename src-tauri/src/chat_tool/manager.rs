use std::process::Stdio;
use std::sync::Arc;

use tokio::io::BufWriter;
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{AppError, AppResult};
use crate::models::chat_tool::BridgeCommand;

#[derive(Debug)]
pub struct ChatToolProcess {
    pub chat_tool_id: String,
    pub plugin_type: String,
    pub child: Child,
    pub stdin: Arc<AsyncMutex<BufWriter<ChildStdin>>>,
}

/// Spawn a bridge subprocess and return (process, stdout) for the event loop.
pub async fn spawn_bridge(
    chat_tool_id: &str,
    plugin_type: &str,
    config_json: &str,
) -> AppResult<(ChatToolProcess, ChildStdout)> {
    let bridge_path = get_bridge_path(plugin_type)?;

    log::info!(
        "Spawning bridge process: plugin_type={}, path={}, chat_tool_id={}",
        plugin_type, bridge_path, chat_tool_id
    );

    let enriched_path = crate::acp::discovery::get_enriched_path();

    let mut cmd = tokio::process::Command::new("node");
    cmd.arg(&bridge_path)
        .env("CHAT_TOOL_CONFIG", config_json)
        .env("CHAT_TOOL_ID", chat_tool_id)
        .env("PATH", &enriched_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        log::error!("Failed to spawn bridge process: {}", e);
        AppError::Internal(format!(
            "Failed to spawn bridge process for '{}': {e}",
            plugin_type
        ))
    })?;

    log::info!("Bridge process spawned with PID: {:?}", child.id());

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("Failed to capture bridge stdin".into()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("Failed to capture bridge stdout".into()))?;

    // Capture stderr into a shared buffer for diagnostics
    let stderr_lines = std::sync::Arc::new(AsyncMutex::new(Vec::<String>::new()));
    if let Some(stderr) = child.stderr.take() {
        let id_str = chat_tool_id.to_string();
        let stderr_buf = stderr_lines.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log::warn!("[Bridge:{}:stderr] {}", id_str, line);
                let mut buf = stderr_buf.lock().await;
                if buf.len() < 50 {
                    buf.push(line);
                }
            }
        });
    }

    // Brief delay then check for immediate crash
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    match child.try_wait() {
        Ok(Some(exit_status)) => {
            let stderr_output = {
                let lines = stderr_lines.lock().await;
                if lines.is_empty() {
                    "(no stderr output)".to_string()
                } else {
                    lines.join("\n")
                }
            };
            let msg = format!(
                "Bridge process exited immediately with {}\nStderr:\n{}",
                exit_status, stderr_output
            );
            log::error!("{}", msg);
            return Err(AppError::Internal(msg));
        }
        Ok(None) => {
            log::info!("Bridge process is running (PID: {:?})", child.id());
        }
        Err(e) => {
            log::warn!("Could not check bridge process status: {}", e);
        }
    }

    let process = ChatToolProcess {
        chat_tool_id: chat_tool_id.to_string(),
        plugin_type: plugin_type.to_string(),
        child,
        stdin: Arc::new(AsyncMutex::new(BufWriter::new(stdin))),
    };

    Ok((process, stdout))
}

pub async fn send_bridge_command(
    process: &ChatToolProcess,
    command: &BridgeCommand,
) -> AppResult<()> {
    use tokio::io::AsyncWriteExt;

    let json = serde_json::to_string(command)
        .map_err(|e| AppError::Internal(format!("Failed to serialize bridge command: {e}")))?;

    let mut stdin = process.stdin.lock().await;
    stdin
        .write_all(json.as_bytes())
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write to bridge stdin: {e}")))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to write newline to bridge stdin: {e}")))?;
    stdin
        .flush()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to flush bridge stdin: {e}")))?;

    Ok(())
}

pub async fn stop_bridge_process(process: &mut ChatToolProcess) -> AppResult<()> {
    // Try graceful shutdown first
    let stop_cmd = BridgeCommand::Stop;
    if let Err(e) = send_bridge_command(process, &stop_cmd).await {
        log::warn!("Failed to send stop command to bridge: {}", e);
    }

    // Wait briefly for graceful exit
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Force kill if still running
    match process.child.try_wait() {
        Ok(Some(_)) => {
            log::info!("Bridge process exited gracefully");
        }
        _ => {
            log::info!("Force killing bridge process");
            let _ = process.child.kill().await;
        }
    }

    Ok(())
}

/// Check whether the bridge child process is still alive.
/// Returns `true` if the process has not yet exited.
pub fn check_process_alive(process: &mut ChatToolProcess) -> bool {
    match process.child.try_wait() {
        Ok(Some(_)) => false, // process has exited
        Ok(None) => true,     // still running
        Err(e) => {
            log::warn!("Failed to check bridge process status: {}", e);
            false // treat as dead on error
        }
    }
}

fn get_bridge_path(plugin_type: &str) -> AppResult<String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| AppError::Internal(format!("Failed to get exe path: {e}")))?;

    let bridge_file = std::path::PathBuf::from("bridges")
        .join(plugin_type)
        .join("index.js");

    // Walk up from the executable directory to find the project root containing bridges/
    let mut dir = exe_path.parent();
    while let Some(d) = dir {
        let candidate = d.join(&bridge_file);
        if candidate.exists() {
            return Ok(candidate.to_string_lossy().to_string());
        }
        dir = d.parent();
    }

    // Also try relative to the current working directory
    let cwd_candidate = bridge_file.clone();
    if cwd_candidate.exists() {
        return Ok(cwd_candidate.to_string_lossy().to_string());
    }

    Err(AppError::NotFound(format!(
        "Bridge script not found for plugin type '{plugin_type}'"
    )))
}
