"use client";

import { useEffect, useMemo, useState } from "react";
import { MessageList } from "./MessageList";
import { ChatInput } from "./ChatInput";
import { OrchestrationPanel } from "@/components/orchestration/OrchestrationPanel";
import { TaskHistoryPanel } from "@/components/orchestration/TaskHistoryPanel";
import { TaskContextEditor } from "@/components/orchestration/TaskContextEditor";
import { InlinePermission } from "@/components/chat/InlinePermission";
import { useOrchestrationStore, buildTaskContext } from "@/stores/orchestrationStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";

function TaskSwitcherBar({ onNewTask }: { onNewTask: () => void }) {
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const taskRunStates = useOrchestrationStore((s) => s.taskRunStates);
  const focusedTaskRunId = useOrchestrationStore((s) => s.focusedTaskRunId);
  const setFocusedTaskRunId = useOrchestrationStore((s) => s.setFocusedTaskRunId);
  const dismissTaskRun = useOrchestrationStore((s) => s.dismissTaskRun);

  const entries = useMemo(
    () => Object.values(taskRunStates).filter(
      (trs) => trs.taskRun.workspace_id === activeWorkspaceId
    ),
    [taskRunStates, activeWorkspaceId]
  );
  if (entries.length === 0) return null;

  const statusDot = (status: string) => {
    if (["pending", "analyzing", "running"].includes(status))
      return "bg-primary animate-pulse";
    if (status === "awaiting_confirmation") return "bg-amber-400";
    if (status === "completed") return "bg-emerald-400";
    if (status === "failed") return "bg-red-400";
    if (status === "cancelled") return "bg-slate-400";
    return "bg-slate-400";
  };

  return (
    <div className="flex items-center gap-1 px-8 pt-3 pb-1 overflow-x-auto">
      {entries.map((trs) => {
        const isFocused = trs.taskRun.id === focusedTaskRunId;
        return (
          <button
            key={trs.taskRun.id}
            onClick={() => setFocusedTaskRunId(trs.taskRun.id)}
            className={cn(
              "group flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-all shrink-0 max-w-[200px]",
              isFocused
                ? "bg-primary/10 text-primary border border-primary/30"
                : "bg-slate-100 dark:bg-white/5 text-slate-500 dark:text-gray-400 border border-transparent hover:bg-slate-200 dark:hover:bg-white/10"
            )}
          >
            <span className={cn("size-2 rounded-full shrink-0", statusDot(trs.taskRun.status))} />
            <span className="truncate">
              {trs.taskRun.title || trs.taskRun.user_prompt.slice(0, 30)}
            </span>
            <span className="text-[10px] font-mono opacity-50">
              {trs.taskRun.id.slice(-6)}
            </span>
            {["completed", "failed", "cancelled"].includes(trs.taskRun.status) && (
              <span
                onClick={(e) => {
                  e.stopPropagation();
                  dismissTaskRun(trs.taskRun.id);
                }}
                className="opacity-0 group-hover:opacity-100 hover:text-red-400 transition-opacity cursor-pointer"
              >
                <Codicon name="close" className="text-[12px]" />
              </span>
            )}
          </button>
        );
      })}
      {/* New task button */}
      <button
        onClick={onNewTask}
        className="flex items-center justify-center size-7 rounded-lg shrink-0 border border-dashed border-slate-300 dark:border-gray-600 text-slate-400 dark:text-gray-500 hover:border-primary hover:text-primary transition-colors"
        title="New task"
      >
        <Codicon name="add" className="text-[14px]" />
      </button>
    </div>
  );
}

