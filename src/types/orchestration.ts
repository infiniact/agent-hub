export interface TaskRun {
  id: string;
  title: string;
  user_prompt: string;
  control_hub_agent_id: string;
  status: 'pending' | 'analyzing' | 'running' | 'completed' | 'failed' | 'cancelled';
  task_plan_json: string | null;
  result_summary: string | null;
  total_tokens_in: number;
  total_tokens_out: number;
  total_duration_ms: number;
  created_at: string;
  updated_at: string;
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
}

export interface AgentTrackingInfo {
  agentId: string;
  agentName: string;
  model: string;
  status: 'pending' | 'running' | 'completed' | 'failed';
  tokensIn: number;
  tokensOut: number;
  durationMs: number;
  streamedContent: string;
}
