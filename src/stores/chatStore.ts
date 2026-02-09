import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  ChatMessage,
  Session,
  CreateSessionRequest,
} from '@/types/chat';
import { useEffect } from 'react';

interface PermissionRequest {
  id: number | string;
  sessionId: string;
  toolCall?: {
    toolCallId: string;
    title: string;
    rawInput?: any;
  };
  options: Array<{
    optionId: string;
    name: string;
    kind: string;
  }>;
}

interface ChatState {
  sessions: Session[];
  currentSessionId: string | null;
  messages: ChatMessage[];
  isStreaming: boolean;
  streamedContent: string;
  toolCalls: any[];
  pendingPermission: PermissionRequest | null;
}

interface ChatActions {
  fetchSessions: (agentId: string) => Promise<void>;
  createSession: (req: CreateSessionRequest) => Promise<Session>;
  ensureSession: (agentId: string) => Promise<string>;
  deleteSession: (id: string) => Promise<void>;
  selectSession: (id: string | null) => void;
  fetchMessages: (sessionId: string) => Promise<void>;
  sendPrompt: (sessionId: string, content: string) => Promise<void>;
  cancelPrompt: (sessionId: string) => Promise<void>;
  appendStreamChunk: (chunk: string) => void;
  completeMessage: (msg: ChatMessage) => void;
  clearStream: () => void;
  addToolCall: (toolCall: any) => void;
  updateToolCall: (toolCallUpdate: any) => void;
  addUserMessage: (content: string) => void;
  respondToPermission: (agentId: string, optionId: string, userMessage?: string) => Promise<void>;
  clearPendingPermission: () => void;
}

