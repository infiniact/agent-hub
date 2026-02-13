import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  AgentConfig,
  CreateAgentRequest,
  UpdateAgentRequest,
} from '@/types/agent';
import { useChatStore } from '@/stores/chatStore';
import { useAcpStore } from '@/stores/acpStore';

interface AgentState {
  agents: AgentConfig[];
  selectedAgentId: string | null;
  controlHubAgentId: string | null;
  loading: boolean;
  /** Whether the ACP agent process is being initialized */
  agentInitializing: boolean;
  /** Error message from the last agent initialization attempt */
  agentError: string | null;
  /** Agent IDs that have been successfully initialized and are ready */
  readyAgentIds: string[];
  /** Whether the Kanban panel is visible */
  showKanban: boolean;
}

interface AgentActions {
  fetchAgents: () => Promise<void>;
  selectAgent: (id: string | null) => Promise<void>;
  createAgent: (req: CreateAgentRequest) => Promise<AgentConfig>;
  updateAgent: (id: string, req: UpdateAgentRequest) => Promise<AgentConfig>;
  deleteAgent: (id: string) => Promise<void>;
  setControlHub: (id: string) => Promise<AgentConfig>;
  getControlHub: () => Promise<void>;
  /** Enable a disabled agent (backend performs health check) */
  enableAgent: (id: string) => Promise<AgentConfig>;
  /** Disable an agent (no health check needed) */
  disableAgent: (id: string) => Promise<AgentConfig>;
  /** Ensure the ACP agent is spawned, initialized, and models are fetched */
  ensureAgentReady: (agentId: string, forceRefresh?: boolean) => Promise<void>;
  /** Force re-fetch models from the agent (ignores cache) */
  refreshModels: (agentId: string) => Promise<void>;
  /** Toggle or set Kanban panel visibility */
  setShowKanban: (show: boolean) => void;
}

