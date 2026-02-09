use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub title: String,
    pub mode: String,
    pub acp_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub agent_id: String,
    #[serde(default = "default_title")]
    pub title: String,
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_title() -> String {
    "New Session".into()
}
fn default_mode() -> String {
    "code".into()
}
