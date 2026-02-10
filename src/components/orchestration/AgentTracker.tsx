"use client";

import { useState } from "react";
import type { AgentTrackingInfo } from "@/types/orchestration";
import { Codicon } from "@/components/ui/Codicon";

interface AgentTrackerProps {
  info: AgentTrackingInfo;
  isStreaming: boolean;
  isExpanded: boolean;
  onToggleExpand: () => void;
  isAwaitingConfirmation?: boolean;
  onRegenerate?: () => void;
  onCancel?: () => void;
}

export function AgentTracker({
  info,
  isStreaming,
  isExpanded,
  onToggleExpand,
  isAwaitingConfirmation,
  onRegenerate,
  onCancel,
}: AgentTrackerProps) {
  const statusIcon =
    info.status === "completed" ? (
      <Codicon name="pass-filled" className="text-emerald-400" />
    ) : info.status === "failed" ? (
      <Codicon name="error" className="text-red-400" />
    ) : info.status === "cancelled" ? (
      <Codicon name="error" className="text-amber-400" />
    ) : info.status === "running" ? (
      <Codicon name="loading" className="codicon-modifier-spin text-primary" />
    ) : (
      <Codicon name="watch" className="text-slate-400" />
    );

  const durationStr = info.durationMs > 0 ? formatDuration(info.durationMs) : "...";

  return (
    <div className="rounded-lg border border-slate-200 dark:border-border-dark/50 bg-white dark:bg-surface-dark p-3">
      {/* Header row */}
      <div className="flex items-center justify-between">
        <button
          onClick={onToggleExpand}
          className="flex items-center gap-2 min-w-0 flex-1 text-left"
        >
          {statusIcon}
          <span className="text-sm font-medium text-slate-800 dark:text-white truncate">
            {info.agentName}
          </span>
          {info.model && (
            <span className="flex items-center gap-1 text-[10px] text-slate-400 dark:text-gray-500 bg-slate-100 dark:bg-white/5 px-1.5 py-0.5 rounded shrink-0">
              <Codicon name="server-process" className="text-[10px]" />
              {info.model}
            </span>
          )}
          {isExpanded ? (
            <Codicon name="chevron-up" className="text-[14px] text-slate-400 shrink-0" />
          ) : (
            <Codicon name="chevron-down" className="text-[14px] text-slate-400 shrink-0" />
          )}
        </button>

        <div className="flex items-center gap-3 text-[11px] text-slate-400 dark:text-gray-500 shrink-0 ml-2">
          {(info.tokensIn > 0 || info.tokensOut > 0) && (
            <span>{info.tokensIn} in / {info.tokensOut} out</span>
          )}
          <span>{durationStr}</span>
          {info.status === "running" && onCancel && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onCancel();
              }}
              className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium text-red-400 hover:bg-red-500/10 transition-colors"
            >
              <Codicon name="error" className="text-[12px]" />
              Cancel
            </button>
          )}
          {isAwaitingConfirmation && info.status === "completed" && onRegenerate && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onRegenerate();
              }}
              className="flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-medium text-primary hover:bg-primary/10 transition-colors"
            >
              <Codicon name="refresh" className="text-[12px]" />
              Re-run
            </button>
          )}
        </div>
      </div>

      {/* ACP Session ID badge */}
      {info.acpSessionId && (
        <div className="mt-1.5 flex items-center gap-1">
          <Codicon name="symbol-number" className="text-[10px] text-slate-400" />
          <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600 truncate">
            {info.acpSessionId}
          </span>
        </div>
      )}

      {/* Streaming preview (when not expanded) */}
      {!isExpanded && isStreaming && info.streamedContent && (
        <div className="mt-2 max-h-24 overflow-y-auto rounded bg-slate-50 dark:bg-black/20 px-3 py-2 text-xs text-slate-600 dark:text-gray-400 font-mono whitespace-pre-wrap">
          {info.streamedContent.slice(-500)}
          <span className="inline-block w-1.5 h-3.5 bg-primary animate-pulse ml-0.5" />
        </div>
      )}

      {/* Expanded detail view */}
      {isExpanded && (
        <div className="mt-2 pt-2 border-t border-slate-100 dark:border-border-dark/30 space-y-3">
          {/* Tool calls */}
          {info.toolCalls && info.toolCalls.length > 0 && (
            <div>
              <p className="text-[10px] font-bold uppercase tracking-wider text-slate-400 dark:text-gray-500 mb-1.5">
                Tool Calls
              </p>
              <div className="space-y-1">
                {info.toolCalls.map((tc) => (
                  <ToolCallRow key={tc.toolCallId} toolCall={tc} />
                ))}
              </div>
            </div>
          )}

          {/* Full output */}
          {(info.output || info.streamedContent) && (
            <div>
              <p className="text-[10px] font-bold uppercase tracking-wider text-slate-400 dark:text-gray-500 mb-1.5">
                Output
              </p>
              <div className="max-h-64 overflow-y-auto rounded bg-slate-50 dark:bg-black/20 px-3 py-2 text-xs text-slate-600 dark:text-gray-400 font-mono whitespace-pre-wrap">
                {info.output || info.streamedContent}
                {isStreaming && (
                  <span className="inline-block w-1.5 h-3.5 bg-primary animate-pulse ml-0.5" />
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function ToolCallRow({ toolCall }: { toolCall: NonNullable<AgentTrackingInfo["toolCalls"]>[number] }) {
  const [showDetail, setShowDetail] = useState(false);

  const statusColor =
    toolCall.status === "completed" || toolCall.status === "tool_call_update"
      ? "text-emerald-400"
      : toolCall.status === "failed"
      ? "text-red-400"
      : "text-slate-400";

  return (
    <div className="rounded bg-slate-50 dark:bg-black/10 px-2 py-1.5">
      <button
        onClick={() => setShowDetail(!showDetail)}
        className="flex items-center gap-2 w-full text-left"
      >
        <Codicon name="wrench" className={`text-[12px] ${statusColor}`} />
        <span className="text-[11px] font-medium text-slate-700 dark:text-gray-300 truncate flex-1">
          {toolCall.title || toolCall.name || toolCall.toolCallId}
        </span>
        <span className={`text-[10px] ${statusColor}`}>{toolCall.status}</span>
      </button>
      {showDetail && (
        <div className="mt-1.5 space-y-1">
          {toolCall.rawInput && (
            <pre className="text-[10px] text-slate-500 dark:text-gray-500 bg-white dark:bg-slate-900/50 rounded px-2 py-1 overflow-x-auto max-h-32">
              {typeof toolCall.rawInput === "string"
                ? toolCall.rawInput
                : JSON.stringify(toolCall.rawInput, null, 2)}
            </pre>
          )}
          {toolCall.rawOutput && (
            <pre className="text-[10px] text-slate-500 dark:text-gray-500 bg-white dark:bg-slate-900/50 rounded px-2 py-1 overflow-x-auto max-h-32">
              {typeof toolCall.rawOutput === "string"
                ? toolCall.rawOutput
                : JSON.stringify(toolCall.rawOutput, null, 2)}
            </pre>
          )}
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
