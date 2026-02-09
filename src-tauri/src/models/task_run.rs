use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    pub id: String,
    pub title: String,
    pub user_prompt: String,
    pub control_hub_agent_id: String,
    pub status: String,
    pub task_plan_json: Option<String>,
    pub result_summary: Option<String>,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_duration_ms: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAssignment {
    pub id: String,
    pub task_run_id: String,
    pub agent_id: String,
    pub agent_name: String,
    pub sequence_order: i64,
    pub input_text: String,
    pub output_text: Option<String>,
    pub status: String,
    pub model_used: Option<String>,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub duration_ms: i64,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub analysis: String,
    pub assignments: Vec<PlannedAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedAssignment {
    pub agent_id: String,
    pub task_description: String,
    pub sequence_order: i64,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRunRequest {
    pub user_prompt: String,
    #[serde(default)]
    pub title: String,
}
