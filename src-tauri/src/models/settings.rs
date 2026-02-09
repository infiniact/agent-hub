use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}