export const useChatStore = create<ChatState & ChatActions>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  messages: [],
  isStreaming: false,
  streamedContent: '',
  toolCalls: [],
  pendingPermission: null,

  fetchSessions: async (agentId) => {
    try {
      const sessions = await tauriInvoke<Session[]>('list_sessions', { agentId });
      set({ sessions });
    } catch (error) {
      console.error('Failed to fetch sessions:', error);
    }
  },

  createSession: async (req) => {
    console.log('[ChatStore] Creating session:', req);
    try {
      const session = await tauriInvoke<Session>('create_session', { request: req });
      set((state) => ({ sessions: [...state.sessions, session] }));
      console.log('[ChatStore] Session created:', session);
      return session;
    } catch (error) {
      console.error('Failed to create session:', error);
      throw error;
    }
  },

  // Ensure a session exists for the current agent, create one if needed
  ensureSession: async (agentId) => {
    console.log('[ChatStore] ensureSession called with agentId:', agentId);

    if (!agentId) {
      throw new Error('Invalid agent ID: cannot create session without agent');
    }

    const state = get();
    console.log('[ChatStore] Current state:', {
      currentSessionId: state.currentSessionId,
      sessionsCount: state.sessions.length,
      sessions: state.sessions.map(s => ({ id: s.id, agent_id: s.agent_id }))
    });

    // If we already have a current session, use it
    if (state.currentSessionId) {
      console.log('[ChatStore] Using existing session:', state.currentSessionId);
      return state.currentSessionId;
    }

    // Try to find an existing session for this agent
    if (state.sessions.length > 0) {
      const existingSession = state.sessions.find((s) => s.agent_id === agentId);
      if (existingSession) {
        console.log('[ChatStore] Found existing session:', existingSession.id);
        set({ currentSessionId: existingSession.id });
        // Load previous messages for context persistence
        await get().fetchMessages(existingSession.id);
        return existingSession.id;
      }
    }

    // Create a new session
    console.log('[ChatStore] Creating new session for agent:', agentId);
    const newSession = await get().createSession({
      agent_id: agentId,
      title: 'New Conversation',
      mode: 'chat',
    });
    console.log('[ChatStore] Created new session:', newSession.id);
    set({ currentSessionId: newSession.id });
    return newSession.id;
  },

  deleteSession: async (id) => {
    try {
      await tauriInvoke<void>('delete_session', { id });
      set((state) => ({
        sessions: state.sessions.filter((s) => s.id !== id),
        currentSessionId: state.currentSessionId === id ? null : state.currentSessionId,
        messages: state.currentSessionId === id ? [] : state.messages,
      }));
    } catch (error) {
      console.error('Failed to delete session:', error);
      throw error;
    }
  },

  selectSession: (id) => {
    console.log('[ChatStore] selectSession called with:', id);
    set({ currentSessionId: id, messages: [], streamedContent: '' });
  },

  fetchMessages: async (sessionId) => {
    try {
      const messages = await tauriInvoke<ChatMessage[]>('get_messages', { sessionId });
      set({ messages });
    } catch (error) {
      console.error('Failed to fetch messages:', error);
    }
  },

  sendPrompt: async (sessionId, content) => {
    console.log('[ChatStore] sendPrompt called - sessionId:', sessionId, 'content:', content);

    // Validate sessionId
    if (!sessionId) {
      const error = 'No active session. Please select an agent first.';
      console.error('[ChatStore]', error);
      get().addToolCall({
        id: `error-${Date.now()}`,
        name: 'Error',
        status: 'failed',
        result: error,
      });
      throw new Error(error);
    }

    set({ isStreaming: true, streamedContent: '', toolCalls: [] });
    try {
      const result = await tauriInvoke<ChatMessage>('send_prompt', { sessionId, content });
      console.log('[ChatStore] sendPrompt result:', result);
      // Add user message from backend (it's already saved to DB with proper ID)
      set((state) => ({ messages: [...state.messages, result] }));
    } catch (error) {
      console.error('[ChatStore] Failed to send prompt:', error);
      // Show error to user
      const errorMsg = error instanceof Error ? error.message : String(error);
      get().addToolCall({
        id: `error-${Date.now()}`,
        name: 'Error',
        status: 'failed',
        result: `Failed to send message: ${errorMsg}`,
      });
      set({ isStreaming: false });
      throw error;
    }
  },

  cancelPrompt: async (sessionId) => {
    try {
      await tauriInvoke<void>('cancel_prompt', { sessionId });
      set({ isStreaming: false });
    } catch (error) {
      console.error('Failed to cancel prompt:', error);
    }
  },

  appendStreamChunk: (chunk) => {
    set((state) => ({ streamedContent: state.streamedContent + chunk }));
  },

  completeMessage: (msg) => {
    const state = get();

    // Build the completed message using accumulated streamed content and tool calls,
    // since the backend msg may only contain the ACP result (not the full text).
    const contentBlocks: Array<{ type: string; text: string }> = [];
    if (state.streamedContent) {
      contentBlocks.push({ type: 'text', text: state.streamedContent });
    }

    // If we have streamed content, use it; otherwise fall back to backend msg
    const contentJson = contentBlocks.length > 0
      ? JSON.stringify(contentBlocks)
      : msg.content_json;

    // Preserve tool calls in the completed message
    const toolCallsJson = state.toolCalls.length > 0
      ? JSON.stringify(state.toolCalls)
      : msg.tool_calls_json;

    const completedMsg: ChatMessage = {
      ...msg,
      content_json: contentJson,
      tool_calls_json: toolCallsJson,
    };

    set((state) => ({
      messages: [...state.messages, completedMsg],
      isStreaming: false,
      streamedContent: '',
      toolCalls: [],
    }));
  },

  clearStream: () => {
    set({ isStreaming: false, streamedContent: '' });
  },

  addToolCall: (toolCall) => {
    set((state) => {
      // Deduplicate: if a tool call with the same ID exists, update it instead
      const exists = state.toolCalls.some((tc) => tc.id === toolCall.id);
      if (exists) {
        return {
          toolCalls: state.toolCalls.map((tc) =>
            tc.id === toolCall.id ? { ...tc, ...toolCall } : tc
          ),
        };
      }
      return { toolCalls: [...state.toolCalls, toolCall] };
    });
  },

  updateToolCall: (toolCallUpdate) => {
    set((state) => ({
      toolCalls: state.toolCalls.map((tc) =>
        tc.id === toolCallUpdate.id ? { ...tc, ...toolCallUpdate } : tc
      ),
    }));
  },

  addUserMessage: (content) => {
    const state = get();
    const userMsg: ChatMessage = {
      id: `local-${Date.now()}`,
      session_id: state.currentSessionId || '',
      role: 'User',
      content_json: JSON.stringify([{ type: 'text', text: content }]),
      tool_calls_json: null,
      created_at: new Date().toISOString(),
    };
    console.log('[ChatStore] Adding user message:', userMsg);
    set((state) => ({ messages: [...state.messages, userMsg] }));
  },

  respondToPermission: async (agentId: string, optionId: string, userMessage?: string) => {
    const state = get();
    const requestId = state.pendingPermission?.id;
    if (!requestId) {
      console.error('[ChatStore] No pending permission to respond to');
      return;
    }
    try {
      await tauriInvoke('respond_permission', {
        agentId,
        requestId,
        optionId,
        userMessage,
      });
      console.log('[ChatStore] Permission response sent');
    } catch (error) {
      console.error('[ChatStore] Failed to send permission response:', error);
    }
  },

  clearPendingPermission: () => {
    set({ pendingPermission: null });
  },
}));

// Initialize Tauri event listeners
let unlistenFns: Array<() => void> = [];

