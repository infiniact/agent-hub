use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::BufWriter;
use tokio::process::ChildStdin;
use rusqlite::Connection;
use tokio_util::sync::CancellationToken;

use serde::{Deserialize, Serialize};

use crate::acp::manager::AgentProcess;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpSessionInfo {
    pub session_id: String,
    pub agent_id: String,
    pub acp_session_id: String,
}

pub struct AppState {
    /// SQLite database connection
    pub db: Arc<std::sync::Mutex<Connection>>,
    /// Running agent processes keyed by agent ID
    pub agent_processes: Arc<Mutex<HashMap<String, AgentProcess>>>,
    /// Agent stdin handles for sending responses (keyed by agent ID)
    pub agent_stdins: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<BufWriter<ChildStdin>>>>>>,
    /// Active ACP sessions keyed by session ID
    pub acp_sessions: Arc<Mutex<HashMap<String, AcpSessionInfo>>>,
    /// Discovered agents from scanning
    pub discovered_agents: Arc<Mutex<Vec<crate::models::agent::DiscoveredAgent>>>,
    /// Active orchestration task runs with cancellation tokens
    pub active_task_runs: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl AppState {
    pub fn new(conn: Connection) -> Self {
        Self {
            db: Arc::new(std::sync::Mutex::new(conn)),
            agent_processes: Arc::new(Mutex::new(HashMap::new())),
            agent_stdins: Arc::new(Mutex::new(HashMap::new())),
            acp_sessions: Arc::new(Mutex::new(HashMap::new())),
            discovered_agents: Arc::new(Mutex::new(Vec::new())),
            active_task_runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// Implement Clone manually to allow state sharing in spawned tasks
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            db: Arc::clone(&self.db),
            agent_processes: Arc::clone(&self.agent_processes),
            agent_stdins: Arc::clone(&self.agent_stdins),
            acp_sessions: Arc::clone(&self.acp_sessions),
            discovered_agents: Arc::clone(&self.discovered_agents),
            active_task_runs: Arc::clone(&self.active_task_runs),
        }
    }
}
