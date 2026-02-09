use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub status: String,
    pub execution_mode: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: i64,
    pub system_prompt: String,
    pub capabilities_json: String,
    pub acp_command: Option<String>,
    pub acp_args_json: Option<String>,
    pub is_control_hub: bool,
    pub md_file_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_execution_mode")]
    pub execution_mode: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: i64,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default = "default_capabilities")]
    pub capabilities_json: String,
    pub acp_command: Option<String>,
    pub acp_args_json: Option<String>,
    #[serde(default)]
    pub is_control_hub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub execution_mode: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i64>,
    pub system_prompt: Option<String>,
    pub capabilities_json: Option<String>,
    pub acp_command: Option<String>,
    pub acp_args_json: Option<String>,
    pub is_control_hub: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgent {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args_json: String,
    pub env_json: String,
    pub source_path: String,
    pub last_seen_at: String,
    /// Whether this agent is actually installed / available on the system.
    #[serde(default)]
    pub available: bool,
}

fn default_icon() -> String {
    "code".into()
}
fn default_execution_mode() -> String {
    "RunNow".into()
}
fn default_model() -> String {
    "gpt-4-turbo".into()
}
fn default_temperature() -> f64 {
    0.7
}
fn default_max_tokens() -> i64 {
    4096
}
fn default_capabilities() -> String {
    "[]".into()
}
