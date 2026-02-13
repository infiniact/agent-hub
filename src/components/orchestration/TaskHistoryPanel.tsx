"use client";

import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { AgentTracker } from "./AgentTracker";
import { TaskPlanView } from "./TaskPlanView";
import { TrackingSummary } from "./TrackingSummary";
import { Codicon } from "@/components/ui/Codicon";
import { MarkdownContent } from "@/components/chat/MarkdownContent";
import { useState } from "react";

/**
 * Panel for viewing historical task runs from the Kanban.
 * Similar to OrchestrationPanel but read-only with no live updates.
 */
export function TaskHistoryPanel() {
  const viewingTaskRun = useOrchestrationStore((s) => s.viewingTaskRun);
  const viewingTaskPlan = useOrchestrationStore((s) => s.viewingTaskPlan);
  const viewingAgentTracking = useOrchestrationStore((s) => s.viewingAgentTracking);
  const clearViewingTaskRun = useOrchestrationStore((s) => s.clearViewingTaskRun);
  const rateTaskRun = useOrchestrationStore((s) => s.rateTaskRun);
  const scheduleTask = useOrchestrationStore((s) => s.scheduleTask);
  const clearSchedule = useOrchestrationStore((s) => s.clearSchedule);
  const [expandedAgentId, setExpandedAgentId] = useState<string | null>(null);

  if (!viewingTaskRun) {
    return (
      <div className="flex items-center justify-center h-full text-slate-400 dark:text-gray-500">
        <p className="text-sm">No task selected</p>
      </div>
    );
  }

  const status = viewingTaskRun.status;
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

  const statusColor =
    status === "completed"
      ? "text-emerald-500"
      : status === "failed"
      ? "text-rose-500"
      : status === "cancelled"
      ? "text-amber-500"
      : "text-slate-500";

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Header with close button */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <button
            onClick={clearViewingTaskRun}
            className="size-7 rounded-lg flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-slate-700 dark:hover:text-white hover:bg-slate-200 dark:hover:bg-white/10 transition-colors"
            title="Close"
          >
            <Codicon name="close" className="text-[16px]" />
          </button>
          <div className="flex items-center gap-2">
            <span className={`text-xs font-bold uppercase tracking-wider ${statusColor}`}>
              {statusLabel}
            </span>
          </div>
          {/* Task run ID for traceability */}
          <div className="flex items-center gap-1">
            <Codicon name="symbol-number" className="text-[10px] text-slate-400" />
            <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600">
              {viewingTaskRun.id.slice(0, 8)}
            </span>
          </div>
        </div>
        <span className="text-[10px] text-slate-400 dark:text-gray-500">
          {new Date(viewingTaskRun.created_at).toLocaleString("zh-CN")}
        </span>
      </div>

      {/* User prompt */}
      <div className="px-3 py-2 rounded-lg bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-border-dark/50">
        <p className="text-xs text-slate-500 dark:text-gray-500 font-medium mb-1">Task</p>
        <div className="text-sm text-slate-700 dark:text-gray-300">
          <MarkdownContent content={viewingTaskRun.user_prompt} className="text-sm" />
        </div>
      </div>

      {/* Plan */}
      {viewingTaskPlan && (
        <TaskPlanView plan={viewingTaskPlan} agentTracking={viewingAgentTracking} />
      )}

      {/* Agent execution tracking */}
      {Object.keys(viewingAgentTracking).length > 0 && (
        <div className="flex flex-col gap-2">
          <p className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400">
            Agent Executions
          </p>
          {Object.values(viewingAgentTracking).map((info) => (
            <AgentTracker
              key={info.agentId}
              info={info}
              isStreaming={false}
              isExpanded={expandedAgentId === info.agentId}
              onToggleExpand={() =>
                setExpandedAgentId(expandedAgentId === info.agentId ? null : info.agentId)
              }
              isAwaitingConfirmation={false}
            />
          ))}
        </div>
      )}

      {/* Completion summary */}
      {isCompleted && (
        <TrackingSummary
          taskRun={viewingTaskRun}
          agentTracking={viewingAgentTracking}
          onRateTask={rateTaskRun}
          onRateComplete={clearViewingTaskRun}
          onScheduleTask={scheduleTask}
          onClearSchedule={clearSchedule}
        />
      )}
    </div>
  );
}
