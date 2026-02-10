import type { AgentModel } from './agent';

export interface AcpAgentStatus {
  agent_id: string;
  status: string;
}

export type AcpSessionState = 'new' | 'active' | 'ended';

export interface AcpSessionInfo {
  session_id: string;
  agent_id: string;
  acp_session_id: string;
  state: AcpSessionState;
  created_at: string;
  last_used_at: string;
}

export interface ResumeSessionResult {
  acp_session_id: string;
  is_loaded: boolean;
  models: AgentModel[];
}

export interface AcpPermissionRequest {
  agent_id: string;
  session_id: string;
  permission_id: string;
  description: string;
  tool_name: string;
  arguments: Record<string, unknown>;
}

export interface AcpMessageChunk {
  method: string;
  params?: {
    sessionId?: string;
    content?: Array<{ type: string; text?: string }>;
    [key: string]: unknown;
  };
}