export function initializeChatListeners() {
  if (!isTauri() || unlistenFns.length > 0) {
    console.log('[ChatStore] Skipping listener initialization:', { isTauri: isTauri(), alreadyInitialized: unlistenFns.length > 0 });
    return;
  }

  console.log('[ChatStore] Initializing Tauri event listeners');

  // Listen for streaming message chunks
  tauriListen<any>('acp:agent_message_chunk', (payload) => {
    // payload is the full JSON-RPC message: { params: { update: { content: { type, text } } } }
    const update = payload?.params?.update;
    if (update?.content) {
      const contentBlock = update.content;
      if (contentBlock.type === 'text' && contentBlock.text) {
        useChatStore.getState().appendStreamChunk(contentBlock.text);
      }
    }
  }).then((unlisten) => {
    console.log('[ChatStore] agent_message_chunk listener registered');
    unlistenFns.push(unlisten);
  }).catch((e) => {
    console.error('[ChatStore] Failed to register agent_message_chunk listener:', e);
  });

  // Listen for agent thought chunks
  tauriListen<any>('acp:agent_thought_chunk', (payload) => {
    const update = payload?.params?.update;
    if (update?.content?.type === 'text' && update?.content?.text) {
      // Append thinking content with a visual marker
      useChatStore.getState().appendStreamChunk(update.content.text);
    }
  }).then((unlisten) => {
    console.log('[ChatStore] agent_thought_chunk listener registered');
    unlistenFns.push(unlisten);
  });

  // Listen for tool calls
  tauriListen<any>('acp:tool_call', (payload) => {
    const update = payload?.params?.update;
    if (update) {
      useChatStore.getState().addToolCall({
        id: update.toolCallId,
        name: update._meta?.claudeCode?.toolName || update.title || 'Unknown',
        title: update.title,
        status: update.status || 'pending',
        rawInput: update.rawInput,
      });
    }
  }).then((unlisten) => {
    console.log('[ChatStore] tool_call listener registered');
    unlistenFns.push(unlisten);
  }).catch((e) => {
    console.error('[ChatStore] Failed to register tool_call listener:', e);
  });

  // Listen for tool call updates
  tauriListen<any>('acp:tool_call_update', (payload) => {
    const update = payload?.params?.update;
    if (update) {
      useChatStore.getState().updateToolCall({
        id: update.toolCallId,
        status: update.status || 'completed',
        rawOutput: update.rawOutput,
      });
    }
  }).then((unlisten) => {
    console.log('[ChatStore] tool_call_update listener registered');
    unlistenFns.push(unlisten);
  }).catch((e) => {
    console.error('[ChatStore] Failed to register tool_call_update listener:', e);
  });

  // Listen for plan events
  tauriListen<any>('acp:plan', (payload) => {
    console.log('[ChatStore] Received plan:', payload);
  }).then((unlisten) => {
    console.log('[ChatStore] plan listener registered');
    unlistenFns.push(unlisten);
  });

  // Listen for permission requests
  tauriListen<any>('acp:permission_request', (payload) => {
    console.log('[ChatStore] Received permission_request:', payload);

    // Extract permission request details
    const params = payload?.params;
    if (params) {
      // Use payload.id if it exists (including 0), otherwise fallback to timestamp
      const id = payload.id !== undefined ? payload.id : Date.now();
      const permissionRequest: PermissionRequest = {
        id,
        sessionId: params.sessionId || '',
        toolCall: params.toolCall,
        options: params.options || [],
      };
      // Set pending permission - this will trigger the dialog
      useChatStore.setState({ pendingPermission: permissionRequest });
    }
  }).then((unlisten) => {
    console.log('[ChatStore] permission_request listener registered');
    unlistenFns.push(unlisten);
  });

  // Listen for message completion
  tauriListen<ChatMessage>('acp:message_complete', (message) => {
    console.log('[ChatStore] Received message_complete:', message);
    useChatStore.getState().completeMessage(message);
  }).then((unlisten) => {
    console.log('[ChatStore] message_complete listener registered');
    unlistenFns.push(unlisten);
  }).catch((e) => {
    console.error('[ChatStore] Failed to register message_complete listener:', e);
  });

  // Listen for errors
  tauriListen<any>('acp:error', (payload) => {
    console.error('[ChatStore] Received error:', payload);
    const errorMessage = payload?.message || payload?.data || 'Unknown agent error';
    useChatStore.getState().addToolCall({
      id: `error-${Date.now()}`,
      name: 'Error',
      title: typeof errorMessage === 'string' ? errorMessage : JSON.stringify(errorMessage),
      status: 'failed',
    });
    useChatStore.setState({ isStreaming: false });
  }).then((unlisten) => {
    console.log('[ChatStore] error listener registered');
    unlistenFns.push(unlisten);
  });

  // Listen for agent started events
  tauriListen<any>('acp:agent_started', (payload) => {
    console.log('[ChatStore] Agent started:', payload);
  }).then((unlisten) => {
    console.log('[ChatStore] agent_started listener registered');
    unlistenFns.push(unlisten);
  });

  console.log('[ChatStore] All event listeners initialized, count:', unlistenFns.length);
}

export function cleanupChatListeners() {
  console.log('[ChatStore] Cleaning up event listeners');
  unlistenFns.forEach((unlisten) => unlisten());
  unlistenFns = [];
}
