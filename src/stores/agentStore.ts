import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  AgentConfig,
  CreateAgentRequest,
  UpdateAgentRequest,
} from '@/types/agent';
import { useChatStore } from '@/stores/chatStore';

interface AgentState {
  agents: AgentConfig[];
  selectedAgentId: string | null;
  controlHubAgentId: string | null;
  loading: boolean;
}

interface AgentActions {
  fetchAgents: () => Promise<void>;
  selectAgent: (id: string | null) => Promise<void>;
  createAgent: (req: CreateAgentRequest) => Promise<AgentConfig>;
  updateAgent: (id: string, req: UpdateAgentRequest) => Promise<AgentConfig>;
  deleteAgent: (id: string) => Promise<void>;
  setControlHub: (id: string) => Promise<AgentConfig>;
  getControlHub: () => Promise<void>;
}

export const useAgentStore = create<AgentState & AgentActions>((set, get) => ({
  agents: [],
  selectedAgentId: null,
  controlHubAgentId: null,
  loading: false,

  fetchAgents: async () => {
    set({ loading: true });
    try {
      const agents = await tauriInvoke<AgentConfig[]>('list_agents');
      const hub = agents.find((a) => a.is_control_hub);
      set({ agents, controlHubAgentId: hub?.id ?? null });
    } catch (error) {
      console.error('Failed to fetch agents:', error);
    } finally {
      set({ loading: false });
    }
  },

  selectAgent: async (id) => {
    console.log('[AgentStore] selectAgent called:', id);
    set({ selectedAgentId: id });

    // Clear the current chat state when switching agents
    const chatStore = useChatStore.getState();
    chatStore.selectSession(null); // Clear current session

    // Only fetch sessions if we have a valid agent ID
    if (id) {
      await chatStore.fetchSessions(id);
      console.log('[AgentStore] Sessions fetched for agent:', id);
    } else {
      // Clear sessions list if no agent selected
      useChatStore.setState({ sessions: [] });
      console.log('[AgentStore] Sessions cleared (no agent selected)');
    }
  },

  createAgent: async (req) => {
    try {
      const agent = await tauriInvoke<AgentConfig>('create_agent', { request: req });
      set((state) => ({ agents: [...state.agents, agent] }));
      return agent;
    } catch (error) {
      console.error('Failed to create agent:', error);
      throw error;
    }
  },

  updateAgent: async (id, req) => {
    try {
      const updated = await tauriInvoke<AgentConfig>('update_agent', { id, request: req });
      set((state) => ({
        agents: state.agents.map((a) => (a.id === id ? updated : a)),
      }));
      return updated;
    } catch (error) {
      console.error('Failed to update agent:', error);
      throw error;
    }
  },

  deleteAgent: async (id) => {
    try {
      await tauriInvoke<void>('delete_agent', { id });
      set((state) => ({
        agents: state.agents.filter((a) => a.id !== id),
        selectedAgentId: state.selectedAgentId === id ? null : state.selectedAgentId,
        controlHubAgentId: state.controlHubAgentId === id ? null : state.controlHubAgentId,
      }));
    } catch (error) {
      console.error('Failed to delete agent:', error);
      throw error;
    }
  },

  setControlHub: async (id) => {
    try {
      const updated = await tauriInvoke<AgentConfig>('set_control_hub', { agentId: id });
      set((state) => ({
        controlHubAgentId: id,
        agents: state.agents.map((a) => ({
          ...a,
          is_control_hub: a.id === id,
        })),
      }));
      return updated;
    } catch (error) {
      console.error('Failed to set control hub:', error);
      throw error;
    }
  },

  getControlHub: async () => {
    try {
      const hub = await tauriInvoke<AgentConfig | null>('get_control_hub');
      set({ controlHubAgentId: hub?.id ?? null });
    } catch (error) {
      console.error('Failed to get control hub:', error);
    }
  },
}));