export const useAgentStore = create<AgentState & AgentActions>((set, get) => ({
  agents: [],
  selectedAgentId: null,
  controlHubAgentId: null,
  loading: false,
  agentInitializing: false,
  agentError: null,
  readyAgentIds: [],
  showKanban: false,

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
    set({ selectedAgentId: id, agentError: null, agentInitializing: false });

    // Clear the current chat state when switching agents
    const chatStore = useChatStore.getState();
    chatStore.selectSession(null); // Clear current session

    // Only fetch sessions if we have a valid agent ID
    if (id) {
      await chatStore.fetchSessions(id);
      console.log('[AgentStore] Sessions fetched for agent:', id);

      // Load cached models from DB immediately (before live fetch)
      const agent = get().agents.find((a) => a.id === id);
      if (agent?.acp_command) {
        const acpStore = useAcpStore.getState();
        if (agent.available_models_json) {
          try {
            const cached: string[] = JSON.parse(agent.available_models_json);
            if (cached.length > 0) {
              console.log('[AgentStore] Loading cached models:', cached.length);
              acpStore.updateDiscoveredAgentModels(agent.acp_command, cached);
            } else {
              // Clear model list so previous agent's models don't leak
              acpStore.updateDiscoveredAgentModels(agent.acp_command, []);
            }
          } catch {
            acpStore.updateDiscoveredAgentModels(agent.acp_command, []);
          }
        } else {
          // No cached models — clear so previous agent's models don't show
          acpStore.updateDiscoveredAgentModels(agent.acp_command, []);
        }
      }

      // Ensure the agent process is running and models are loaded
      get().ensureAgentReady(id).catch((e) => {
        console.warn('[AgentStore] ensureAgentReady failed (non-blocking):', e);
      });
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
        readyAgentIds: state.readyAgentIds.filter((rid) => rid !== id),
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

  enableAgent: async (id) => {
    try {
      const updated = await tauriInvoke<AgentConfig>('enable_agent', { agentId: id });
      set((state) => ({
        agents: state.agents.map((a) => (a.id === id ? updated : a)),
      }));
      return updated;
    } catch (error) {
      // Re-fetch the agent to get the latest state (may have reverted to disabled)
      try {
        const agent = await tauriInvoke<AgentConfig>('get_agent', { id });
        set((state) => ({
          agents: state.agents.map((a) => (a.id === id ? agent : a)),
        }));
      } catch { /* ignore */ }
      console.error('Failed to enable agent:', error);
      throw error;
    }
  },

  disableAgent: async (id) => {
    try {
      const updated = await tauriInvoke<AgentConfig>('update_agent', {
        id,
        request: { is_enabled: false },
      });
      set((state) => ({
        agents: state.agents.map((a) => (a.id === id ? updated : a)),
      }));
      return updated;
    } catch (error) {
      console.error('Failed to disable agent:', error);
      throw error;
    }
  },

  ensureAgentReady: async (agentId, forceRefresh) => {
    if (!forceRefresh && get().readyAgentIds.includes(agentId)) {
      console.log('[AgentStore] Agent already ready, skipping:', agentId);
      return;
    }

    const agent = get().agents.find((a) => a.id === agentId);
    if (!agent || !agent.acp_command) {
      console.log('[AgentStore] Agent has no ACP command, skipping initialization');
      return;
    }

    // Only show the "Loading models…" spinner when there are no cached models.
    // If models are already cached we still connect in the background but the UI
    // shows the cached list immediately.
    let hasCachedModels = false;
    try {
      if (agent.available_models_json) {
        const cached: string[] = JSON.parse(agent.available_models_json);
        hasCachedModels = cached.length > 0;
      }
    } catch { /* ignore */ }

    console.log('[AgentStore] Ensuring agent ready:', agentId, 'forceRefresh:', !!forceRefresh, 'hasCachedModels:', hasCachedModels);
    if (!hasCachedModels) {
      set({ agentInitializing: true, agentError: null });
    } else {
      set({ agentError: null });
    }

    try {
      const result = await tauriInvoke<{ status: string; models: { model_id: string; name: string }[] }>(
        'ensure_agent_ready',
        { agentId, forceRefresh: forceRefresh || false }
      );
      console.log('[AgentStore] Agent ready:', result.status, 'models:', result.models.length);

      const acpStore = useAcpStore.getState();
      const modelIds = result.models.map((m) => m.model_id);
      acpStore.updateDiscoveredAgentModels(agent.acp_command!, modelIds);

      if (modelIds.length > 0) {
        const currentModel = agent.model;
        const needsModelUpdate = !modelIds.includes(currentModel);
        await get().updateAgent(agentId, {
          available_models_json: JSON.stringify(modelIds),
          ...(needsModelUpdate ? { model: modelIds[0] } : {}),
        });
        console.log(
          '[AgentStore] Models persisted:',
          modelIds.length,
          needsModelUpdate ? `auto-selected: ${modelIds[0]}` : `kept: ${currentModel}`
        );
      }

      set((s) => ({
        readyAgentIds: s.readyAgentIds.includes(agentId)
          ? s.readyAgentIds
          : [...s.readyAgentIds, agentId],
      }));
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : String(error);
      console.error('[AgentStore] Failed to ensure agent ready:', errorMessage);
      if (get().selectedAgentId === agentId) {
        set({ agentError: errorMessage });
      }
      set((s) => ({
        readyAgentIds: s.readyAgentIds.filter((id) => id !== agentId),
      }));
    } finally {
      if (get().selectedAgentId === agentId) {
        set({ agentInitializing: false });
      }
    }
  },

  refreshModels: async (agentId) => {
    const agent = get().agents.find((a) => a.id === agentId);
    if (!agent || !agent.acp_command) return;

    console.log('[AgentStore] Refreshing models for agent:', agentId);

    // Clear ready state so ensureAgentReady runs fresh
    set((s) => ({ readyAgentIds: s.readyAgentIds.filter((id) => id !== agentId) }));

    // Clear cached models so UI shows loading state
    const acpStore = useAcpStore.getState();
    acpStore.updateDiscoveredAgentModels(agent.acp_command, []);
    await get().updateAgent(agentId, { available_models_json: '[]' });

    // Re-run full ensureAgentReady with force refresh to get fresh models via session/new
    await get().ensureAgentReady(agentId, true);
  },

  setShowKanban: (show) => {
    set({ showKanban: show });
  },
}));

// Listen for auto-disable events from orchestration
if (isTauri()) {
  tauriListen<{ agentId: string; agentName: string; reason: string }>(
    'orchestration:agent_auto_disabled',
    (payload) => {
      console.log('[AgentStore] Agent auto-disabled:', payload.agentId, payload.reason);
      const state = useAgentStore.getState();
      const agent = state.agents.find((a) => a.id === payload.agentId);
      if (agent) {
        useAgentStore.setState({
          agents: state.agents.map((a) =>
            a.id === payload.agentId
              ? { ...a, is_enabled: false, disabled_reason: payload.reason }
              : a
          ),
        });
      }
    }
  );
}