export function ChatSection() {
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const taskRunStates = useOrchestrationStore((s) => s.taskRunStates);
  const focusedTaskRunId = useOrchestrationStore((s) => s.focusedTaskRunId);
  const setFocusedTaskRunId = useOrchestrationStore((s) => s.setFocusedTaskRunId);
  const viewingTaskRun = useOrchestrationStore((s) => s.viewingTaskRun);
  const pendingOrchPermissions = useOrchestrationStore((s) => s.pendingOrchPermissions);
  const respondToOrchPermission = useOrchestrationStore((s) => s.respondToOrchPermission);
  const restoredTaskRunIds = useOrchestrationStore((s) => s.restoredTaskRunIds);
  const clearRestoredTaskRunId = useOrchestrationStore((s) => s.clearRestoredTaskRunId);
  const resumeWithEditedContext = useOrchestrationStore((s) => s.resumeWithEditedContext);
  const dismissTaskRun = useOrchestrationStore((s) => s.dismissTaskRun);

  const [userCreatingNew, setUserCreatingNew] = useState(false);

  // Filter task runs to the active workspace
  const wsTaskRunIds = useMemo(
    () => Object.keys(taskRunStates).filter(
      (id) => taskRunStates[id].taskRun.workspace_id === activeWorkspaceId
    ),
    [taskRunStates, activeWorkspaceId]
  );
  const wsHasTaskRuns = wsTaskRunIds.length > 0;
  const focusedInWorkspace = focusedTaskRunId
    ? wsTaskRunIds.includes(focusedTaskRunId)
    : false;

  // Auto-focus on a workspace task run when switching workspaces or when
  // the current focus belongs to another workspace
  useEffect(() => {
    if (wsHasTaskRuns && !focusedInWorkspace && !userCreatingNew) {
      setFocusedTaskRunId(wsTaskRunIds[0]);
    }
  }, [wsHasTaskRuns, focusedInWorkspace, wsTaskRunIds, setFocusedTaskRunId, userCreatingNew]);

  // Reset userCreatingNew when focusedTaskRunId changes to non-null
  useEffect(() => {
    if (focusedTaskRunId !== null) {
      setUserCreatingNew(false);
    }
  }, [focusedTaskRunId]);

  const handleNewTask = () => {
    setFocusedTaskRunId(null);
    setUserCreatingNew(true);
  };

  const showOrchestration = wsHasTaskRuns && focusedInWorkspace;
  const showTaskHistory = !showOrchestration && viewingTaskRun !== null;

  // Restored task context editor
  const firstRestoredId = restoredTaskRunIds.find((id) => taskRunStates[id]);
  const restoredTrs = firstRestoredId ? taskRunStates[firstRestoredId] : null;

  return (
    <>
      {wsHasTaskRuns && <TaskSwitcherBar onNewTask={handleNewTask} />}

      <div className="flex-1 overflow-y-auto px-8 py-6 flex flex-col gap-6">
        {showOrchestration ? (
          <OrchestrationPanel />
        ) : showTaskHistory ? (
          <TaskHistoryPanel />
        ) : (
          <MessageList />
        )}
      </div>

      {/* Permission dialogs — pinned between scroll area and input */}
      {pendingOrchPermissions.length > 0 && (
        <div className="px-8 py-2 border-t border-slate-200 dark:border-border-dark/50 bg-slate-50 dark:bg-[#07070C] flex flex-col gap-2">
          <div className="max-w-6xl mx-auto w-full flex flex-col gap-2">
            {pendingOrchPermissions.map((perm) => {
              const trs = taskRunStates[perm.taskRunId];
              if (!trs || trs.taskRun.workspace_id !== activeWorkspaceId) return null;
              const taskLabel = trs.taskRun.title || trs.taskRun.user_prompt.slice(0, 40);
              const agentInfo = trs?.agentTracking[perm.agentId];
              const isFocused = perm.taskRunId === focusedTaskRunId;
              return (
                <div key={`${perm.taskRunId}-${perm.requestId}`}>
                  {/* Task context label */}
                  {wsTaskRunIds.length > 1 && (
                    <div className={cn(
                      "flex items-center gap-1.5 px-2 py-1 mb-1 rounded-t-lg text-[10px] font-medium",
                      isFocused
                        ? "bg-primary/10 text-primary"
                        : "bg-amber-500/10 text-amber-500"
                    )}>
                      <Codicon name="symbol-number" className="text-[10px]" />
                      <span className="truncate">{taskLabel}</span>
                      {agentInfo && (
                        <>
                          <span className="opacity-50">/</span>
                          <span className="truncate">{agentInfo.agentName}</span>
                        </>
                      )}
                    </div>
                  )}
                  <InlinePermission
                    request={{
                      id: perm.requestId,
                      sessionId: perm.sessionId,
                      toolCall: perm.toolCall,
                      options: perm.options,
                    }}
                    onResponse={(optionId) => {
                      respondToOrchPermission(
                        perm.taskRunId,
                        perm.agentId,
                        String(perm.requestId),
                        optionId
                      );
                    }}
                    onDismiss={() => {
                      respondToOrchPermission(
                        perm.taskRunId,
                        perm.agentId,
                        String(perm.requestId),
                        "allow"
                      );
                    }}
                  />
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Hide chat input when viewing task history */}
      {!showTaskHistory && <ChatInput />}

      {/* Auto-show context editor for restored tasks */}
      {restoredTrs && firstRestoredId && (
        <TaskContextEditor
          open={true}
          onClose={() => clearRestoredTaskRunId(firstRestoredId)}
          taskRunId={firstRestoredId}
          initialContext={buildTaskContext(restoredTrs.taskRun, restoredTrs.agentTracking)}
          taskTitle={restoredTrs.taskRun.title || restoredTrs.taskRun.user_prompt.slice(0, 60)}
          onResume={(id, ctx) => resumeWithEditedContext(id, ctx)}
          onDismiss={(id) => {
            clearRestoredTaskRunId(id);
            dismissTaskRun(id);
          }}
        />
      )}
    </>
  );
}
