"use client";

import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { AgentTracker } from "./AgentTracker";
import { TaskPlanView } from "./TaskPlanView";
import { TrackingSummary } from "./TrackingSummary";
import { Loader2, XCircle } from "lucide-react";

export function OrchestrationPanel() {
  const activeTaskRun = useOrchestrationStore((s) => s.activeTaskRun);
  const taskPlan = useOrchestrationStore((s) => s.taskPlan);
  const agentTracking = useOrchestrationStore((s) => s.agentTracking);
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const streamingAgentId = useOrchestrationStore((s) => s.streamingAgentId);
  const cancelOrchestration = useOrchestrationStore((s) => s.cancelOrchestration);

  if (!activeTaskRun) {
    return (
      <div className="flex items-center justify-center h-full text-slate-400 dark:text-gray-500">
        <p className="text-sm">No active orchestration</p>
      </div>
    );
  }

  const status = activeTaskRun.status;
  const isCompleted = status === "completed" || status === "failed" || status === "cancelled";

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2">
            {!isCompleted && (
              <Loader2 className="size-4 animate-spin text-primary" />
            )}
            <span className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400">
              {status === "analyzing"
                ? "Analyzing Task..."
                : status === "running"
                ? "Executing Agents"
                : status === "completed"
                ? "Completed"
                : status === "failed"
                ? "Failed"
                : status === "cancelled"
                ? "Cancelled"
                : "Pending"}
            </span>
          </div>
        </div>
        {isOrchestrating && (
          <button
            onClick={cancelOrchestration}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-red-400 hover:text-red-300 hover:bg-red-500/10 transition-colors"
          >
            <XCircle className="size-3.5" />
            Cancel
          </button>
        )}
      </div>

      {/* User prompt */}
      <div className="px-3 py-2 rounded-lg bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-border-dark/50">
        <p className="text-xs text-slate-500 dark:text-gray-500 font-medium mb-1">Task</p>
        <p className="text-sm text-slate-700 dark:text-gray-300">{activeTaskRun.user_prompt}</p>
      </div>

      {/* Plan */}
      {taskPlan && <TaskPlanView plan={taskPlan} agentTracking={agentTracking} />}

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
            />
          ))}
        </div>
      )}

      {/* Completion summary */}
      {isCompleted && <TrackingSummary taskRun={activeTaskRun} agentTracking={agentTracking} />}
    </div>
  );
}
