import { create } from 'zustand';
import { tauriInvoke, tauriListen, isTauri } from '@/lib/tauri';
import { useWorkspaceStore } from './workspaceStore';
import { showError } from './toastStore';
import type {
  TaskRun,
  TaskAssignment,
  TaskPlan,
  AgentTrackingInfo,
  OrchPermissionRequest,
  OrchToolCall,
  ScheduleTaskRequest,
  PlanValidation,
  TaskRunState,
} from '@/types/orchestration';
import type { SkillDiscoveryResult } from '@/types/agent';

// ---------------------------------------------------------------------------
// State shape
// ---------------------------------------------------------------------------

interface OrchestrationState {
  /** Per-task-run state keyed by taskRunId */
  taskRunStates: Record<string, TaskRunState>;
  /** Which task the UI is currently showing */
  focusedTaskRunId: string | null;
  /** True while any task is actively running */
  isOrchestrating: boolean;
  /** Historical list of all task runs */
  taskRuns: TaskRun[];
  /** Permission requests awaiting user response */
  pendingOrchPermissions: OrchPermissionRequest[];
  /** Task run being viewed from Kanban (read-only historical view) */
  viewingTaskRun: TaskRun | null;
  viewingAssignments: TaskAssignment[];
  viewingAgentTracking: Record<string, AgentTrackingInfo>;
  viewingTaskPlan: TaskPlan | null;
  /** Discovered skills from workspace scanning */
  discoveredSkills: SkillDiscoveryResult | null;
  /** Task run IDs restored on app restart that need user attention */
  restoredTaskRunIds: string[];
}

