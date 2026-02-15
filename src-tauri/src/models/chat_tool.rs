use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatTool {
    pub id: String,
    pub name: String,
    pub plugin_type: String,
    pub config_json: String,
    pub linked_agent_id: Option<String>,
    pub status: String,
    pub status_message: Option<String>,
    pub auto_reply_mode: String,
    pub workspace_id: Option<String>,
    pub messages_received: i64,
    pub messages_sent: i64,
    pub last_active_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChatToolRequest {
    pub name: String,
    #[serde(default = "default_plugin_type")]
    pub plugin_type: String,
    #[serde(default = "default_config")]
    pub config_json: String,
    pub linked_agent_id: Option<String>,
    #[serde(default = "default_auto_reply_mode")]
    pub auto_reply_mode: String,
    pub workspace_id: Option<String>,
}

fn default_plugin_type() -> String {
    "wechat".into()
}

fn default_config() -> String {
    "{}".into()
}

fn default_auto_reply_mode() -> String {
    "all".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateChatToolRequest {
    pub name: Option<String>,
    pub config_json: Option<String>,
    pub linked_agent_id: Option<String>,
    pub auto_reply_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolMessage {
    pub id: String,
    pub chat_tool_id: String,
    pub direction: String,
    pub external_sender_id: Option<String>,
    pub external_sender_name: Option<String>,
    pub content: String,
    pub content_type: String,
    pub agent_response: Option<String>,
    pub is_processed: bool,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolContact {
    pub id: String,
    pub chat_tool_id: String,
    pub external_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub contact_type: String,
    pub is_blocked: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Events emitted by the Bridge subprocess via stdout NDJSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeEvent {
    Status {
        status: String,
    },
    Qrcode {
        url: String,
        #[serde(default)]
        image_base64: String,
    },
    Login {
        user_id: String,
        user_name: String,
    },
    Logout,
    Message {
        message_id: String,
        sender_id: String,
        sender_name: String,
        content: String,
        #[serde(default = "default_content_type")]
        content_type: String,
    },
    Contacts {
        contacts: Vec<BridgeContact>,
    },
    Error {
        error: String,
    },
    Heartbeat,
    Pong {
        ts: i64,
    },
}

fn default_content_type() -> String {
    "text".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeContact {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default = "default_contact_type")]
    pub contact_type: String,
}

fn default_contact_type() -> String {
    "personal".into()
}

/// Commands sent from Rust to the Bridge subprocess via stdin NDJSON
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeCommand {
    SendMessage {
        to_id: String,
        content: String,
        #[serde(default = "default_content_type")]
        content_type: String,
    },
    GetContacts,
    Logout,
    Stop,
    Ping {
        ts: i64,
    },
}
