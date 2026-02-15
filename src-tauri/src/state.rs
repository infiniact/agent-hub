use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::io::BufWriter;
use tokio::process::ChildStdin;
use rusqlite::Connection;
use tokio_util::sync::CancellationToken;

use serde::{Deserialize, Serialize};

use crate::acp::manager::AgentProcess;
use crate::chat_tool::manager::ChatToolProcess;
use crate::scheduler::SchedulerState;

/// Action the user can take when orchestration awaits confirmation
#[derive(Debug)]
pub enum ConfirmationAction {
    /// Proceed with summarization
    Confirm,
    /// Re-run a specific agent by ID
    RegenerateAgent(String),
    /// Re-run all agents
    RegenerateAll,
}

/// Key for a pending orchestration permission request: (task_run_id, request_id)
pub type OrchPermissionKey = (String, String);

/// ACP session state following the ACP protocol specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AcpSessionState {
    /// Session has been created but not yet loaded/used
    #[serde(rename = "new")]
    New,
    /// Session is active and ready for prompts
    #[serde(rename = "active")]
    Active,
    /// Session has been ended
    #[serde(rename = "ended")]
    Ended,
}

impl Default for AcpSessionState {
    fn default() -> Self {
        Self::New
    }
}

impl std::fmt::Display for AcpSessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcpSessionState::New => write!(f, "new"),
            AcpSessionState::Active => write!(f, "active"),
            AcpSessionState::Ended => write!(f, "ended"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcpSessionInfo {
    /// Internal session ID (from our DB)
    pub session_id: String,
    /// Agent ID that owns this session
    pub agent_id: String,
    /// ACP protocol session ID (from agent)
    pub acp_session_id: String,
    /// Current state of the session following ACP spec
    #[serde(default)]
    pub state: AcpSessionState,
    /// Timestamp when session was created
    pub created_at: String,
    /// Timestamp when session was last used
    pub last_used_at: String,
}

impl AcpSessionInfo {
    /// Create a new session info with default state
    pub fn new(session_id: String, agent_id: String, acp_session_id: String) -> Self {
        let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        Self {
            session_id,
            agent_id,
            acp_session_id,
            state: AcpSessionState::New,
            created_at: now.clone(),
            last_used_at: now,
        }
    }

    /// Mark session as active (after first prompt or successful load)
    pub fn mark_active(&mut self) {
        self.state = AcpSessionState::Active;
        self.last_used_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    }

    /// Mark session as ended
    pub fn mark_ended(&mut self) {
        self.state = AcpSessionState::Ended;
        self.last_used_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    }

    /// Update last used timestamp
    pub fn touch(&mut self) {
        self.last_used_at = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    }

    /// Check if session is usable for prompts
    pub fn is_usable(&self) -> bool {
        self.state == AcpSessionState::New || self.state == AcpSessionState::Active
    }
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
    /// Per-agent cancellation tokens: (task_run_id, agent_id) -> CancellationToken
    pub agent_cancellations: Arc<Mutex<HashMap<(String, String), CancellationToken>>>,
    /// Pending confirmation channels: task_run_id -> oneshot sender
    pub pending_confirmations: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ConfirmationAction>>>>,
    /// Pending orchestration permission channels: (task_run_id, request_id) -> oneshot sender(option_id)
    pub pending_orch_permissions: Arc<Mutex<HashMap<OrchPermissionKey, tokio::sync::oneshot::Sender<String>>>>,
    /// Scheduler state for background task execution
    pub scheduler: Arc<Mutex<Option<SchedulerState>>>,
    /// Cached skill discovery result from workspace scanning
    pub discovered_skills: Arc<Mutex<Option<crate::acp::skill_discovery::SkillDiscoveryResult>>>,
    /// Running chat tool bridge processes keyed by chat_tool_id
    pub chat_tool_processes: Arc<Mutex<HashMap<String, ChatToolProcess>>>,
    /// Cancellation tokens for chat tool bridge event loops
    pub chat_tool_cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Cached QR code images for chat tool login (chat_tool_id -> base64 image)
    pub chat_tool_qr_codes: Arc<Mutex<HashMap<String, String>>>,
    /// Persistent ACP session IDs for chat tools (chat_tool_id -> acp_session_id)
    pub chat_tool_acp_sessions: Arc<Mutex<HashMap<String, String>>>,
    /// Task run IDs for chat tool message processing (chat_tool_id -> task_run_id)
    pub chat_tool_task_runs: Arc<Mutex<HashMap<String, String>>>,
    /// Set of chat_tool_ids currently processing a message (used for busy-reply)
    pub chat_tool_processing: Arc<Mutex<HashSet<String>>>,
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
            agent_cancellations: Arc::new(Mutex::new(HashMap::new())),
            pending_confirmations: Arc::new(Mutex::new(HashMap::new())),
            pending_orch_permissions: Arc::new(Mutex::new(HashMap::new())),
            scheduler: Arc::new(Mutex::new(None)),
            discovered_skills: Arc::new(Mutex::new(None)),
            chat_tool_processes: Arc::new(Mutex::new(HashMap::new())),
            chat_tool_cancellations: Arc::new(Mutex::new(HashMap::new())),
            chat_tool_qr_codes: Arc::new(Mutex::new(HashMap::new())),
            chat_tool_acp_sessions: Arc::new(Mutex::new(HashMap::new())),
            chat_tool_task_runs: Arc::new(Mutex::new(HashMap::new())),
            chat_tool_processing: Arc::new(Mutex::new(HashSet::new())),
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
            agent_cancellations: Arc::clone(&self.agent_cancellations),
            pending_confirmations: Arc::clone(&self.pending_confirmations),
            pending_orch_permissions: Arc::clone(&self.pending_orch_permissions),
            scheduler: Arc::clone(&self.scheduler),
            discovered_skills: Arc::clone(&self.discovered_skills),
            chat_tool_processes: Arc::clone(&self.chat_tool_processes),
            chat_tool_cancellations: Arc::clone(&self.chat_tool_cancellations),
            chat_tool_qr_codes: Arc::clone(&self.chat_tool_qr_codes),
            chat_tool_acp_sessions: Arc::clone(&self.chat_tool_acp_sessions),
            chat_tool_task_runs: Arc::clone(&self.chat_tool_task_runs),
            chat_tool_processing: Arc::clone(&self.chat_tool_processing),
        }
    }
}
