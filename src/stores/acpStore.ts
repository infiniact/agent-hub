import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { tauriInvoke } from '@/lib/tauri';
import type { DiscoveredAgent, AgentModel } from '@/types/agent';
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
  createAcpSession: (agentId: string, sessionId: string, command: string) => Promise<void>;
  getAgentModels: (agentId: string, command: string) => Promise<void>;
  getAgentStatus: (agentId: string) => Promise<string>;
  stopAgent: (agentId: string) => Promise<void>;
  updateDiscoveredAgentModels: (command: string, models: string[]) => void;
  endAcpSession: (sessionId: string) => Promise<void>;
  resumeAcpSession: (sessionId: string) => Promise<{ acpSessionId: string; isLoaded: boolean; models: AgentModel[] }>;
  installAgent: (registryId: string) => Promise<void>;
  uninstallAgent: (registryId: string) => Promise<void>;
}

export const useAcpStore = create<AcpState & AcpActions>()(
  persist(
    (set, get) => ({
      discoveredAgents: [],
      agentStatuses: {},
      scanning: false,

      scanForAgents: async () => {
        set({ scanning: true });
        try {
          const fresh = await tauriInvoke<DiscoveredAgent[]>('discover_agents');
          // Merge: keep cached models for agents that the fresh scan returns with empty models
          const cached = get().discoveredAgents;
          const merged = fresh.map((agent) => {
            if (agent.models && agent.models.length > 0) return agent;
            const prev = cached.find((c) => c.command === agent.command);
            if (prev && prev.models && prev.models.length > 0) {
              return { ...agent, models: prev.models };
            }
            return agent;
          });
          set({ discoveredAgents: merged });
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

          // After initialization, try to get models from the agent
          // Find the discovered agent that matches this agentId
          const discovered = get().discoveredAgents.find(d => d.name === 'Claude Code' || d.available);
          if (discovered) {
            console.log('[AcpStore] Fetching models for agent:', discovered.command);
            await get().getAgentModels(agentId, discovered.command);
          }
        } catch (error) {
          console.error('Failed to initialize agent:', error);
          throw error;
        }
      },

      createAcpSession: async (agentId, sessionId, command) => {
        try {
          const result = await tauriInvoke<{ acpSessionId: string; models: AgentModel[] }>(
            'create_acp_session',
            { agentId, sessionId }
          );
          // Update the discovered agents with the models from ACP
          const modelIds = result.models.map((m) => m.model_id);
          get().updateDiscoveredAgentModels(command, modelIds);
        } catch (error) {
          console.error('Failed to create ACP session:', error);
          throw error;
        }
      },

      getAgentModels: async (agentId, command) => {
        try {
          const result = await tauriInvoke<{ models: AgentModel[]; temp_acp_session_id: string }>(
            'get_agent_models',
            { agentId }
          );
          console.log('[AcpStore] Got models from agent:', result.models, 'temp session:', result.temp_acp_session_id);
          const modelIds = result.models.map((m) => m.model_id);
          get().updateDiscoveredAgentModels(command, modelIds);
        } catch (error) {
          console.error('Failed to get agent models:', error);
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

      updateDiscoveredAgentModels: (command, models) => {
        set((state) => ({
          discoveredAgents: state.discoveredAgents.map((agent) =>
            agent.command === command ? { ...agent, models } : agent
          ),
        }));
      },

      endAcpSession: async (sessionId) => {
        try {
          await tauriInvoke<void>('end_acp_session', { sessionId });
        } catch (error) {
          console.error('Failed to end ACP session:', error);
          throw error;
        }
      },

      resumeAcpSession: async (sessionId) => {
        try {
          const result = await tauriInvoke<{ acpSessionId: string; isLoaded: boolean; models: AgentModel[] }>(
            'resume_acp_session',
            { sessionId }
          );
          console.log('[AcpStore] Resumed ACP session:', result);
          return result;
        } catch (error) {
          console.error('Failed to resume ACP session:', error);
          throw error;
        }
      },

      installAgent: async (registryId) => {
        try {
          const updated = await tauriInvoke<DiscoveredAgent[]>(
            'install_registry_agent',
            { registryId }
          );
          // Merge models cache from previous state
          const cached = get().discoveredAgents;
          const merged = updated.map((agent) => {
            if (agent.models && agent.models.length > 0) return agent;
            const prev = cached.find((c) => c.command === agent.command);
            if (prev && prev.models && prev.models.length > 0) {
              return { ...agent, models: prev.models };
            }
            return agent;
          });
          set({ discoveredAgents: merged });
        } catch (error) {
          console.error('Failed to install agent:', error);
          throw error;
        }
      },

      uninstallAgent: async (registryId) => {
        try {
          const updated = await tauriInvoke<DiscoveredAgent[]>(
            'uninstall_registry_agent',
            { registryId }
          );
          const cached = get().discoveredAgents;
          const merged = updated.map((agent) => {
            if (agent.models && agent.models.length > 0) return agent;
            const prev = cached.find((c) => c.command === agent.command);
            if (prev && prev.models && prev.models.length > 0) {
              return { ...agent, models: prev.models };
            }
            return agent;
          });
          set({ discoveredAgents: merged });
        } catch (error) {
          console.error('Failed to uninstall agent:', error);
          throw error;
        }
      },
    }),
    {
      name: 'acp-store',
      partialize: (state) => ({
        // Only persist discoveredAgents (with their models cache)
        discoveredAgents: state.discoveredAgents,
      }),
    }
  )
);
