use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub skill_type: String,
    pub description: String,
    #[serde(default)]
    pub task_keywords: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Source of this skill: "", "manual", "discovered:mcp", "discovered:project"
    #[serde(default)]
    pub skill_source: String,
    /// SPDX license identifier (Agent Skills spec)
    #[serde(default)]
    pub license: Option<String>,
    /// Compatibility string, e.g. "claude" (Agent Skills spec)
    #[serde(default)]
    pub compatibility: Option<String>,
    /// Arbitrary key-value metadata from SKILL.md frontmatter (Agent Skills spec)
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

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
    #[serde(default = "default_skills")]
    pub skills_json: String,
    pub acp_command: Option<String>,
    pub acp_args_json: Option<String>,
    pub is_control_hub: bool,
    pub md_file_path: Option<String>,
    pub max_concurrency: i64,
    pub available_models_json: Option<String>,
    pub is_enabled: bool,
    pub disabled_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
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
    #[serde(default = "default_skills")]
    pub skills_json: String,
    pub acp_command: Option<String>,
    pub acp_args_json: Option<String>,
    #[serde(default)]
    pub is_control_hub: bool,
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: i64,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub skills_json: Option<String>,
    pub acp_command: Option<String>,
    pub acp_args_json: Option<String>,
    pub is_control_hub: Option<bool>,
    pub max_concurrency: Option<i64>,
    pub available_models_json: Option<String>,
    pub is_enabled: Option<bool>,
    pub disabled_reason: Option<String>,
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
    /// Known model list for this CLI identity.
    #[serde(default)]
    pub models: Vec<String>,
    /// Registry entry ID (e.g. "claude-code-acp", "gemini").
    #[serde(default)]
    pub registry_id: Option<String>,
    /// Agent icon URL from registry.
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Agent description from registry.
    #[serde(default)]
    pub description: String,
    /// ACP adapter version (e.g. "0.16.1").
    #[serde(default)]
    pub adapter_version: Option<String>,
    /// CLI version (e.g. "2.1.39").
    #[serde(default)]
    pub cli_version: Option<String>,
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
fn default_skills() -> String {
    "[]".into()
}
fn default_max_concurrency() -> i64 {
    1
}
