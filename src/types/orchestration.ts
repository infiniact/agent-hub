export interface RecurrencePattern {
  frequency: 'daily' | 'weekly' | 'monthly' | 'yearly';
  time: string;  // HH:MM format
  interval?: number;  // Interval between occurrences, default 1
  days_of_week?: number[];  // Days of week for weekly (0 = Sunday, 6 = Saturday)
  day_of_month?: number;  // Day of month for monthly/yearly (1-31)
  month?: number;  // Month for yearly (1-12)
}

export interface TaskRun {
  id: string;
  title: string;
  user_prompt: string;
  control_hub_agent_id: string;
  status: 'pending' | 'analyzing' | 'running' | 'awaiting_confirmation' | 'completed' | 'failed' | 'cancelled';
  task_plan_json: string | null;
  result_summary: string | null;
  total_tokens_in: number;
  total_tokens_out: number;
  total_cache_creation_tokens: number;
  total_cache_read_tokens: number;
  total_duration_ms: number;
  created_at: string;
  updated_at: string;
  rating: number | null; // 1-5 stars rating for completed tasks
  // Scheduling fields
  schedule_type: 'none' | 'once' | 'recurring';
  scheduled_time: string | null;
  recurrence_pattern_json: string | null;
  next_run_at: string | null;
  is_paused: boolean;
}

export interface TaskAssignment {
  id: string;
  task_run_id: string;
  agent_id: string;
  agent_name: string;
  sequence_order: number;
  input_text: string;
  output_text: string | null;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'skipped';
  model_used: string | null;
  tokens_in: number;
  tokens_out: number;
  cache_creation_tokens: number;
  cache_read_tokens: number;
  started_at: string | null;
  completed_at: string | null;
  duration_ms: number;
  error_message: string | null;
  created_at: string;
}

export interface TaskPlan {
  analysis: string;
  assignments: PlannedAssignment[];
}

export interface PlannedAssignment {
  agent_id: string;
  task_description: string;
  sequence_order: number;
  depends_on: string[];
  matched_skills?: string[];
  selection_reason?: string;
}

export interface AssignmentValidation {
  agent_id: string;
  agent_name: string;
  warnings: string[];
}

export interface PlanValidation {
  is_valid: boolean;
  assignment_validations: AssignmentValidation[];
  total_warnings: number;
}

export interface AgentTrackingInfo {
  agentId: string;
  agentName: string;
  model: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  tokensIn: number;
  tokensOut: number;
  cacheCreationTokens: number;
  cacheReadTokens: number;
  durationMs: number;
  streamedContent: string;
  acpSessionId?: string;
  output?: string;
  toolCalls?: OrchToolCall[];
  assignmentId?: string;
}

export interface OrchToolCall {
  toolCallId: string;
  name: string;
  title?: string;
  status: string;
  rawInput?: any;
  rawOutput?: any;
}

export interface OrchPermissionRequest {
  taskRunId: string;
  agentId: string;
  requestId: number | string;
  sessionId: string;
  toolCall?: { toolCallId: string; title: string; rawInput?: any };
  options: Array<{ optionId: string; name: string; kind: string }>;
}

// Scheduling request types
export interface ScheduleTaskRequest {
  task_run_id: string;
  schedule_type: 'none' | 'once' | 'recurring';
  scheduled_time?: string;  // ISO 8601 datetime for one-time execution
  recurrence_pattern?: RecurrencePattern;
}
