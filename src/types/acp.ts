export interface AcpAgentStatus {
  agent_id: string;
  status: string;
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
