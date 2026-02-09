import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  TaskRun,
  TaskAssignment,
  TaskPlan,
  AgentTrackingInfo,
} from '@/types/orchestration';

interface OrchestrationState {
  activeTaskRun: TaskRun | null;
  assignments: TaskAssignment[];
  agentTracking: Record<string, AgentTrackingInfo>;
  isOrchestrating: boolean;
  streamingAgentId: string | null;
  streamedContent: string;
  taskPlan: TaskPlan | null;
  taskRuns: TaskRun[];
}

interface OrchestrationActions {
  startOrchestration: (prompt: string) => Promise<void>;
  cancelOrchestration: () => Promise<void>;
  fetchTaskRuns: () => Promise<void>;
  fetchAssignments: (taskRunId: string) => Promise<void>;
  reset: () => void;
}

export const useOrchestrationStore = create<OrchestrationState & OrchestrationActions>(
  (set, get) => ({
    activeTaskRun: null,
    assignments: [],
    agentTracking: {},
    isOrchestrating: false,
    streamingAgentId: null,
    streamedContent: '',
    taskPlan: null,
    taskRuns: [],

    startOrchestration: async (prompt: string) => {
      set({
        isOrchestrating: true,
        activeTaskRun: null,
        assignments: [],
        agentTracking: {},
        streamingAgentId: null,
        streamedContent: '',
        taskPlan: null,
      });

      try {
        const taskRun = await tauriInvoke<TaskRun>('start_orchestration', {
          request: { user_prompt: prompt, title: '' },
        });
        set({ activeTaskRun: taskRun });
      } catch (error) {
        console.error('[Orchestration] Failed to start:', error);
        set({ isOrchestrating: false });
        throw error;
      }
    },

    cancelOrchestration: async () => {
      const { activeTaskRun } = get();
      if (!activeTaskRun) return;

      try {
        await tauriInvoke('cancel_orchestration', {
          taskRunId: activeTaskRun.id,
        });
      } catch (error) {
        console.error('[Orchestration] Failed to cancel:', error);
      }
      set({ isOrchestrating: false });
    },

    fetchTaskRuns: async () => {
      try {
        const runs = await tauriInvoke<TaskRun[]>('list_task_runs');
        set({ taskRuns: runs });
      } catch (error) {
        console.error('[Orchestration] Failed to fetch task runs:', error);
      }
    },

    fetchAssignments: async (taskRunId: string) => {
      try {
        const assignments = await tauriInvoke<TaskAssignment[]>('get_task_assignments', {
          taskRunId,
        });
        set({ assignments });
      } catch (error) {
        console.error('[Orchestration] Failed to fetch assignments:', error);
      }
    },

    reset: () => {
      set({
        activeTaskRun: null,
        assignments: [],
        agentTracking: {},
        isOrchestrating: false,
        streamingAgentId: null,
        streamedContent: '',
        taskPlan: null,
      });
    },
  })
);

// Orchestration event listeners
let orchestrationUnlistenFns: Array<() => void> = [];

export function initializeOrchestrationListeners() {
  if (!isTauri() || orchestrationUnlistenFns.length > 0) return;

  console.log('[Orchestration] Initializing event listeners');

  // orchestration:started
  tauriListen<any>('orchestration:started', (payload) => {
    console.log('[Orchestration] Started:', payload);
    useOrchestrationStore.setState((state) => ({
      activeTaskRun: state.activeTaskRun
        ? { ...state.activeTaskRun, status: payload?.status || 'analyzing' }
        : state.activeTaskRun,
    }));
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:plan_ready
  tauriListen<any>('orchestration:plan_ready', (payload) => {
    console.log('[Orchestration] Plan ready:', payload);
    const plan = payload?.plan as TaskPlan | undefined;
    useOrchestrationStore.setState({ taskPlan: plan ?? null });
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_started
  tauriListen<any>('orchestration:agent_started', (payload) => {
    console.log('[Orchestration] Agent started:', payload);
    if (payload?.agentId) {
      useOrchestrationStore.setState((state) => ({
        streamingAgentId: payload.agentId,
        streamedContent: '',
        agentTracking: {
          ...state.agentTracking,
          [payload.agentId]: {
            agentId: payload.agentId,
            agentName: payload.agentName || '',
            model: payload.model || '',
            status: 'running',
            tokensIn: 0,
            tokensOut: 0,
            durationMs: 0,
            streamedContent: '',
          },
        },
      }));
    }
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_chunk
  tauriListen<any>('orchestration:agent_chunk', (payload) => {
    if (payload?.text && payload?.agentId) {
      useOrchestrationStore.setState((state) => {
        const existing = state.agentTracking[payload.agentId];
        return {
          streamedContent: state.streamedContent + payload.text,
          agentTracking: existing
            ? {
                ...state.agentTracking,
                [payload.agentId]: {
                  ...existing,
                  streamedContent: existing.streamedContent + payload.text,
                },
              }
            : state.agentTracking,
        };
      });
    }
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_completed
  tauriListen<any>('orchestration:agent_completed', (payload) => {
    console.log('[Orchestration] Agent completed:', payload);
    if (payload?.agentId) {
      useOrchestrationStore.setState((state) => {
        const existing = state.agentTracking[payload.agentId];
        return {
          streamingAgentId: null,
          streamedContent: '',
          agentTracking: existing
            ? {
                ...state.agentTracking,
                [payload.agentId]: {
                  ...existing,
                  status: payload.status || 'completed',
                  durationMs: payload.durationMs || 0,
                },
              }
            : state.agentTracking,
        };
      });
    }
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:feedback
  tauriListen<any>('orchestration:feedback', (payload) => {
    console.log('[Orchestration] Feedback:', payload);
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:completed
  tauriListen<any>('orchestration:completed', (payload) => {
    console.log('[Orchestration] Completed:', payload);
    useOrchestrationStore.setState((state) => ({
      isOrchestrating: false,
      activeTaskRun: state.activeTaskRun
        ? {
            ...state.activeTaskRun,
            status: 'completed',
            result_summary: payload?.summary || null,
            total_tokens_in: payload?.totalTokensIn || 0,
            total_tokens_out: payload?.totalTokensOut || 0,
            total_duration_ms: payload?.totalDurationMs || 0,
          }
        : state.activeTaskRun,
    }));
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:error
  tauriListen<any>('orchestration:error', (payload) => {
    console.error('[Orchestration] Error:', payload);
    useOrchestrationStore.setState((state) => ({
      isOrchestrating: false,
      activeTaskRun: state.activeTaskRun
        ? { ...state.activeTaskRun, status: 'failed' }
        : state.activeTaskRun,
    }));
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  console.log('[Orchestration] Event listeners initialized');
}

export function cleanupOrchestrationListeners() {
  orchestrationUnlistenFns.forEach((unlisten) => unlisten());
  orchestrationUnlistenFns = [];
}
