import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  ChatTool,
  CreateChatToolRequest,
  UpdateChatToolRequest,
  ChatToolMessage,
  ChatToolContact,
} from '@/types/chatTool';

/** Resolve the currently selected chat tool ID for the active workspace. */
function getSelectedId(byWorkspace: Record<string, string | null>): string | null {
  // Lazy import to avoid circular dependency
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const { useWorkspaceStore } = require('@/stores/workspaceStore') as typeof import('@/stores/workspaceStore');
  const wsId = useWorkspaceStore.getState().activeWorkspaceId ?? '__global__';
  return byWorkspace[wsId] ?? null;
}

interface ChatToolState {
  chatTools: ChatTool[];
  /** Per-workspace selected chat tool ID map */
  selectedChatToolIdByWorkspace: Record<string, string | null>;
  messages: ChatToolMessage[];
  contacts: ChatToolContact[];
  qrCodeUrl: string | null;
  qrCodeImage: string | null;
  loading: boolean;
  messagesLoading: boolean;
}

interface ChatToolActions {
  fetchChatTools: () => Promise<void>;
  selectChatTool: (id: string | null) => void;
  /** Derived getter: current workspace's selected chat tool ID */
  getSelectedChatToolId: () => string | null;
  createChatTool: (req: CreateChatToolRequest) => Promise<ChatTool>;
  updateChatTool: (id: string, req: UpdateChatToolRequest) => Promise<ChatTool>;
  deleteChatTool: (id: string) => Promise<void>;
  startChatTool: (id: string) => Promise<void>;
  stopChatTool: (id: string) => Promise<void>;
  logoutChatTool: (id: string) => Promise<void>;
  fetchMessages: (chatToolId: string) => Promise<void>;
  fetchContacts: (chatToolId: string) => Promise<void>;
  setContactBlocked: (contactId: string, blocked: boolean) => Promise<void>;
  sendMessage: (chatToolId: string, toId: string, content: string) => Promise<void>;
  getQrCode: (id: string) => Promise<void>;
}

export const useChatToolStore = create<ChatToolState & ChatToolActions>(
  (set, get) => ({
    chatTools: [],
    selectedChatToolIdByWorkspace: {},
    messages: [],
    contacts: [],
    qrCodeUrl: null,
    qrCodeImage: null,
    loading: false,
    messagesLoading: false,

    getSelectedChatToolId: () => {
      return getSelectedId(get().selectedChatToolIdByWorkspace);
    },

    fetchChatTools: async () => {
      if (!isTauri()) return;
      set({ loading: true });
      try {
        const { useWorkspaceStore } = await import('@/stores/workspaceStore');
        const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
        const chatTools = await tauriInvoke<ChatTool[]>('list_chat_tools', {
          workspaceId: workspaceId ?? undefined,
        });
        set({ chatTools, loading: false });
      } catch (error) {
        console.error('[ChatToolStore] Failed to fetch chat tools:', error);
        set({ loading: false });
      }
    },

    selectChatTool: (id) => {
      // Lazy import to avoid circular dependency
      const { useWorkspaceStore } = require('@/stores/workspaceStore') as typeof import('@/stores/workspaceStore');
      const wsId = useWorkspaceStore.getState().activeWorkspaceId ?? '__global__';

      set((state) => ({
        selectedChatToolIdByWorkspace: {
          ...state.selectedChatToolIdByWorkspace,
          [wsId]: id,
        },
        messages: [],
        contacts: [],
        qrCodeUrl: null,
        qrCodeImage: null,
      }));
      if (id) {
        get().fetchMessages(id);
        get().fetchContacts(id);
        get().getQrCode(id);
      }
    },

    createChatTool: async (req) => {
      const chatTool = await tauriInvoke<ChatTool>('create_chat_tool', { request: req });
      set((state) => ({ chatTools: [...state.chatTools, chatTool] }));
      return chatTool;
    },

    updateChatTool: async (id, req) => {
      const updated = await tauriInvoke<ChatTool>('update_chat_tool', { id, request: req });
      set((state) => ({
        chatTools: state.chatTools.map((t) => (t.id === id ? updated : t)),
      }));
      return updated;
    },

    deleteChatTool: async (id) => {
      await tauriInvoke('delete_chat_tool', { id });
      const selectedId = get().getSelectedChatToolId();
      if (selectedId === id) {
        // Clear selection for current workspace
        const { useWorkspaceStore } = require('@/stores/workspaceStore') as typeof import('@/stores/workspaceStore');
        const wsId = useWorkspaceStore.getState().activeWorkspaceId ?? '__global__';
        set((state) => ({
          chatTools: state.chatTools.filter((t) => t.id !== id),
          selectedChatToolIdByWorkspace: {
            ...state.selectedChatToolIdByWorkspace,
            [wsId]: null,
          },
        }));
      } else {
        set((state) => ({
          chatTools: state.chatTools.filter((t) => t.id !== id),
        }));
      }
    },

    startChatTool: async (id) => {
      await tauriInvoke('start_chat_tool', { id });
    },

    stopChatTool: async (id) => {
      // Eagerly update local state so UI responds immediately
      set((state) => ({
        chatTools: state.chatTools.map((t) =>
          t.id === id ? { ...t, status: 'stopped', status_message: null } : t
        ),
      }));
      await tauriInvoke('stop_chat_tool', { id });
    },

    logoutChatTool: async (id) => {
      // Clear QR code state so new one will be shown
      set({ qrCodeUrl: null, qrCodeImage: null });
      await tauriInvoke('logout_chat_tool', { id });
    },

    fetchMessages: async (chatToolId) => {
      if (!isTauri()) return;
      set({ messagesLoading: true });
      try {
        const messages = await tauriInvoke<ChatToolMessage[]>('list_chat_tool_messages', {
          chatToolId,
          limit: 100,
          offset: 0,
        });
        set({ messages, messagesLoading: false });
      } catch (error) {
        console.error('[ChatToolStore] Failed to fetch messages:', error);
        set({ messagesLoading: false });
      }
    },

    fetchContacts: async (chatToolId) => {
      if (!isTauri()) return;
      try {
        const contacts = await tauriInvoke<ChatToolContact[]>('list_chat_tool_contacts', {
          chatToolId,
        });
        set({ contacts });
      } catch (error) {
        console.error('[ChatToolStore] Failed to fetch contacts:', error);
      }
    },

    setContactBlocked: async (contactId, blocked) => {
      await tauriInvoke('set_chat_tool_contact_blocked', { contactId, blocked });
      set((state) => ({
        contacts: state.contacts.map((c) =>
          c.id === contactId ? { ...c, is_blocked: blocked } : c
        ),
      }));
    },

    sendMessage: async (chatToolId, toId, content) => {
      await tauriInvoke('send_chat_tool_message', {
        chatToolId,
        toId,
        content,
      });
    },

    getQrCode: async (id) => {
      if (!isTauri()) return;
      try {
        const qrCode = await tauriInvoke<string | null>('get_chat_tool_qr_code', { id });
        console.log('[ChatToolStore] getQrCode result for', id, ':', qrCode ? `${qrCode.length} chars` : 'null');
        if (qrCode) {
          set({ qrCodeImage: qrCode, qrCodeUrl: null });
        }
      } catch {
        // QR code may not be available
      }
    },
  })
);

