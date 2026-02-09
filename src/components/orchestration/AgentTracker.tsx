"use client";

import type { AgentTrackingInfo } from "@/types/orchestration";
import {
  CheckCircle2,
  XCircle,
  Loader2,
  Clock,
  Cpu,
} from "lucide-react";

interface AgentTrackerProps {
  info: AgentTrackingInfo;
  isStreaming: boolean;
}

export function AgentTracker({ info, isStreaming }: AgentTrackerProps) {
  const statusIcon =
    info.status === "completed" ? (
      <CheckCircle2 className="size-4 text-emerald-400" />
    ) : info.status === "failed" ? (
      <XCircle className="size-4 text-red-400" />
    ) : info.status === "running" ? (
      <Loader2 className="size-4 animate-spin text-primary" />
    ) : (
      <Clock className="size-4 text-slate-400" />
    );

  const durationStr = info.durationMs > 0 ? formatDuration(info.durationMs) : "...";

  return (
    <div className="rounded-lg border border-slate-200 dark:border-border-dark/50 bg-white dark:bg-surface-dark p-3">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          {statusIcon}
          <span className="text-sm font-medium text-slate-800 dark:text-white">
            {info.agentName}
          </span>
          {info.model && (
            <span className="flex items-center gap-1 text-[10px] text-slate-400 dark:text-gray-500 bg-slate-100 dark:bg-white/5 px-1.5 py-0.5 rounded">
              <Cpu className="size-2.5" />
              {info.model}
            </span>
          )}
        </div>
        <div className="flex items-center gap-3 text-[11px] text-slate-400 dark:text-gray-500">
          {info.tokensIn > 0 && (
            <span>{info.tokensIn} in / {info.tokensOut} out</span>
          )}
          <span>{durationStr}</span>
        </div>
      </div>

      {/* Streaming preview */}
      {isStreaming && info.streamedContent && (
        <div className="mt-2 max-h-24 overflow-y-auto rounded bg-slate-50 dark:bg-black/20 px-3 py-2 text-xs text-slate-600 dark:text-gray-400 font-mono whitespace-pre-wrap">
          {info.streamedContent.slice(-500)}
          <span className="inline-block w-1.5 h-3.5 bg-primary animate-pulse ml-0.5" />
        </div>
      )}
    </div>
  );
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  return `${mins}m ${secs}s`;
}
