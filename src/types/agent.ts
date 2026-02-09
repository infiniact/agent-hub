export interface AgentConfig {
  id: string;
  name: string;
  icon: string;
  description: string;
  status: string;
  execution_mode: string;
  model: string;
  temperature: number;
  max_tokens: number;
  system_prompt: string;
  capabilities_json: string;
  acp_command: string | null;
  acp_args_json: string | null;
  is_control_hub: boolean;
  md_file_path: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateAgentRequest {
  name: string;
  icon?: string;
  description?: string;
  execution_mode?: string;
  model?: string;
  temperature?: number;
  max_tokens?: number;
  system_prompt?: string;
  capabilities_json?: string;
  acp_command?: string;
  acp_args_json?: string;
  is_control_hub?: boolean;
}

export interface UpdateAgentRequest {
  name?: string;
  icon?: string;
  description?: string;
  status?: string;
  execution_mode?: string;
  model?: string;
  temperature?: number;
  max_tokens?: number;
  system_prompt?: string;
  capabilities_json?: string;
  acp_command?: string;
  acp_args_json?: string;
  is_control_hub?: boolean;
}

export interface DiscoveredAgent {
  id: string;
  name: string;
  command: string;
  args_json: string;
  env_json: string;
  source_path: string;
  last_seen_at: string;
  available: boolean;
}
