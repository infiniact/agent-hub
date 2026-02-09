import { create } from 'zustand';
import { tauriInvoke } from '@/lib/tauri';
import type { DiscoveredAgent } from '@/types/agent';
import type { AcpAgentStatus } from '@/types/acp';

interface AcpState {
  discoveredAgents: DiscoveredAgent[];
  agentStatuses: Record<string, string>;
  scanning: boolean;
}

interface AcpActions {
  scanForAgents: () => Promise<void>;
  spawnAgent: (agentId: string, command: string, args: string[]) => Promise<void>;
  initializeAgent: (agentId: string) => Promise<void>;
  createAcpSession: (agentId: string, sessionId: string) => Promise<void>;
  getAgentStatus: (agentId: string) => Promise<string>;
  stopAgent: (agentId: string) => Promise<void>;
}

export const useAcpStore = create<AcpState & AcpActions>((set, get) => ({
  discoveredAgents: [],
  agentStatuses: {},
  scanning: false,

  scanForAgents: async () => {
    set({ scanning: true });
    try {
      const discoveredAgents = await tauriInvoke<DiscoveredAgent[]>('discover_agents');
      set({ discoveredAgents });
    } catch (error) {
      console.error('Failed to scan for agents:', error);
    } finally {
      set({ scanning: false });
    }
  },

  spawnAgent: async (agentId, command, args) => {
    try {
      await tauriInvoke<void>('spawn_agent', { agentId, command, args });
      set((state) => ({
        agentStatuses: { ...state.agentStatuses, [agentId]: 'spawned' },
      }));
    } catch (error) {
      console.error('Failed to spawn agent:', error);
      throw error;
    }
  },

  initializeAgent: async (agentId) => {
    try {
      await tauriInvoke<void>('initialize_agent', { agentId });
      set((state) => ({
        agentStatuses: { ...state.agentStatuses, [agentId]: 'initialized' },
      }));
    } catch (error) {
      console.error('Failed to initialize agent:', error);
      throw error;
    }
  },

  createAcpSession: async (agentId, sessionId) => {
    try {
      await tauriInvoke<void>('create_acp_session', { agentId, sessionId });
    } catch (error) {
      console.error('Failed to create ACP session:', error);
      throw error;
    }
  },

  getAgentStatus: async (agentId) => {
    try {
      const result = await tauriInvoke<AcpAgentStatus>('get_agent_status', { agentId });
      set((state) => ({
        agentStatuses: { ...state.agentStatuses, [agentId]: result.status },
      }));
      return result.status;
    } catch (error) {
      console.error('Failed to get agent status:', error);
      throw error;
    }
  },

  stopAgent: async (agentId) => {
    try {
      await tauriInvoke<void>('stop_agent', { agentId });
      set((state) => ({
        agentStatuses: { ...state.agentStatuses, [agentId]: 'stopped' },
      }));
    } catch (error) {
      console.error('Failed to stop agent:', error);
      throw error;
    }
  },
}));
