import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import type {
  TaskRun,
  TaskAssignment,
  TaskPlan,
  AgentTrackingInfo,
  OrchPermissionRequest,
  OrchToolCall,
  ScheduleTaskRequest,
  RecurrencePattern,
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
  /** Task run being viewed from Kanban (read-only historical view) */
  viewingTaskRun: TaskRun | null;
  /** Assignments for the viewed task run */
  viewingAssignments: TaskAssignment[];
  /** Agent tracking info for the viewed task run */
  viewingAgentTracking: Record<string, AgentTrackingInfo>;
  /** Task plan for the viewed task run */
  viewingTaskPlan: TaskPlan | null;
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
  rateTaskRun: (taskRunId: string, rating: number) => Promise<void>;
  setExpandedAgentId: (agentId: string | null) => void;
  /** Load a historical task run for viewing from Kanban */
  viewTaskRun: (taskRun: TaskRun) => Promise<void>;
  /** Clear the viewed task run */
  clearViewingTaskRun: () => void;
  // Scheduling actions
  scheduleTask: (request: ScheduleTaskRequest) => Promise<TaskRun>;
  pauseScheduledTask: (taskRunId: string) => Promise<void>;
  resumeScheduledTask: (taskRunId: string) => Promise<void>;
  clearSchedule: (taskRunId: string) => Promise<void>;
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
    viewingTaskRun: null,
    viewingAssignments: [],
    viewingAgentTracking: {},
    viewingTaskPlan: null,

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

    rateTaskRun: async (taskRunId: string, rating: number) => {
      try {
        await tauriInvoke('rate_task_run', { taskRunId, rating });
        set((state) => ({
          activeTaskRun: state.activeTaskRun
            ? { ...state.activeTaskRun, rating }
            : null,
          // Also update viewingTaskRun if it's the same task
          viewingTaskRun: state.viewingTaskRun?.id === taskRunId
            ? { ...state.viewingTaskRun, rating }
            : state.viewingTaskRun,
        }));
        // Also update taskRuns list
        useOrchestrationStore.setState((state) => ({
          taskRuns: state.taskRuns.map((tr) =>
            tr.id === taskRunId ? { ...tr, rating } : tr
          ),
        }));
      } catch (error) {
        console.error('[Orchestration] Failed to rate task run:', error);
        throw error;
      }
    },

    viewTaskRun: async (taskRun: TaskRun) => {
      console.log('[Orchestration] Viewing task run:', taskRun.id);
      try {
        // Fetch assignments for this task run
        const assignments = await tauriInvoke<TaskAssignment[]>('get_task_assignments', {
          taskRunId: taskRun.id,
        });

        // Parse task plan
        let taskPlan: TaskPlan | null = null;
        if (taskRun.task_plan_json) {
          try {
            taskPlan = JSON.parse(taskRun.task_plan_json);
          } catch {
            console.warn('[Orchestration] Failed to parse task plan JSON');
          }
        }

        // Build agent tracking info from assignments
        const agentTracking: Record<string, AgentTrackingInfo> = {};
        for (const assignment of assignments) {
          agentTracking[assignment.agent_id] = {
            agentId: assignment.agent_id,
            agentName: assignment.agent_name,
            model: assignment.model_used || '',
            status: assignment.status as AgentTrackingInfo['status'],
            tokensIn: assignment.tokens_in,
            tokensOut: assignment.tokens_out,
            cacheCreationTokens: assignment.cache_creation_tokens,
            cacheReadTokens: assignment.cache_read_tokens,
            durationMs: assignment.duration_ms,
            streamedContent: '',
            output: assignment.output_text || undefined,
            assignmentId: assignment.id,
            toolCalls: [],
          };
        }

        set({
          viewingTaskRun: taskRun,
          viewingAssignments: assignments,
          viewingAgentTracking: agentTracking,
          viewingTaskPlan: taskPlan,
        });
      } catch (error) {
        console.error('[Orchestration] Failed to view task run:', error);
      }
    },

    clearViewingTaskRun: () => {
      set({
        viewingTaskRun: null,
        viewingAssignments: [],
        viewingAgentTracking: {},
        viewingTaskPlan: null,
      });
    },

    // Scheduling actions
    scheduleTask: async (request: ScheduleTaskRequest) => {
      const taskRun = await tauriInvoke<TaskRun>('schedule_task', { request });
      // Update task in the taskRuns list
      set((state) => ({
        taskRuns: state.taskRuns.map((tr) =>
          tr.id === taskRun.id ? taskRun : tr
        ),
        // Also update viewingTaskRun if it's the same task
        viewingTaskRun: state.viewingTaskRun?.id === taskRun.id
          ? taskRun
          : state.viewingTaskRun,
      }));
      return taskRun;
    },

    pauseScheduledTask: async (taskRunId: string) => {
      await tauriInvoke('pause_scheduled_task', { taskRunId });
      set((state) => ({
        taskRuns: state.taskRuns.map((tr) =>
          tr.id === taskRunId ? { ...tr, is_paused: true } : tr
        ),
        viewingTaskRun: state.viewingTaskRun?.id === taskRunId
          ? { ...state.viewingTaskRun, is_paused: true }
          : state.viewingTaskRun,
      }));
    },

    resumeScheduledTask: async (taskRunId: string) => {
      await tauriInvoke('resume_scheduled_task', { taskRunId });
      set((state) => ({
        taskRuns: state.taskRuns.map((tr) =>
          tr.id === taskRunId ? { ...tr, is_paused: false } : tr
        ),
        viewingTaskRun: state.viewingTaskRun?.id === taskRunId
          ? { ...state.viewingTaskRun, is_paused: false }
          : state.viewingTaskRun,
      }));
    },

    clearSchedule: async (taskRunId: string) => {
      await tauriInvoke('clear_schedule', { taskRunId });
      set((state) => ({
        taskRuns: state.taskRuns.map((tr) =>
          tr.id === taskRunId
            ? { ...tr, schedule_type: 'none', scheduled_time: null, recurrence_pattern_json: null, next_run_at: null, is_paused: false }
            : tr
        ),
        viewingTaskRun: state.viewingTaskRun?.id === taskRunId
          ? { ...state.viewingTaskRun, schedule_type: 'none', scheduled_time: null, recurrence_pattern_json: null, next_run_at: null, is_paused: false }
          : state.viewingTaskRun,
      }));
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
            cacheCreationTokens: 0,
            cacheReadTokens: 0,
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
                  durationMs: payload.durationMs ?? 0,
                  tokensIn: payload.tokensIn ?? existing.tokensIn ?? 0,
                  tokensOut: payload.tokensOut ?? existing.tokensOut ?? 0,
                  cacheCreationTokens: payload.cacheCreationTokens ?? existing.cacheCreationTokens ?? 0,
                  cacheReadTokens: payload.cacheReadTokens ?? existing.cacheReadTokens ?? 0,
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
            total_tokens_in: payload?.totalTokensIn ?? 0,
            total_tokens_out: payload?.totalTokensOut ?? 0,
            total_cache_creation_tokens: payload?.totalCacheCreationTokens ?? 0,
            total_cache_read_tokens: payload?.totalCacheReadTokens ?? 0,
            total_duration_ms: payload?.totalDurationMs ?? 0,
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
