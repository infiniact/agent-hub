"use client";

import { useState } from "react";
import { Check, X, ChevronDown, ChevronUp } from "lucide-react";

interface PermissionRequest {
  id: number | string;
  sessionId: string;
  toolCall?: {
    toolCallId: string;
    title: string;
    rawInput?: any;
  };
  options: Array<{
    optionId: string;
    name: string;
    kind: string;
  }>;
}

interface InlinePermissionProps {
  request: PermissionRequest;
  onResponse: (optionId: string, userMessage?: string) => void;
  onDismiss: () => void;
}

export function InlinePermission({ request, onResponse, onDismiss }: InlinePermissionProps) {
  const [expanded, setExpanded] = useState(false);
  const [selectedOption, setSelectedOption] = useState<string>("allow");
  const [userMessage, setUserMessage] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const toolName = request.toolCall?.title || "Tool execution";

  const handleSubmit = async () => {
    setIsSubmitting(true);
    try {
      await onResponse(selectedOption, userMessage.trim() || undefined);
    } finally {
      setIsSubmitting(false);
    }
  };

  const getButtonColor = (optionId: string) => {
    if (optionId === "reject" || optionId === "reject_once") {
      return selectedOption === optionId
        ? "bg-rose-500 border-rose-500 text-white"
        : "bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-300 hover:border-rose-400";
    }
    return selectedOption === optionId
      ? "bg-primary border-primary text-white"
      : "bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-300 hover:border-primary";
  };

  return (
    <div className="bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800/50 rounded-lg px-3 py-2 mb-2">
      {/* Header - always visible */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <span className="text-xs font-medium text-amber-700 dark:text-amber-400 shrink-0">
            Permission:
          </span>
          <span className="text-xs font-mono text-slate-700 dark:text-gray-300 truncate">
            {toolName}
          </span>
        </div>

        <div className="flex items-center gap-1">
          {/* Quick action buttons when collapsed */}
          {!expanded && (
            <>
              <button
                onClick={() => {
                  setSelectedOption("allow");
                  handleSubmit();
                }}
                disabled={isSubmitting}
                className="px-2 py-1 text-xs font-medium bg-emerald-500 hover:bg-emerald-600 text-white rounded transition-colors disabled:opacity-50"
              >
                Allow
              </button>
              <button
                onClick={() => {
                  setSelectedOption("reject");
                  handleSubmit();
                }}
                disabled={isSubmitting}
                className="px-2 py-1 text-xs font-medium bg-rose-500 hover:bg-rose-600 text-white rounded transition-colors disabled:opacity-50"
              >
                Deny
              </button>
            </>
          )}

          {/* Expand/collapse button */}
          <button
            onClick={() => setExpanded(!expanded)}
            className="size-6 flex items-center justify-center rounded hover:bg-amber-200 dark:hover:bg-amber-900/50 transition-colors"
          >
            {expanded ? (
              <ChevronUp className="size-3.5 text-amber-700 dark:text-amber-400" />
            ) : (
              <ChevronDown className="size-3.5 text-amber-700 dark:text-amber-400" />
            )}
          </button>
        </div>
      </div>

      {/* Expanded content */}
      {expanded && (
        <div className="mt-2 pt-2 border-t border-amber-200 dark:border-amber-800/50 space-y-2">
          {/* Tool input details */}
          {request.toolCall?.rawInput && (
            <pre className="text-xs text-slate-600 dark:text-gray-400 bg-white dark:bg-slate-900/50 rounded px-2 py-1 overflow-x-auto">
              {typeof request.toolCall.rawInput === "string"
                ? request.toolCall.rawInput
                : JSON.stringify(request.toolCall.rawInput, null, 2)}
            </pre>
          )}

          {/* Options */}
          <div className="flex flex-wrap gap-1.5">
            {request.options.map((option) => (
              <button
                key={option.optionId}
                onClick={() => setSelectedOption(option.optionId)}
                className={`px-2.5 py-1 text-xs font-medium border-2 rounded-full transition-all ${
                  getButtonColor(option.optionId)
                }`}
              >
                {option.name}
              </button>
            ))}
          </div>

          {/* User message input */}
          <div className="relative">
            <input
              type="text"
              value={userMessage}
              onChange={(e) => setUserMessage(e.target.value)}
              placeholder="Add a note or instruction..."
              className="w-full px-3 py-1.5 text-xs rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-white placeholder:text-slate-400 focus:outline-none focus:ring-1 focus:ring-amber-500"
            />
            {userMessage && (
              <button
                onClick={() => setUserMessage("")}
                className="absolute right-2 top-1/2 -translate-y-1/2 size-4 flex items-center justify-center rounded-full hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-400"
              >
                <X className="size-3" />
              </button>
            )}
          </div>

          {/* Submit button */}
          <button
            onClick={handleSubmit}
            disabled={isSubmitting}
            className={`w-full py-1.5 text-xs font-medium text-white rounded-lg transition-all flex items-center justify-center gap-1.5 ${
              isSubmitting
                ? "bg-slate-300 dark:bg-slate-700 cursor-not-allowed"
                : selectedOption === "reject" || selectedOption === "reject_once"
                ? "bg-rose-500 hover:bg-rose-600"
                : "bg-primary hover:bg-primary/90"
            }`}
          >
            {isSubmitting ? (
              <span className="inline-block size-3 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            ) : (
              <>
                <Check className="size-3.5" />
                {request.options.find((o) => o.optionId === selectedOption)?.name || "Submit"}
              </>
            )}
          </button>
        </div>
      )}
    </div>
  );
}
