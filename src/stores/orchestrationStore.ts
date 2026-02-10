import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  TaskRun,
  TaskAssignment,
  TaskPlan,
  AgentTrackingInfo,
  OrchPermissionRequest,
  OrchToolCall,
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
  pendingOrchPermission: OrchPermissionRequest | null;
  isAwaitingConfirmation: boolean;
  expandedAgentId: string | null;
}

interface OrchestrationActions {
  startOrchestration: (prompt: string) => Promise<void>;
  cancelOrchestration: () => Promise<void>;
  cancelAgent: (taskRunId: string, agentId: string) => Promise<void>;
  continueOrchestration: (supplementaryPrompt: string) => Promise<void>;
  dismissTaskRun: () => void;
  fetchTaskRuns: () => Promise<void>;
  fetchAssignments: (taskRunId: string) => Promise<void>;
  confirmResults: (taskRunId: string) => Promise<void>;
  regenerateAgent: (taskRunId: string, agentId: string) => Promise<void>;
  regenerateAll: (taskRunId: string) => Promise<void>;
  respondToOrchPermission: (
    taskRunId: string,
    agentId: string,
    requestId: string,
    optionId: string
  ) => Promise<void>;
  setExpandedAgentId: (agentId: string | null) => void;
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
    pendingOrchPermission: null,
    isAwaitingConfirmation: false,
    expandedAgentId: null,

