"use client";

import type { TaskRun, AgentTrackingInfo } from "@/types/orchestration";

interface TrackingSummaryProps {
  taskRun: TaskRun;
  agentTracking: Record<string, AgentTrackingInfo>;
}

export function TrackingSummary({ taskRun, agentTracking }: TrackingSummaryProps) {
  const trackingEntries = Object.values(agentTracking);
  const totalDuration = formatDuration(taskRun.total_duration_ms);

  return (
    <div className="rounded-lg border border-slate-200 dark:border-border-dark/50 bg-white dark:bg-surface-dark p-3">
      <p className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400 mb-3">
        Summary
      </p>

      {/* Status badge */}
      <div className="flex items-center gap-2 mb-3">
        <span
          className={`text-[10px] font-bold uppercase px-2 py-0.5 rounded ${
            taskRun.status === "completed"
              ? "bg-emerald-500/10 text-emerald-400"
              : taskRun.status === "failed"
              ? "bg-red-500/10 text-red-400"
              : "bg-slate-500/10 text-slate-400"
          }`}
        >
          {taskRun.status}
        </span>
        <span className="text-xs text-slate-400 dark:text-gray-500">{totalDuration}</span>
      </div>

      {/* Agent breakdown table */}
      {trackingEntries.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-slate-400 dark:text-gray-500 border-b border-slate-100 dark:border-border-dark/30">
                <th className="text-left py-1.5 pr-3 font-medium">#</th>
                <th className="text-left py-1.5 pr-3 font-medium">Agent</th>
                <th className="text-left py-1.5 pr-3 font-medium">Model</th>
                <th className="text-right py-1.5 pr-3 font-medium">Duration</th>
                <th className="text-right py-1.5 font-medium">Status</th>
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
                    {formatDuration(entry.durationMs)}
                  </td>
                  <td className="py-1.5 text-right">
                    <span
                      className={`text-[10px] font-medium ${
                        entry.status === "completed"
                          ? "text-emerald-400"
                          : entry.status === "failed"
                          ? "text-red-400"
                          : "text-slate-400"
                      }`}
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

      {/* Result summary */}
      {taskRun.result_summary && (
        <div className="mt-3 pt-3 border-t border-slate-100 dark:border-border-dark/30">
          <p className="text-xs text-slate-500 dark:text-gray-500 font-medium mb-1">Result</p>
          <p className="text-xs text-slate-600 dark:text-gray-400 whitespace-pre-wrap">
            {taskRun.result_summary}
          </p>
        </div>
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
