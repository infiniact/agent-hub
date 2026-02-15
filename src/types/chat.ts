export interface ChatMessage {
  id: string;
  session_id: string;
  role: 'User' | 'Agent' | 'System';
  content_json: string;
  tool_calls_json: string | null;
  created_at: string;
}

export interface Session {
  id: string;
  agent_id: string;
  title: string;
  mode: string;
  acp_session_id: string | null;
  created_at: string;
  updated_at: string;
  workspace_id: string | null;
}

export interface CreateSessionRequest {
  agent_id: string;
  title?: string;
  mode?: string;
  workspace_id?: string;
}

export interface ContentBlock {
  type: 'text' | 'image';
  text?: string;
  url?: string;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  status: 'pending' | 'running' | 'complete' | 'error';
  result?: string;
}
