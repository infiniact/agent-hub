use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("ACP error: {0}")]
    Acp(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Agent not running: {0}")]
    AgentNotRunning(String),

    #[error("Agent already running: {0}")]
    AgentAlreadyRunning(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Version upgrade required: {0}")]
    VersionUpgradeRequired(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