interface OrchestrationActions {
  startOrchestration: (prompt: string) => Promise<void>;
  cancelOrchestration: (taskRunId?: string) => Promise<void>;
  cancelAgent: (taskRunId: string, agentId: string) => Promise<void>;
  continueOrchestration: (supplementaryPrompt: string) => Promise<void>;
  dismissTaskRun: (taskRunId?: string) => void;
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
  setFocusedTaskRunId: (id: string | null) => void;
  viewTaskRun: (taskRun: TaskRun) => Promise<void>;
  clearViewingTaskRun: () => void;
  scheduleTask: (request: ScheduleTaskRequest) => Promise<TaskRun>;
  pauseScheduledTask: (taskRunId: string) => Promise<void>;
  resumeScheduledTask: (taskRunId: string) => Promise<void>;
  clearSchedule: (taskRunId: string) => Promise<void>;
  discoverWorkspaceSkills: (forceRefresh?: boolean) => Promise<SkillDiscoveryResult | null>;
  restoreIncompleteTaskRun: () => Promise<void>;
  clearRestoredTaskRunId: (taskRunId: string) => void;
  resumeWithEditedContext: (taskRunId: string, editedContext: string) => Promise<void>;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Helper: immutably update a single TaskRunState entry
// ---------------------------------------------------------------------------

function updateTaskRunState(
  state: OrchestrationState,
  taskRunId: string,
  updater: (current: TaskRunState) => Partial<TaskRunState>
): Partial<OrchestrationState> {
  const existing = state.taskRunStates[taskRunId];
  if (!existing) return {};
  return {
    taskRunStates: {
      ...state.taskRunStates,
      [taskRunId]: { ...existing, ...updater(existing) },
    },
  };
}

/**
 * Like updateTaskRunState, but auto-creates a placeholder TaskRunState if the
 * entry doesn't exist yet. This handles the race condition where Tauri events
 * arrive before the startOrchestration invoke resolves.
 */
function upsertTaskRunState(
  state: OrchestrationState,
  taskRunId: string,
  updater: (current: TaskRunState) => Partial<TaskRunState>
): Partial<OrchestrationState> {
  const existing = state.taskRunStates[taskRunId] ?? createPlaceholderTaskRunState(taskRunId);
  return {
    taskRunStates: {
      ...state.taskRunStates,
      [taskRunId]: { ...existing, ...updater(existing) },
    },
  };
}

/** Create a minimal placeholder TaskRunState for events that arrive before startOrchestration resolves */
function createPlaceholderTaskRunState(taskRunId: string): TaskRunState {
  return {
    taskRun: {
      id: taskRunId,
      title: '',
      user_prompt: '',
      control_hub_agent_id: '',
      status: 'pending',
      task_plan_json: null,
      result_summary: null,
      total_tokens_in: 0,
      total_tokens_out: 0,
      total_cache_creation_tokens: 0,
      total_cache_read_tokens: 0,
      total_duration_ms: 0,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      rating: null,
      schedule_type: 'none',
      scheduled_time: null,
      recurrence_pattern_json: null,
      next_run_at: null,
      is_paused: false,
      workspace_id: null,
    },
    assignments: [],
    agentTracking: {},
    streamingAgentId: null,
    streamedContent: '',
    taskPlan: null,
    planValidation: null,
    isAwaitingConfirmation: false,
    expandedAgentId: null,
  };
}

/** Create a fresh TaskRunState for a new task */
function createTaskRunState(taskRun: TaskRun): TaskRunState {
  return {
    taskRun,
    assignments: [],
    agentTracking: {},
    streamingAgentId: null,
    streamedContent: '',
    taskPlan: null,
    planValidation: null,
    isAwaitingConfirmation: false,
    expandedAgentId: null,
  };
}

/** Recalculate isOrchestrating from taskRunStates */
function computeIsOrchestrating(states: Record<string, TaskRunState>): boolean {
  return Object.values(states).some(
    (trs) => ['pending', 'analyzing', 'running'].includes(trs.taskRun.status)
  );
}

// ---------------------------------------------------------------------------
// Exported helper: build task context string from a task run
// ---------------------------------------------------------------------------

export function buildTaskContext(
  taskRun: TaskRun,
  agentTracking: Record<string, AgentTrackingInfo>
): string {
  const parts: string[] = [];
  parts.push('## 上次任务\n');
  parts.push(taskRun.user_prompt);

  if (taskRun.result_summary) {
    parts.push('\n\n---\n\n## 执行摘要\n');
    parts.push(taskRun.result_summary);
  }

  const trackingEntries = Object.values(agentTracking);
  if (trackingEntries.length > 0) {
    parts.push('\n\n---\n\n## Agent 输出\n');
    for (const info of trackingEntries) {
      parts.push(`\n### ${info.agentName} (${info.status})\n`);
      parts.push(info.output || info.streamedContent || '(no output)');
    }
  }

  return parts.join('');
}

// ---------------------------------------------------------------------------
// Selector hooks for backward compatibility
// ---------------------------------------------------------------------------

export function useFocusedTaskRunState(): TaskRunState | null {
  return useOrchestrationStore((s) => {
    if (!s.focusedTaskRunId) return null;
    return s.taskRunStates[s.focusedTaskRunId] ?? null;
  });
}

export function useActiveTaskRun(): TaskRun | null {
  return useOrchestrationStore((s) => {
    if (!s.focusedTaskRunId) return null;
    return s.taskRunStates[s.focusedTaskRunId]?.taskRun ?? null;
  });
}

export function useFocusedAgentTracking(): Record<string, AgentTrackingInfo> {
  return useOrchestrationStore((s) => {
    if (!s.focusedTaskRunId) return {};
    return s.taskRunStates[s.focusedTaskRunId]?.agentTracking ?? {};
  });
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useOrchestrationStore = create<OrchestrationState & OrchestrationActions>(
  (set, get) => ({
    taskRunStates: {},
    focusedTaskRunId: null,
    isOrchestrating: false,
    taskRuns: [],
    pendingOrchPermissions: [],
    viewingTaskRun: null,
    viewingAssignments: [],
    viewingAgentTracking: {},
    viewingTaskPlan: null,
    discoveredSkills: null,
    restoredTaskRunIds: [],

    startOrchestration: async (prompt: string) => {
      set({ discoveredSkills: null });

      try {
        const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
        const taskRun = await tauriInvoke<TaskRun>('start_orchestration', {
          request: {
            user_prompt: prompt,
            title: '',
            workspace_id: workspaceId,
          },
        });
        set((state) => {
          // Merge with any existing placeholder state that was created by
          // events arriving before this invoke resolved (race condition fix)
          const existing = state.taskRunStates[taskRun.id];
          const fresh = createTaskRunState(taskRun);
          const merged = existing
            ? {
                ...fresh,
                // Preserve event-populated fields from placeholder
                agentTracking: Object.keys(existing.agentTracking).length > 0
                  ? existing.agentTracking
                  : fresh.agentTracking,
                streamingAgentId: existing.streamingAgentId ?? fresh.streamingAgentId,
                streamedContent: existing.streamedContent || fresh.streamedContent,
                taskPlan: existing.taskPlan ?? fresh.taskPlan,
                planValidation: existing.planValidation ?? fresh.planValidation,
                expandedAgentId: existing.expandedAgentId ?? fresh.expandedAgentId,
                isAwaitingConfirmation: existing.isAwaitingConfirmation,
                // Always use the real TaskRun from the invoke (has full data)
                taskRun: {
                  ...taskRun,
                  status: existing.taskRun.status !== 'pending'
                    ? existing.taskRun.status
                    : taskRun.status,
                },
              }
            : fresh;
          const newStates = {
            ...state.taskRunStates,
            [taskRun.id]: merged,
          };
          return {
            taskRunStates: newStates,
            focusedTaskRunId: taskRun.id,
            isOrchestrating: true,
          };
        });
      } catch (error) {
        console.error('[Orchestration] Failed to start:', error);
        showError('启动编排失败', error);
        throw error;
      }
    },

    cancelOrchestration: async (taskRunId?: string) => {
      const { focusedTaskRunId, taskRunStates } = get();
      const targetId = taskRunId ?? focusedTaskRunId;
      if (!targetId) return;

      const trs = taskRunStates[targetId];
      if (!trs) return;

      try {
        await tauriInvoke('cancel_orchestration', { taskRunId: targetId });
      } catch (error) {
        console.error('[Orchestration] Failed to cancel:', error);
        showError('取消编排失败', error);
      }

      set((state) => {
        const updated = updateTaskRunState(state, targetId, (cur) => ({
          taskRun: { ...cur.taskRun, status: 'cancelled' },
          isAwaitingConfirmation: false,
        }));
        const newStates = updated.taskRunStates ?? state.taskRunStates;
        return {
          ...updated,
          isOrchestrating: computeIsOrchestrating(newStates),
        };
      });
    },

    cancelAgent: async (taskRunId: string, agentId: string) => {
      try {
        await tauriInvoke('cancel_agent', { taskRunId, agentId });
      } catch (error) {
        console.error('[Orchestration] Failed to cancel agent:', error);
        showError('取消 Agent 失败', error);
      }
    },

    continueOrchestration: async (supplementaryPrompt: string) => {
      const { focusedTaskRunId, taskRunStates } = get();
      if (!focusedTaskRunId) return;
      const trs = taskRunStates[focusedTaskRunId];
      if (!trs) return;

      const { taskRun, agentTracking } = trs;

      // Build context prompt from previous run
      const contextPrompt = buildTaskContext(taskRun, agentTracking)
        + '\n\n---\n\n## 补充指令\n'
        + supplementaryPrompt;

      // Cancel the old task run on the backend
      try {
        await tauriInvoke('cancel_orchestration', { taskRunId: taskRun.id });
      } catch {
        // Ignore — task may already be finished or cleaned up
      }

      // Remove old task run state
      set((state) => {
        const { [focusedTaskRunId]: _removed, ...rest } = state.taskRunStates;
        return {
          taskRunStates: rest,
          focusedTaskRunId: null,
        };
      });

      // Start new orchestration with context prompt
      try {
        const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
        const newTaskRun = await tauriInvoke<TaskRun>('start_orchestration', {
          request: {
            user_prompt: contextPrompt,
            title: '',
            workspace_id: workspaceId,
          },
        });
        set((state) => {
          const newStates = {
            ...state.taskRunStates,
            [newTaskRun.id]: createTaskRunState(newTaskRun),
          };
          return {
            taskRunStates: newStates,
            focusedTaskRunId: newTaskRun.id,
            isOrchestrating: true,
            discoveredSkills: null,
          };
        });
      } catch (error) {
        console.error('[Orchestration] Failed to continue:', error);
        showError('继续编排失败', error);
        set((state) => ({
          isOrchestrating: computeIsOrchestrating(state.taskRunStates),
        }));
        throw error;
      }
    },

    dismissTaskRun: (taskRunId?: string) => {
      const { focusedTaskRunId } = get();
      const targetId = taskRunId ?? focusedTaskRunId;
      if (!targetId) return;

      set((state) => {
        const { [targetId]: _removed, ...rest } = state.taskRunStates;
        const remainingIds = Object.keys(rest);
        // Find next running task, or first remaining, or null
        const nextFocus =
          remainingIds.find((id) =>
            ['pending', 'analyzing', 'running'].includes(rest[id].taskRun.status)
          ) ?? remainingIds[0] ?? null;
        return {
          taskRunStates: rest,
          focusedTaskRunId: nextFocus,
          isOrchestrating: computeIsOrchestrating(rest),
          discoveredSkills: null,
        };
      });
    },

    fetchTaskRuns: async () => {
      try {
        const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
        if (!workspaceId) return; // No workspace selected — skip fetching
        const runs = await tauriInvoke<TaskRun[]>('list_task_runs', { workspaceId });
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
        set((state) => updateTaskRunState(state, taskRunId, () => ({ assignments })));
      } catch (error) {
        console.error('[Orchestration] Failed to fetch assignments:', error);
      }
    },

    confirmResults: async (taskRunId: string) => {
      try {
        await tauriInvoke('confirm_orchestration', { taskRunId });
        set((state) => updateTaskRunState(state, taskRunId, () => ({ isAwaitingConfirmation: false })));
      } catch (error) {
        console.warn('[Orchestration] confirm failed (stale?), dismissing:', error);
        set((state) => {
          const trs = state.taskRunStates[taskRunId];
          if (!trs) return {};
          const updated = updateTaskRunState(state, taskRunId, (cur) => ({
            isAwaitingConfirmation: false,
            taskRun: { ...cur.taskRun, status: 'completed' },
          }));
          const newStates = updated.taskRunStates ?? state.taskRunStates;
          return { ...updated, isOrchestrating: computeIsOrchestrating(newStates) };
        });
      }
    },

    regenerateAgent: async (taskRunId: string, agentId: string) => {
      try {
        set((state) => updateTaskRunState(state, taskRunId, () => ({ isAwaitingConfirmation: false })));
        await tauriInvoke('regenerate_agent', { taskRunId, agentId });
      } catch (error) {
        console.warn('[Orchestration] regenerate failed (stale?), using continueOrchestration:', error);
        const trs = get().taskRunStates[taskRunId];
        const agentInfo = trs?.agentTracking[agentId];
        const instruction = agentInfo
          ? `请重新运行 ${agentInfo.agentName} 的任务`
          : '请重新运行该任务';
        // Focus this task before continuing
        set({ focusedTaskRunId: taskRunId });
        await get().continueOrchestration(instruction);
      }
    },

    regenerateAll: async (taskRunId: string) => {
      try {
        set((state) => updateTaskRunState(state, taskRunId, () => ({ isAwaitingConfirmation: false })));
        await tauriInvoke('regenerate_agent', { taskRunId, agentId: '__all__' });
      } catch (error) {
        console.warn('[Orchestration] regenerateAll failed (stale?), using continueOrchestration:', error);
        set({ focusedTaskRunId: taskRunId });
        await get().continueOrchestration('请重新运行所有 Agent 的任务');
      }
    },

    respondToOrchPermission: async (
      taskRunId: string,
      agentId: string,
      requestId: string,
      optionId: string
    ) => {
      try {
        await tauriInvoke('respond_orch_permission', { taskRunId, agentId, requestId, optionId });
        set((state) => ({
          pendingOrchPermissions: state.pendingOrchPermissions.filter(
            (p) => !(p.taskRunId === taskRunId && p.requestId === requestId)
          ),
        }));
      } catch (error) {
        console.error('[Orchestration] Failed to respond to permission:', error);
        showError('权限响应失败', error);
      }
    },

    setExpandedAgentId: (agentId: string | null) => {
      const { focusedTaskRunId } = get();
      if (!focusedTaskRunId) return;
      set((state) => updateTaskRunState(state, focusedTaskRunId, () => ({ expandedAgentId: agentId })));
    },

    setFocusedTaskRunId: (id: string | null) => {
      set({ focusedTaskRunId: id });
    },

    rateTaskRun: async (taskRunId: string, rating: number) => {
      try {
        await tauriInvoke('rate_task_run', { taskRunId, rating });
        set((state) => {
          const trs = state.taskRunStates[taskRunId];
          const taskRunStatesUpdate = trs
            ? {
                taskRunStates: {
                  ...state.taskRunStates,
                  [taskRunId]: { ...trs, taskRun: { ...trs.taskRun, rating } },
                },
              }
            : {};
          return {
            ...taskRunStatesUpdate,
            viewingTaskRun: state.viewingTaskRun?.id === taskRunId
              ? { ...state.viewingTaskRun, rating }
              : state.viewingTaskRun,
          };
        });
        // Also update taskRuns list
        useOrchestrationStore.setState((state) => ({
          taskRuns: state.taskRuns.map((tr) =>
            tr.id === taskRunId ? { ...tr, rating } : tr
          ),
        }));
      } catch (error) {
        console.error('[Orchestration] Failed to rate task run:', error);
        showError('评分失败', error);
        throw error;
      }
    },

    viewTaskRun: async (taskRun: TaskRun) => {
      console.log('[Orchestration] Viewing task run:', taskRun.id);
      try {
        const assignments = await tauriInvoke<TaskAssignment[]>('get_task_assignments', {
          taskRunId: taskRun.id,
        });

        let taskPlan: TaskPlan | null = null;
        if (taskRun.task_plan_json) {
          try {
            taskPlan = JSON.parse(taskRun.task_plan_json);
          } catch {
            console.warn('[Orchestration] Failed to parse task plan JSON');
          }
        }

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
        showError('查看任务失败', error);
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

    scheduleTask: async (request: ScheduleTaskRequest) => {
      const taskRun = await tauriInvoke<TaskRun>('schedule_task', { request });
      set((state) => ({
        taskRuns: state.taskRuns.map((tr) =>
          tr.id === taskRun.id ? taskRun : tr
        ),
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
            ? { ...tr, schedule_type: 'none' as const, scheduled_time: null, recurrence_pattern_json: null, next_run_at: null, is_paused: false }
            : tr
        ),
        viewingTaskRun: state.viewingTaskRun?.id === taskRunId
          ? { ...state.viewingTaskRun, schedule_type: 'none' as const, scheduled_time: null, recurrence_pattern_json: null, next_run_at: null, is_paused: false }
          : state.viewingTaskRun,
      }));
    },

    discoverWorkspaceSkills: async (forceRefresh?: boolean) => {
      try {
        const result = await tauriInvoke<SkillDiscoveryResult>('discover_workspace_skills', {
          forceRefresh: forceRefresh ?? false,
        });
        set({ discoveredSkills: result });
        return result;
      } catch (error) {
        console.error('[Orchestration] Failed to discover workspace skills:', error);
        return null;
      }
    },

    restoreIncompleteTaskRun: async () => {
      try {
        const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
        if (!workspaceId) return; // No workspace selected — skip restoring
        const runs = await tauriInvoke<TaskRun[]>('list_task_runs', { workspaceId });
        set({ taskRuns: runs });

        // Find ALL non-completed task runs that the user might want to continue
        const incompleteStatuses = ['pending', 'running', 'analyzing', 'awaiting_confirmation', 'failed'];
        const incompleteRuns = runs.filter((r) => incompleteStatuses.includes(r.status));

        if (incompleteRuns.length === 0) return;

        const newTaskRunStates: Record<string, TaskRunState> = {};
        let firstId: string | null = null;

        for (const incomplete of incompleteRuns) {
          console.log('[Orchestration] Restoring incomplete task run:', incomplete.id, incomplete.status);

          // Tasks with running/analyzing/pending status cannot be resumed after app restart
          if (['running', 'analyzing', 'pending'].includes(incomplete.status)) {
            incomplete.status = 'failed';
            try {
              await tauriInvoke('update_task_run_status', {
                taskRunId: incomplete.id,
                status: 'failed',
              });
            } catch {
              // Best-effort
            }
          }

          // Fetch assignments for this task run
          const assignments = await tauriInvoke<TaskAssignment[]>('get_task_assignments', {
            taskRunId: incomplete.id,
          });

          // Parse task plan
          let taskPlan: TaskPlan | null = null;
          if (incomplete.task_plan_json) {
            try {
              taskPlan = JSON.parse(incomplete.task_plan_json);
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

          newTaskRunStates[incomplete.id] = {
            taskRun: incomplete,
            assignments,
            agentTracking,
            taskPlan,
            planValidation: null,
            streamingAgentId: null,
            streamedContent: '',
            isAwaitingConfirmation: incomplete.status === 'awaiting_confirmation',
            expandedAgentId: null,
          };

          if (!firstId) firstId = incomplete.id;
        }

        set((state) => ({
          taskRunStates: { ...state.taskRunStates, ...newTaskRunStates },
          focusedTaskRunId: firstId,
          isOrchestrating: false,
          restoredTaskRunIds: Object.keys(newTaskRunStates),
        }));
      } catch (error) {
        console.error('[Orchestration] Failed to restore incomplete task run:', error);
      }
    },

    clearRestoredTaskRunId: (taskRunId: string) => {
      set((state) => ({
        restoredTaskRunIds: state.restoredTaskRunIds.filter((id) => id !== taskRunId),
      }));
    },

    resumeWithEditedContext: async (taskRunId: string, editedContext: string) => {
      const { taskRunStates } = get();
      const trs = taskRunStates[taskRunId];
      if (!trs) return;

      // Cancel the old task run on the backend (best-effort)
      try {
        await tauriInvoke('cancel_orchestration', { taskRunId });
      } catch {
        // Ignore — task may already be finished or cleaned up
      }

      // Remove old task run state and clear from restored list
      set((state) => {
        const { [taskRunId]: _removed, ...rest } = state.taskRunStates;
        return {
          taskRunStates: rest,
          focusedTaskRunId: null,
          restoredTaskRunIds: state.restoredTaskRunIds.filter((id) => id !== taskRunId),
        };
      });

      // Start new orchestration with edited context
      try {
        const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
        const newTaskRun = await tauriInvoke<TaskRun>('start_orchestration', {
          request: {
            user_prompt: editedContext,
            title: '',
            workspace_id: workspaceId,
          },
        });
        set((state) => {
          const newStates = {
            ...state.taskRunStates,
            [newTaskRun.id]: createTaskRunState(newTaskRun),
          };
          return {
            taskRunStates: newStates,
            focusedTaskRunId: newTaskRun.id,
            isOrchestrating: true,
            discoveredSkills: null,
          };
        });
      } catch (error) {
        console.error('[Orchestration] Failed to resume with edited context:', error);
        showError('恢复编排失败', error);
        set((state) => ({
          isOrchestrating: computeIsOrchestrating(state.taskRunStates),
        }));
        throw error;
      }
    },

    reset: () => {
      set({
        taskRunStates: {},
        focusedTaskRunId: null,
        isOrchestrating: false,
        pendingOrchPermissions: [],
        discoveredSkills: null,
        restoredTaskRunIds: [],
      });
    },
  })
);

// ---------------------------------------------------------------------------
// Orchestration event listeners
// ---------------------------------------------------------------------------
let orchestrationUnlistenFns: Array<() => void> = [];

export function initializeOrchestrationListeners() {
  if (!isTauri() || orchestrationUnlistenFns.length > 0) return;

  console.log('[Orchestration] Initializing event listeners');

  // orchestration:started
  tauriListen<any>('orchestration:started', (payload) => {
    console.log('[Orchestration] Started:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId) return;
    useOrchestrationStore.setState((state) => {
      const updated = upsertTaskRunState(state, taskRunId, (cur) => ({
        taskRun: {
          ...cur.taskRun,
          status: payload?.status || 'analyzing',
          // Preserve workspace_id from event so placeholder gets correct scope
          workspace_id: cur.taskRun.workspace_id ?? (payload?.workspaceId ?? null),
        },
      }));
      const newStates = updated.taskRunStates ?? state.taskRunStates;
      return { ...updated, isOrchestrating: computeIsOrchestrating(newStates) };
    });
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:skills_discovered
  tauriListen<any>('orchestration:skills_discovered', (payload) => {
    console.log('[Orchestration] Skills discovered:', payload);
    useOrchestrationStore.getState().discoverWorkspaceSkills();
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:plan_ready
  tauriListen<any>('orchestration:plan_ready', (payload) => {
    console.log('[Orchestration] Plan ready:', payload);
    const taskRunId = payload?.taskRunId;
    const plan = payload?.plan as TaskPlan | undefined;
    if (!taskRunId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, () => ({ taskPlan: plan ?? null }))
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:plan_validated
  tauriListen<any>('orchestration:plan_validated', (payload) => {
    console.log('[Orchestration] Plan validated:', payload);
    const taskRunId = payload?.taskRunId;
    const validation = payload?.validation as PlanValidation | undefined;
    if (!taskRunId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, () => ({ planValidation: validation ?? null }))
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_started
  tauriListen<any>('orchestration:agent_started', (payload) => {
    console.log('[Orchestration] Agent started:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.agentId) return;
    useOrchestrationStore.setState((state) => {
      const updated = upsertTaskRunState(state, taskRunId, (cur) => ({
        streamingAgentId: payload.agentId,
        streamedContent: '',
        expandedAgentId: payload.agentId,
        agentTracking: {
          ...cur.agentTracking,
          [payload.agentId]: {
            agentId: payload.agentId,
            agentName: payload.agentName || '',
            model: payload.model || '',
            status: 'running' as const,
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
      const newStates = updated.taskRunStates ?? state.taskRunStates;
      return {
        ...updated,
        isOrchestrating: computeIsOrchestrating(newStates),
        // Auto-focus if no task is currently focused
        focusedTaskRunId: state.focusedTaskRunId ?? taskRunId,
      };
    });
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_chunk
  tauriListen<any>('orchestration:agent_chunk', (payload) => {
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.text || !payload?.agentId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (trs) => {
        const existing = trs.agentTracking[payload.agentId];
        if (!existing) return {};
        return {
          streamedContent: trs.streamedContent + payload.text,
          agentTracking: {
            ...trs.agentTracking,
            [payload.agentId]: {
              ...existing,
              streamedContent: existing.streamedContent + payload.text,
            },
          },
        };
      })
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_tool_call
  tauriListen<any>('orchestration:agent_tool_call', (payload) => {
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.agentId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (trs) => {
        const existing = trs.agentTracking[payload.agentId];
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
            ...trs.agentTracking,
            [payload.agentId]: { ...existing, toolCalls: updatedCalls },
          },
        };
      })
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_thought
  tauriListen<any>('orchestration:agent_thought', (payload) => {
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.text || !payload?.agentId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (trs) => {
        const existing = trs.agentTracking[payload.agentId];
        if (!existing) return {};
        return {
          agentTracking: {
            ...trs.agentTracking,
            [payload.agentId]: {
              ...existing,
              streamedContent: existing.streamedContent + payload.text,
            },
          },
        };
      })
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:a2a_call
  tauriListen<any>('orchestration:a2a_call', (payload) => {
    console.log('[Orchestration] A2A call:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.callerAgentId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (trs) => {
        const caller = trs.agentTracking[payload.callerAgentId];
        if (!caller) return {};
        const a2aCalls = [...(caller.a2aCalls ?? []), {
          targetAgentId: payload.targetAgentId ?? '',
          targetAgentName: '',
          prompt: payload.prompt ?? '',
          iteration: payload.iteration ?? 0,
        }];
        return {
          agentTracking: {
            ...trs.agentTracking,
            [payload.callerAgentId]: { ...caller, a2aCalls },
          },
        };
      })
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:a2a_result
  tauriListen<any>('orchestration:a2a_result', (payload) => {
    console.log('[Orchestration] A2A result:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.callerAgentId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (trs) => {
        const caller = trs.agentTracking[payload.callerAgentId];
        if (!caller) return {};
        const a2aCalls = (caller.a2aCalls ?? []).map((call) => {
          if (call.targetAgentId === payload.targetAgentId && call.iteration === payload.iteration) {
            return { ...call, result: payload.resultPreview ?? '' };
          }
          return call;
        });
        return {
          agentTracking: {
            ...trs.agentTracking,
            [payload.callerAgentId]: { ...caller, a2aCalls },
          },
        };
      })
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:agent_completed
  tauriListen<any>('orchestration:agent_completed', (payload) => {
    console.log('[Orchestration] Agent completed:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId || !payload?.agentId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (trs) => {
        const existing = trs.agentTracking[payload.agentId];
        if (!existing) return {};

        const updatedTracking: Record<string, AgentTrackingInfo> = {
          ...trs.agentTracking,
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
        };

        const nextRunning = Object.values(updatedTracking).find(
          (info) => info.agentId !== payload.agentId && info.status === 'running'
        );

        return {
          streamingAgentId: nextRunning?.agentId ?? null,
          streamedContent: nextRunning?.streamedContent ?? '',
          agentTracking: updatedTracking,
        };
      })
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:awaiting_confirmation
  tauriListen<any>('orchestration:awaiting_confirmation', (payload) => {
    console.log('[Orchestration] Awaiting confirmation:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId) return;
    useOrchestrationStore.setState((state) =>
      upsertTaskRunState(state, taskRunId, (cur) => ({
        isAwaitingConfirmation: true,
        taskRun: { ...cur.taskRun, status: 'awaiting_confirmation' as const },
      }))
    );
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:orch_permission
  tauriListen<any>('orchestration:orch_permission', (payload) => {
    console.log('[Orchestration] Permission request:', payload);
    if (payload) {
      const newPerm: OrchPermissionRequest = {
        taskRunId: payload.taskRunId || '',
        agentId: payload.agentId || '',
        requestId: payload.requestId || '',
        sessionId: payload.sessionId || '',
        toolCall: payload.toolCall || undefined,
        options: payload.options || [],
      };
      useOrchestrationStore.setState((state) => ({
        pendingOrchPermissions: [...state.pendingOrchPermissions, newPerm],
      }));
    }
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:feedback
  tauriListen<any>('orchestration:feedback', (payload) => {
    console.log('[Orchestration] Feedback:', payload);
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:completed
  tauriListen<any>('orchestration:completed', (payload) => {
    console.log('[Orchestration] Completed:', payload);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId) return;
    useOrchestrationStore.setState((state) => {
      const updated = upsertTaskRunState(state, taskRunId, (cur) => ({
        isAwaitingConfirmation: false,
        taskRun: {
          ...cur.taskRun,
          status: 'completed' as const,
          result_summary: payload?.summary || null,
          total_tokens_in: payload?.totalTokensIn ?? 0,
          total_tokens_out: payload?.totalTokensOut ?? 0,
          total_cache_creation_tokens: payload?.totalCacheCreationTokens ?? 0,
          total_cache_read_tokens: payload?.totalCacheReadTokens ?? 0,
          total_duration_ms: payload?.totalDurationMs ?? 0,
        },
      }));
      const newStates = updated.taskRunStates ?? state.taskRunStates;
      return { ...updated, isOrchestrating: computeIsOrchestrating(newStates) };
    });
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  // orchestration:error
  tauriListen<any>('orchestration:error', (payload) => {
    let errorMsg = 'Unknown orchestration error';
    try {
      errorMsg =
        payload?.error ||
        (typeof payload === 'string' ? payload : JSON.stringify(payload));
    } catch {
      // payload may not be serializable
    }
    console.error('[Orchestration] Error:', errorMsg);
    showError('编排错误', errorMsg);
    const taskRunId = payload?.taskRunId;
    if (!taskRunId) return;
    useOrchestrationStore.setState((state) => {
      const updated = upsertTaskRunState(state, taskRunId, (cur) => ({
        isAwaitingConfirmation: false,
        taskRun: { ...cur.taskRun, status: 'failed' as const },
      }));
      const newStates = updated.taskRunStates ?? state.taskRunStates;
      return { ...updated, isOrchestrating: computeIsOrchestrating(newStates) };
    });
  }).then((unlisten) => orchestrationUnlistenFns.push(unlisten));

  console.log('[Orchestration] Event listeners initialized');
}

export function cleanupOrchestrationListeners() {
  orchestrationUnlistenFns.forEach((unlisten) => unlisten());
  orchestrationUnlistenFns = [];
}
