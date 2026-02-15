export interface AgentSkill {
  id: string;
  name: string;
  skill_type: 'mcp' | 'skill' | 'tool';
  description: string;
  task_keywords: string[];
  constraints: string[];
  skill_source?: string;
  license?: string;
  compatibility?: string;
  metadata?: Record<string, string>;
}

export interface SkillDirEntry {
  skill: AgentSkill;
  dir_path: string;
  md_file: string;
  location: string; // "workspace" | "global"
  body_content?: string;
  has_scripts: boolean;
  has_references: boolean;
  has_assets: boolean;
}

export interface SkillDiscoveryResult {
  skills: SkillDirEntry[];
  scanned_directories: string[];
  last_scanned_at: string;
}

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
  skills_json: string;
  acp_command: string | null;
  acp_args_json: string | null;
  is_control_hub: boolean;
  md_file_path: string | null;
  max_concurrency: number;
  available_models_json: string | null;
  is_enabled: boolean;
  disabled_reason: string | null;
  workspace_id: string | null;
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
  skills_json?: string;
  acp_command?: string;
  acp_args_json?: string;
  is_control_hub?: boolean;
  max_concurrency?: number;
  workspace_id?: string;
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
  skills_json?: string;
  acp_command?: string;
  acp_args_json?: string;
  is_control_hub?: boolean;
  max_concurrency?: number;
  available_models_json?: string;
  is_enabled?: boolean;
  disabled_reason?: string | null;
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
  models: string[];
  registry_id: string | null;
  icon_url: string | null;
  description: string;
  adapter_version: string | null;
  cli_version: string | null;
}

export interface AgentModel {
  model_id: string;
  name: string;
  description: string | null;
}
