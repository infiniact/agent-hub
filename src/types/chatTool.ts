export interface ChatTool {
  id: string;
  name: string;
  plugin_type: string;
  config_json: string;
  linked_agent_id: string | null;
  status: string;
  status_message: string | null;
  auto_reply_mode: string;
  workspace_id: string | null;
  messages_received: number;
  messages_sent: number;
  last_active_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateChatToolRequest {
  name: string;
  plugin_type?: string;
  config_json?: string;
  linked_agent_id?: string;
  auto_reply_mode?: string;
  workspace_id?: string;
}

export interface UpdateChatToolRequest {
  name?: string;
  config_json?: string;
  linked_agent_id?: string;
  auto_reply_mode?: string;
}

export interface ChatToolMessage {
  id: string;
  chat_tool_id: string;
  direction: 'incoming' | 'outgoing';
  external_sender_id: string | null;
  external_sender_name: string | null;
  content: string;
  content_type: string;
  agent_response: string | null;
  is_processed: boolean;
  error_message: string | null;
  created_at: string;
}

export interface ChatToolContact {
  id: string;
  chat_tool_id: string;
  external_id: string;
  name: string;
  avatar_url: string | null;
  contact_type: string;
  is_blocked: boolean;
  created_at: string;
  updated_at: string;
}

export interface ChatToolConfigField {
  key: string;
  label: string;
  type: 'text' | 'password' | 'select';
  placeholder?: string;
  required?: boolean;
  options?: { label: string; value: string }[];
}

export interface ChatToolPluginInfo {
  type: string;
  name: string;
  icon: string;
  description: string;
  configFields: ChatToolConfigField[];
}

export const CHAT_TOOL_PLUGINS: ChatToolPluginInfo[] = [
  {
    type: 'wechat',
    name: 'WeChat',
    icon: 'comment-discussion',
    description: 'Connect personal WeChat account via Wechaty',
    configFields: [
      {
        key: 'botName',
        label: 'Bot Name',
        type: 'text',
        placeholder: 'iagenthub-wechat',
      },
    ],
  },
];
