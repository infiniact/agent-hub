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
    pub child: Child,
    pub stdin: Arc<AsyncMutex<BufWriter<ChildStdin>>>,
    pub reader_handle: tokio::task::JoinHandle<()>,
    pub message_rx: mpsc::Receiver<serde_json::Value>,
    pub status: AgentProcessStatus,
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
) -> AppResult<AgentProcess> {
    log::info!("Spawning agent process: command={}, args={:?}", command, args);

    // Build enriched PATH so child process can find node, claude, etc.
    let enriched_path = discovery::get_enriched_path();
    log::debug!("Enriched PATH for agent process: {}", enriched_path);

    let mut child = tokio::process::Command::new(command)
        .args(args)
        .env("PATH", &enriched_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())  // Inherit stderr for debugging
        .spawn()
        .map_err(|e| {
            log::error!("Failed to spawn agent process: {}", e);
            AppError::Acp(format!("Failed to spawn agent process: {e}"))
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

    let (message_tx, message_rx) = mpsc::channel::<serde_json::Value>(256);
    let reader_handle = spawn_reader_task(stdout, message_tx);

    Ok(AgentProcess {
        agent_id: agent_id.to_string(),
        child,
        stdin: Arc::new(AsyncMutex::new(BufWriter::new(stdin))),
        reader_handle,
        message_rx,
        status: AgentProcessStatus::Starting,
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