    startOrchestration: async (prompt: string) => {
      set({
        isOrchestrating: true,
        activeTaskRun: null,
        assignments: [],
        agentTracking: {},
        streamingAgentId: null,
        streamedContent: '',
        taskPlan: null,
        pendingOrchPermission: null,
        isAwaitingConfirmation: false,
        expandedAgentId: null,
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
      set((state) => ({
        isOrchestrating: false,
        isAwaitingConfirmation: false,
        activeTaskRun: state.activeTaskRun
          ? { ...state.activeTaskRun, status: 'cancelled' }
          : null,
      }));
    },

    cancelAgent: async (taskRunId: string, agentId: string) => {
      try {
        await tauriInvoke('cancel_agent', { taskRunId, agentId });
      } catch (error) {
        console.error('[Orchestration] Failed to cancel agent:', error);
      }
    },

    continueOrchestration: async (supplementaryPrompt: string) => {
      const { activeTaskRun, agentTracking, taskPlan } = get();
      if (!activeTaskRun) return;

      // Build context prompt from previous run
      const parts: string[] = [];
      parts.push(`Previous task: ${activeTaskRun.user_prompt}`);
      if (activeTaskRun.result_summary) {
        parts.push(`Previous summary: ${activeTaskRun.result_summary}`);
      }

      const trackingEntries = Object.values(agentTracking);
      if (trackingEntries.length > 0) {
        parts.push('Previous agent outputs:');
        for (const info of trackingEntries) {
          parts.push(`[${info.agentName}] (${info.status}): ${info.output || info.streamedContent || '(no output)'}`);
        }
      }

      parts.push(`Additional instructions: ${supplementaryPrompt}`);

      const contextPrompt = parts.join('\n');

      // Reset state and start a new orchestration with the context prompt
      set({
        isOrchestrating: true,
        activeTaskRun: null,
        assignments: [],
        agentTracking: {},
        streamingAgentId: null,
        streamedContent: '',
        taskPlan: null,
        pendingOrchPermission: null,
        isAwaitingConfirmation: false,
        expandedAgentId: null,
      });

      try {
        const taskRun = await tauriInvoke<TaskRun>('start_orchestration', {
          request: { user_prompt: contextPrompt, title: '' },
        });
        set({ activeTaskRun: taskRun });
      } catch (error) {
        console.error('[Orchestration] Failed to continue:', error);
        set({ isOrchestrating: false });
        throw error;
      }
    },

    dismissTaskRun: () => {
      set({
        activeTaskRun: null,
        assignments: [],
        agentTracking: {},
        isOrchestrating: false,
        streamingAgentId: null,
        streamedContent: '',
        taskPlan: null,
        pendingOrchPermission: null,
        isAwaitingConfirmation: false,
        expandedAgentId: null,
      });
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

    confirmResults: async (taskRunId: string) => {
      try {
        await tauriInvoke('confirm_orchestration', { taskRunId });
        set({ isAwaitingConfirmation: false });
      } catch (error) {
        console.error('[Orchestration] Failed to confirm:', error);
      }
    },

    regenerateAgent: async (taskRunId: string, agentId: string) => {
      try {
        set({ isAwaitingConfirmation: false });
        await tauriInvoke('regenerate_agent', { taskRunId, agentId });
      } catch (error) {
        console.error('[Orchestration] Failed to regenerate agent:', error);
      }
    },

    regenerateAll: async (taskRunId: string) => {
      try {
        set({ isAwaitingConfirmation: false });
        await tauriInvoke('regenerate_agent', { taskRunId, agentId: '__all__' });
      } catch (error) {
        console.error('[Orchestration] Failed to regenerate all:', error);
      }
    },

    respondToOrchPermission: async (
      taskRunId: string,
      agentId: string,
      requestId: string,
      optionId: string
    ) => {
      try {
        await tauriInvoke('respond_orch_permission', {
          taskRunId,
          agentId,
          requestId,
          optionId,
        });
        set({ pendingOrchPermission: null });
      } catch (error) {
        console.error('[Orchestration] Failed to respond to permission:', error);
      }
    },

    setExpandedAgentId: (agentId: string | null) => {
      set({ expandedAgentId: agentId });
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
        pendingOrchPermission: null,
        isAwaitingConfirmation: false,
        expandedAgentId: null,
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
            acpSessionId: payload.acpSessionId || undefined,
            assignmentId: payload.assignmentId || undefined,
            toolCalls: [],
            output: undefined,
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

  // orchestration:agent_tool_call
  tauriListen<any>('orchestration:agent_tool_call', (payload) => {
    if (payload?.agentId) {
      useOrchestrationStore.setState((state) => {
        const existing = state.agentTracking[payload.agentId];
        if (!existing) return {};

        const toolCall: OrchToolCall = {
          toolCallId: payload.toolCallId || '',
          name: payload.name || '',
          title: payload.title || undefined,
          status: payload.status || 'running',
          rawInput: payload.rawInput || undefined,
          rawOutput: payload.rawOutput || undefined,
        };

        const existingCalls = existing.toolCalls || [];
        const idx = existingCalls.findIndex((tc) => tc.toolCallId === toolCall.toolCallId);
        let updatedCalls: OrchToolCall[];
        if (idx >= 0) {
          updatedCalls = [...existingCalls];
          updatedCalls[idx] = { ...updatedCalls[idx], ...toolCall };
        } else {
          updatedCalls = [...existingCalls, toolCall];
        }

        return {
          agentTracking: {
            ...state.agentTracking,
            [payload.agentId]: {
              ...existing,
              toolCalls: updatedCalls,
            },
          },
        };
      });
    }
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_thought
  tauriListen<any>('orchestration:agent_thought', (payload) => {
    if (payload?.text && payload?.agentId) {
      useOrchestrationStore.setState((state) => {
        const existing = state.agentTracking[payload.agentId];
        if (!existing) return {};
        return {
          agentTracking: {
            ...state.agentTracking,
            [payload.agentId]: {
              ...existing,
              streamedContent: existing.streamedContent + payload.text,
            },
          },
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
                  tokensIn: payload.tokensIn || existing.tokensIn || 0,
                  tokensOut: payload.tokensOut || existing.tokensOut || 0,
                  acpSessionId: payload.acpSessionId || existing.acpSessionId,
                  output: payload.output || existing.streamedContent || undefined,
                  assignmentId: payload.assignmentId || existing.assignmentId,
                },
              }
            : state.agentTracking,
        };
      });
    }
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:awaiting_confirmation
  tauriListen<any>('orchestration:awaiting_confirmation', (payload) => {
    console.log('[Orchestration] Awaiting confirmation:', payload);
    useOrchestrationStore.setState((state) => ({
      isAwaitingConfirmation: true,
      activeTaskRun: state.activeTaskRun
        ? { ...state.activeTaskRun, status: 'awaiting_confirmation' as const }
        : state.activeTaskRun,
    }));
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:orch_permission
  tauriListen<any>('orchestration:orch_permission', (payload) => {
    console.log('[Orchestration] Permission request:', payload);
    if (payload) {
      useOrchestrationStore.setState({
        pendingOrchPermission: {
          taskRunId: payload.taskRunId || '',
          agentId: payload.agentId || '',
          requestId: payload.requestId || '',
          sessionId: payload.sessionId || '',
          toolCall: payload.toolCall || undefined,
          options: payload.options || [],
        },
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
      isAwaitingConfirmation: false,
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
    const errorMsg =
      payload?.error ||
      (typeof payload === 'string' ? payload : JSON.stringify(payload));
    console.error('[Orchestration] Error:', errorMsg, 'Raw payload:', JSON.stringify(payload));
    useOrchestrationStore.setState((state) => ({
      isOrchestrating: false,
      isAwaitingConfirmation: false,
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
