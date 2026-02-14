"use client";

import type { TaskRun, AgentTrackingInfo, ScheduleTaskRequest } from "@/types/orchestration";
import { MarkdownContent } from "@/components/chat/MarkdownContent";
import { useState } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";
import { ScheduleDialog } from "./ScheduleDialog";

function formatTokens(n: number): string {
  if (n === 0) return "--";
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return n.toString();
}

interface TrackingSummaryProps {
  taskRun: TaskRun;
  agentTracking: Record<string, AgentTrackingInfo>;
  onRateTask?: (taskRunId: string, rating: number) => void;
  onRateComplete?: () => void;
  onScheduleTask?: (request: ScheduleTaskRequest) => Promise<TaskRun>;
  onClearSchedule?: (taskRunId: string) => Promise<void>;
}

export function TrackingSummary({ taskRun, agentTracking, onRateTask, onRateComplete, onScheduleTask, onClearSchedule }: TrackingSummaryProps) {
  const trackingEntries = Object.values(agentTracking);
  const totalDuration = formatDuration(taskRun.total_duration_ms);
  const isCompleted = taskRun.status === "completed";

  const [userFeedback, setUserFeedback] = useState<'thumbsup' | 'thumbsdown' | null>(
    taskRun.rating ? (taskRun.rating >= 4 ? 'thumbsup' : 'thumbsdown') : null
  );
  const [showScheduleDialog, setShowScheduleDialog] = useState(false);

  const handleFeedback = async (feedback: 'thumbsup' | 'thumbsdown') => {
    setUserFeedback(feedback);
    const rating = feedback === 'thumbsup' ? 5 : 2;
    onRateTask?.(taskRun.id, rating);
    // Brief delay to show the selection, then dismiss back to new conversation
    setTimeout(() => {
      onRateComplete?.();
    }, 500);
  };

  const handleSchedule = async (request: ScheduleTaskRequest) => {
    if (onScheduleTask) {
      await onScheduleTask(request);
    }
  };

  const handleClearSchedule = async () => {
    if (onClearSchedule) {
      await onClearSchedule(taskRun.id);
    }
  };

  return (
    <div className="rounded-lg border border-slate-200 dark:border-border-dark/50 bg-white dark:bg-surface-dark p-3">
      <p className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400 mb-3">
        Summary
      </p>

      {/* Status badge */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              "text-[10px] font-bold uppercase px-2 py-0.5 rounded",
              taskRun.status === "completed"
                ? "bg-emerald-500/10 text-emerald-400"
                : taskRun.status === "failed"
                ? "bg-red-500/10 text-red-400"
                : "bg-slate-500/10 text-slate-400"
            )}
          >
            {taskRun.status}
          </span>
          <span className="text-xs text-slate-400 dark:text-gray-500">{totalDuration}</span>
        </div>
      </div>

      {/* Agent breakdown table */}
      {trackingEntries.length > 0 && (
        <div className="overflow-x-auto mt-3">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-slate-400 dark:text-gray-500 border-b border-slate-100 dark:border-border-dark/30">
                <th className="text-left py-1.5 pr-3 font-medium">#</th>
                <th className="text-left py-1.5 pr-3 font-medium">Agent</th>
                <th className="text-left py-1.5 pr-3 font-medium">Model</th>
                <th className="text-right py-1.5 pr-3 font-medium">Tokens In</th>
                <th className="text-right py-1.5 pr-3 font-medium">Tokens Out</th>
                <th className="text-right py-1.5 pr-3 font-medium">Cache</th>
                <th className="text-right py-1.5 pr-3 font-medium">Duration</th>
                <th className="text-right pr-3 font-medium">Status</th>
              </tr>
            </thead>
            <tbody>
              {trackingEntries.map((entry, idx) => (
                <tr
                  key={entry.agentId}
                  className="border-b border-slate-50 dark:border-border-dark/20 last:border-0"
                >
                  <td className="py-1.5 pr-3 text-slate-400 dark:text-gray-500">
                    {idx + 1}
                  </td>
                  <td className="py-1.5 pr-3 text-slate-700 dark:text-gray-300 font-medium">
                    {entry.agentName}
                  </td>
                  <td className="py-1.5 pr-3 text-slate-400 dark:text-gray-500">
                    {entry.model || "--"}
                  </td>
                  <td className="py-1.5 pr-3 text-right text-slate-400 dark:text-gray-500">
                    {formatTokens(entry.tokensIn)}
                  </td>
                  <td className="py-1.5 pr-3 text-right text-slate-400 dark:text-gray-500">
                    {formatTokens(entry.tokensOut)}
                  </td>
                  <td className="py-1.5 pr-3 text-right text-slate-400 dark:text-gray-500">
                    {(entry.cacheCreationTokens > 0 || entry.cacheReadTokens > 0)
                      ? `${formatTokens(entry.cacheReadTokens)}r / ${formatTokens(entry.cacheCreationTokens)}w`
                      : "--"}
                  </td>
                  <td className="py-1.5 pr-3 text-right text-slate-400 dark:text-gray-500">
                    {formatDuration(entry.durationMs)}
                  </td>
                  <td className="py-1.5 pr-3 text-right">
                    <span
                      className={cn(
                        "text-[10px] font-medium",
                        entry.status === "completed"
                          ? "text-emerald-400"
                          : entry.status === "failed"
                          ? "text-red-400"
                          : "text-slate-400"
                      )}
                    >
                      {entry.status}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Result summary with feedback buttons */}
      {taskRun.result_summary && (
        <div className="mt-3 pt-3 border-t border-slate-100 dark:border-border-dark/30">
          <p className="text-xs font-bold uppercase tracking-wider text-emerald-500 dark:text-emerald-400 mb-2">Result</p>
          <div className="rounded-lg bg-slate-50 dark:bg-black/20 px-4 py-3 text-xs text-slate-600 dark:text-gray-400">
            <MarkdownContent content={taskRun.result_summary} className="text-xs" />
          </div>
        </div>
      )}

      {/* Feedback buttons for completed tasks */}
      {isCompleted && (
        <div className="flex items-center justify-center gap-3 mt-3 pt-3 border-t border-slate-100 dark:border-border-dark/30">
          {/* Schedule button */}
          <button
            onClick={() => setShowScheduleDialog(true)}
            className={cn(
              "p-2.5 rounded-lg transition-all",
              taskRun.schedule_type !== "none"
                ? "bg-blue-100 dark:bg-blue-900/30 text-blue-600"
                : "bg-slate-100 dark:bg-white/5 text-slate-500 dark:text-gray-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 hover:text-blue-500"
            )}
            aria-label="Schedule task"
            title={taskRun.schedule_type !== "none" ? "Edit Schedule" : "Schedule Task"}
          >
            <Codicon
              name="clock"
              className="text-[20px]"
            />
          </button>

          {/* Thumbs up button */}
          <button
            onClick={() => handleFeedback('thumbsup')}
            className={cn(
              "p-2.5 rounded-lg transition-all",
              userFeedback === 'thumbsup'
                ? "bg-emerald-100 dark:bg-emerald-900/30 text-emerald-600"
                : "bg-slate-100 dark:bg-white/5 text-slate-500 dark:text-gray-400 hover:bg-emerald-50 dark:hover:bg-emerald-900/20 hover:text-emerald-500"
            )}
            aria-label="Helpful"
            title="Helpful"
          >
            <Codicon
              name="thumbsup"
              className={cn(
                "text-[20px]",
                userFeedback === 'thumbsup' && "text-emerald-500"
              )}
            />
          </button>

          {/* Thumbs down button */}
          <button
            onClick={() => handleFeedback('thumbsdown')}
            className={cn(
              "p-2.5 rounded-lg transition-all",
              userFeedback === 'thumbsdown'
                ? "bg-rose-100 dark:bg-rose-900/30 text-rose-600"
                : "bg-slate-100 dark:bg-white/5 text-slate-500 dark:text-gray-400 hover:bg-rose-50 dark:hover:bg-rose-900/20 hover:text-rose-500"
            )}
            aria-label="Not helpful"
            title="Not helpful"
          >
            <Codicon
              name="thumbsdown"
              className={cn(
                "text-[20px]",
                userFeedback === 'thumbsdown' && "text-rose-500"
              )}
            />
          </button>

          {/* Dismiss button */}
          <button
            onClick={() => onRateComplete?.()}
            className="p-2.5 rounded-lg bg-slate-100 dark:bg-white/5 text-slate-500 dark:text-gray-400 hover:bg-slate-200 dark:hover:bg-white/10 transition-all"
            aria-label="Dismiss"
            title="Dismiss"
          >
            <Codicon name="close" className="text-[20px]" />
          </button>
        </div>
      )}

      {/* Schedule Dialog */}
      {showScheduleDialog && onScheduleTask && (
        <ScheduleDialog
          taskRun={taskRun}
          onSchedule={handleSchedule}
          onClear={handleClearSchedule}
          onClose={() => setShowScheduleDialog(false)}
        />
      )}
    </div>
  );
}

function formatDuration(ms: number): string {
  if (ms === 0) return "--";
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  return `${mins}m ${secs}s`;
}
