use serde::{Deserialize, Serialize};

/// Recurrence pattern for scheduled tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurrencePattern {
    /// Frequency of recurrence: daily, weekly, monthly, yearly
    pub frequency: String,
    /// Time of day in HH:MM format (24-hour)
    pub time: String,
    /// Interval between occurrences (e.g., 2 for every 2 days)
    #[serde(default = "default_interval")]
    pub interval: i32,
    /// Days of week for weekly recurrence (0 = Sunday, 6 = Saturday)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days_of_week: Option<Vec<i32>>,
    /// Day of month for monthly recurrence (1-31)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub day_of_month: Option<i32>,
    /// Month for yearly recurrence (1-12)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub month: Option<i32>,
}

fn default_interval() -> i32 {
    1
}

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
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_duration_ms: i64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<i32>,
    // Scheduling fields
    #[serde(default = "default_schedule_type")]
    pub schedule_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence_pattern_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<String>,
    #[serde(default)]
    pub is_paused: bool,
}

fn default_schedule_type() -> String {
    "none".to_string()
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
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
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

/// Request to schedule a task for future execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleTaskRequest {
    pub task_run_id: String,
    /// 'none', 'once', or 'recurring'
    pub schedule_type: String,
    /// ISO 8601 datetime for one-time execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_time: Option<String>,
    /// Recurrence pattern as JSON for recurring execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence_pattern: Option<RecurrencePattern>,
}