// Register Tauri event listeners
if (isTauri()) {
  tauriListen<{ chatToolId: string; status: string; message: string | null }>(
    'chat_tool:status_changed',
    (payload) => {
      const state = useChatToolStore.getState();
      useChatToolStore.setState({
        chatTools: state.chatTools.map((t) =>
          t.id === payload.chatToolId
            ? { ...t, status: payload.status, status_message: payload.message }
            : t
        ),
      });
    }
  );

  tauriListen<{ chatToolId: string; url: string; imageBase64: string }>(
    'chat_tool:qr_code',
    (payload) => {
      console.log('[ChatToolStore] QR code event received:', payload.chatToolId, 'imageBase64 length:', payload.imageBase64?.length, 'url:', payload.url);
      const state = useChatToolStore.getState();
      // Always store QR data — the component will only render it for the selected tool
      useChatToolStore.setState({
        qrCodeUrl: payload.url,
        qrCodeImage: payload.imageBase64,
        chatTools: state.chatTools.map((t) =>
          t.id === payload.chatToolId
            ? { ...t, status: 'login_required', status_message: 'Scan QR code to login' }
            : t
        ),
      });
    }
  );

  tauriListen<{ chatToolId: string; userId: string; userName: string }>(
    'chat_tool:login',
    (payload) => {
      const state = useChatToolStore.getState();
      const selectedId = state.getSelectedChatToolId();
      useChatToolStore.setState({
        chatTools: state.chatTools.map((t) =>
          t.id === payload.chatToolId
            ? { ...t, status: 'running', status_message: `Logged in as ${payload.userName}` }
            : t
        ),
        qrCodeUrl: selectedId === payload.chatToolId ? null : state.qrCodeUrl,
        qrCodeImage: selectedId === payload.chatToolId ? null : state.qrCodeImage,
      });
    }
  );

  tauriListen<{ chatToolId: string }>(
    'chat_tool:logout',
    (payload) => {
      const state = useChatToolStore.getState();
      useChatToolStore.setState({
        chatTools: state.chatTools.map((t) =>
          t.id === payload.chatToolId
            ? { ...t, status: 'stopped', status_message: 'Logged out' }
            : t
        ),
      });
    }
  );

  tauriListen<{ chatToolId: string; message: ChatToolMessage }>(
    'chat_tool:message_received',
    (payload) => {
      const state = useChatToolStore.getState();
      const selectedId = state.getSelectedChatToolId();
      if (selectedId === payload.chatToolId) {
        useChatToolStore.setState({
          messages: [payload.message, ...state.messages],
        });
      }
      // Update message count
      useChatToolStore.setState({
        chatTools: state.chatTools.map((t) =>
          t.id === payload.chatToolId
            ? { ...t, messages_received: t.messages_received + 1 }
            : t
        ),
      });
    }
  );

  tauriListen<{ chatToolId: string; messageId: string; agentResponse: string }>(
    'chat_tool:message_processed',
    (payload) => {
      const state = useChatToolStore.getState();
      const selectedId = state.getSelectedChatToolId();
      if (selectedId === payload.chatToolId) {
        useChatToolStore.setState({
          messages: state.messages.map((m) =>
            m.id === payload.messageId
              ? { ...m, is_processed: true, agent_response: payload.agentResponse }
              : m
          ),
        });
      }
    }
  );

  tauriListen<{ chatToolId: string; error: string }>(
    'chat_tool:error',
    (payload) => {
      const state = useChatToolStore.getState();
      useChatToolStore.setState({
        chatTools: state.chatTools.map((t) =>
          t.id === payload.chatToolId
            ? { ...t, status: 'error', status_message: payload.error }
            : t
        ),
      });
    }
  );
}
