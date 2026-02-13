"use client";

import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { AgentTracker } from "./AgentTracker";
import { TaskPlanView } from "./TaskPlanView";
import { TrackingSummary } from "./TrackingSummary";
import { InlinePermission } from "@/components/chat/InlinePermission";
import { Codicon } from "@/components/ui/Codicon";
import { MarkdownContent } from "@/components/chat/MarkdownContent";

export function OrchestrationPanel() {
  const activeTaskRun = useOrchestrationStore((s) => s.activeTaskRun);
  const taskPlan = useOrchestrationStore((s) => s.taskPlan);
  const agentTracking = useOrchestrationStore((s) => s.agentTracking);
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const streamingAgentId = useOrchestrationStore((s) => s.streamingAgentId);
  const cancelOrchestration = useOrchestrationStore((s) => s.cancelOrchestration);
  const isAwaitingConfirmation = useOrchestrationStore((s) => s.isAwaitingConfirmation);
  const pendingOrchPermission = useOrchestrationStore((s) => s.pendingOrchPermission);
  const expandedAgentId = useOrchestrationStore((s) => s.expandedAgentId);
  const setExpandedAgentId = useOrchestrationStore((s) => s.setExpandedAgentId);
  const confirmResults = useOrchestrationStore((s) => s.confirmResults);
  const regenerateAgent = useOrchestrationStore((s) => s.regenerateAgent);
  const regenerateAll = useOrchestrationStore((s) => s.regenerateAll);
  const respondToOrchPermission = useOrchestrationStore((s) => s.respondToOrchPermission);
  const cancelAgent = useOrchestrationStore((s) => s.cancelAgent);
  const dismissTaskRun = useOrchestrationStore((s) => s.dismissTaskRun);
  const rateTaskRun = useOrchestrationStore((s) => s.rateTaskRun);
  const scheduleTask = useOrchestrationStore((s) => s.scheduleTask);
  const clearSchedule = useOrchestrationStore((s) => s.clearSchedule);

  if (!activeTaskRun) {
    return (
      <div className="flex items-center justify-center h-full text-slate-400 dark:text-gray-500">
        <p className="text-sm">No active orchestration</p>
      </div>
    );
  }

  const status = activeTaskRun.status;
  const isCompleted = status === "completed" || status === "failed" || status === "cancelled";

  const statusLabel =
    status === "analyzing"
      ? "Analyzing Task..."
      : status === "running"
      ? "Executing Agents"
      : status === "awaiting_confirmation"
      ? "Awaiting Confirmation"
      : status === "completed"
      ? "Completed"
      : status === "failed"
      ? "Failed"
      : status === "cancelled"
      ? "Cancelled"
      : "Pending";

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            {!isCompleted && status !== "awaiting_confirmation" && (
              <Codicon name="loading" className="codicon-modifier-spin text-primary" />
            )}
            {status === "awaiting_confirmation" && (
              <Codicon name="pass-filled" className="text-amber-400" />
            )}
            <span className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400">
              {statusLabel}
            </span>
          </div>
          {/* Task run ID for traceability */}
          <div className="flex items-center gap-1">
            <Codicon name="symbol-number" className="text-[10px] text-slate-400" />
            <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600">
              {activeTaskRun.id.slice(0, 8)}
            </span>
          </div>
        </div>
        {isOrchestrating && (
          <button
            onClick={cancelOrchestration}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-red-400 hover:text-red-300 hover:bg-red-500/10 transition-colors"
          >
            <Codicon name="error" className="text-[14px]" />
            Cancel
          </button>
        )}
      </div>

      {/* User prompt */}
      <div className="px-3 py-2 rounded-lg bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-border-dark/50">
        <p className="text-xs text-slate-500 dark:text-gray-500 font-medium mb-1">Task</p>
        <div className="text-sm text-slate-700 dark:text-gray-300">
          <MarkdownContent content={activeTaskRun.user_prompt} className="text-sm" />
        </div>
      </div>

      {/* Plan */}
      {taskPlan && <TaskPlanView plan={taskPlan} agentTracking={agentTracking} />}

      {/* Orchestration permission dialog */}
      {pendingOrchPermission && (
        <InlinePermission
          request={{
            id: pendingOrchPermission.requestId,
            sessionId: pendingOrchPermission.sessionId,
            toolCall: pendingOrchPermission.toolCall,
            options: pendingOrchPermission.options,
          }}
          onResponse={(optionId) => {
            respondToOrchPermission(
              pendingOrchPermission.taskRunId,
              pendingOrchPermission.agentId,
              String(pendingOrchPermission.requestId),
              optionId
            );
          }}
          onDismiss={() => {
            // Auto-allow on dismiss
            respondToOrchPermission(
              pendingOrchPermission.taskRunId,
              pendingOrchPermission.agentId,
              String(pendingOrchPermission.requestId),
              "allow"
            );
          }}
        />
      )}

      {/* Agent execution tracking */}
      {Object.keys(agentTracking).length > 0 && (
        <div className="flex flex-col gap-2">
          <p className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400">
            Agent Executions
          </p>
          {Object.values(agentTracking).map((info) => (
            <AgentTracker
              key={info.agentId}
              info={info}
              isStreaming={streamingAgentId === info.agentId}
              isExpanded={expandedAgentId === info.agentId}
              onToggleExpand={() =>
                setExpandedAgentId(expandedAgentId === info.agentId ? null : info.agentId)
              }
              isAwaitingConfirmation={isAwaitingConfirmation}
              onRegenerate={() => regenerateAgent(activeTaskRun.id, info.agentId)}
              onCancel={() => cancelAgent(activeTaskRun.id, info.agentId)}
            />
          ))}
        </div>
      )}

      {/* Confirmation bar */}
      {isAwaitingConfirmation && (
        <div className="rounded-lg border-2 border-amber-300 dark:border-amber-700/50 bg-amber-50 dark:bg-amber-950/20 px-4 py-3">
          <p className="text-sm font-medium text-amber-800 dark:text-amber-300 mb-2">
            All agents completed. Review results and confirm.
          </p>
          <div className="flex items-center gap-2">
            <button
              onClick={() => confirmResults(activeTaskRun.id)}
              className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium bg-primary text-white hover:bg-primary/90 transition-colors"
            >
              <Codicon name="pass-filled" className="text-[14px]" />
              Confirm
            </button>
            <button
              onClick={() => regenerateAll(activeTaskRun.id)}
              className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-300 dark:hover:bg-slate-600 transition-colors"
            >
              <Codicon name="refresh" className="text-[14px]" />
              Re-run All
            </button>
          </div>
        </div>
      )}

      {/* Completion summary */}
      {isCompleted && (
        <TrackingSummary
          taskRun={activeTaskRun}
          agentTracking={agentTracking}
          onRateTask={rateTaskRun}
          onRateComplete={dismissTaskRun}
          onScheduleTask={scheduleTask}
          onClearSchedule={clearSchedule}
        />
      )}
    </div>
  );
}
