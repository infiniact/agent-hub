use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;

use crate::acp::discovery;
use crate::error::{AppError, AppResult};

#[derive(Debug)]
pub struct AgentProcess {
    pub agent_id: String,
    /// The command used to spawn this process (for detecting agent type changes).
    pub spawn_command: String,
    /// The registry command name (e.g. "gemini", "codex-acp") for identity tracking.
    /// Fixes restart detection when npx agents all share the same basename "npx".
    pub agent_type: String,
    /// The CLI version (from cli.js header) at the time this process was spawned.
    /// Used to detect on-disk SDK upgrades that require a process restart.
    pub cli_version: String,
    pub child: Child,
    pub stdin: Arc<AsyncMutex<BufWriter<ChildStdin>>>,
    pub reader_handle: tokio::task::JoinHandle<()>,
    pub message_rx: mpsc::Receiver<serde_json::Value>,
    pub status: AgentProcessStatus,
    /// Captured stderr lines for debugging
    pub stderr_lines: Arc<AsyncMutex<Vec<String>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentProcessStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

impl std::fmt::Display for AgentProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentProcessStatus::Starting => write!(f, "Starting"),
            AgentProcessStatus::Running => write!(f, "Running"),
            AgentProcessStatus::Stopped => write!(f, "Stopped"),
            AgentProcessStatus::Error(e) => write!(f, "Error: {e}"),
        }
    }
}

pub async fn spawn_agent_process(
    agent_id: &str,
    command: &str,
    args: &[String],
    extra_env: &HashMap<String, String>,
    agent_type: &str,
) -> AppResult<AgentProcess> {
    log::info!("Spawning agent process: command={}, args={:?}, extra_env={:?}, agent_type={}", command, args, extra_env, agent_type);

    // Build enriched PATH so child process can find node, claude, etc.
    let enriched_path = discovery::get_enriched_path();
    log::debug!("Enriched PATH for agent process: {}", enriched_path);

    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args)
        .env("PATH", &enriched_path)
        .envs(extra_env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()
        .map_err(|e| {
            log::error!("Failed to spawn agent process: {}", e);
            AppError::Acp(format!("Failed to spawn agent process '{}': {e}", command))
        })?;

    log::info!("Agent process spawned with PID: {:?}", child.id());

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Acp("Failed to capture agent stdin".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Acp("Failed to capture agent stdout".into()))?;
    let stderr = child
        .stderr
        .take();

    let (message_tx, message_rx) = mpsc::channel::<serde_json::Value>(256);
    let reader_handle = spawn_reader_task(stdout, message_tx);

    // Capture stderr for debugging
    let stderr_lines = Arc::new(AsyncMutex::new(Vec::<String>::new()));
    if let Some(stderr) = stderr {
        let stderr_lines_clone = stderr_lines.clone();
        let agent_id_str = agent_id.to_string();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log::warn!("[Agent:{}:stderr] {}", agent_id_str, line);
                let mut buf = stderr_lines_clone.lock().await;
                // Keep last 50 lines
                if buf.len() >= 50 {
                    buf.remove(0);
                }
                buf.push(line);
            }
        });
    }

    // Brief delay to let the process start, then check if it's still alive.
    // For npx-based agents, skip the early-exit check since npx takes time to download
    // the package on first run, but still sleep to allow pipes to connect.
    let is_npx = crate::acp::provisioner::is_npx_command(command);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Check if process exited immediately (skip for npx — it may still be downloading)
    if !is_npx {
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
                    "Agent process exited immediately with {}\nCommand: {} {}\nStderr:\n{}",
                    exit_status,
                    command,
                    args.join(" "),
                    stderr_output
                );
                log::error!("{}", msg);
                return Err(AppError::Acp(msg));
            }
            Ok(None) => {
                log::info!("Agent process is running (PID: {:?})", child.id());
            }
            Err(e) => {
                log::warn!("Could not check agent process status: {}", e);
            }
        }
    }

    Ok(AgentProcess {
        agent_id: agent_id.to_string(),
        spawn_command: command.to_string(),
        agent_type: agent_type.to_string(),
        cli_version: String::new(),
        child,
        stdin: Arc::new(AsyncMutex::new(BufWriter::new(stdin))),
        reader_handle,
        message_rx,
        status: AgentProcessStatus::Starting,
        stderr_lines,
    })
}

fn spawn_reader_task(
    stdout: ChildStdout,
    tx: mpsc::Sender<serde_json::Value>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut line_count = 0;
        while let Ok(Some(line)) = lines.next_line().await {
            line_count += 1;
            let trimmed = line.trim();
            let preview = if trimmed.len() > 200 {
                let mut end = 200;
                while !trimmed.is_char_boundary(end) { end -= 1; }
                &trimmed[..end]
            } else {
                trimmed
            };
            log::debug!("Agent stdout line {}: {}", line_count, preview);
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(msg) => {
                    log::debug!("Parsed JSON-RPC message: {:?}", serde_json::to_string(&msg).unwrap_or_default());
                    if tx.send(msg).await.is_err() {
                        log::warn!("Failed to send message to channel, closing reader");
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse agent NDJSON line {}: '{}', error: {}", line_count, trimmed, e);
                }
            }
        }
        log::info!("Agent stdout reader ended after {} lines", line_count);
    })
}

pub async fn stop_agent_process(process: &mut AgentProcess) -> AppResult<()> {
    process
        .child
        .kill()
        .await
        .map_err(|e| AppError::Acp(format!("Failed to kill agent process: {e}")))?;
    process.status = AgentProcessStatus::Stopped;
    Ok(())
}
