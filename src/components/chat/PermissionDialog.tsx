"use client";

import { useState, useEffect } from "react";
import { X, CheckCircle, XCircle, Info } from "lucide-react";

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

interface PermissionDialogProps {
  request: PermissionRequest;
  onResponse: (optionId: string, userMessage?: string) => void;
  onClose: () => void;
}

export function PermissionDialog({ request, onResponse, onClose }: PermissionDialogProps) {
  const [selectedOption, setSelectedOption] = useState<string>("allow");
  const [userMessage, setUserMessage] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const toolName = request.toolCall?.title || "Tool execution";
  const toolInput = request.toolCall?.rawInput;

  const handleSubmit = async () => {
    setIsSubmitting(true);
    try {
      await onResponse(selectedOption, userMessage.trim() || undefined);
      onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  // Auto-select "Allow" by default if not set
  useEffect(() => {
    if (!selectedOption) {
      const allowOption = request.options.find((o) => o.optionId === "allow");
      if (allowOption) setSelectedOption("allow");
    }
  }, [request.options, selectedOption]);

  return (
    <div className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div className="bg-white dark:bg-surface-dark rounded-xl shadow-2xl max-w-lg w-full border border-slate-200 dark:border-border-dark">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-200 dark:border-border-dark">
          <div className="flex items-center gap-3">
            <div className="size-8 rounded-lg bg-amber-500/20 flex items-center justify-center">
              <Info className="size-4 text-amber-500" />
            </div>
            <h3 className="font-semibold text-slate-900 dark:text-white">
              Permission Request
            </h3>
          </div>
          <button
            onClick={onClose}
            className="size-8 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 flex items-center justify-center transition-colors"
          >
            <X className="size-4 text-slate-500" />
          </button>
        </div>

        {/* Content */}
        <div className="px-5 py-4">
          <p className="text-sm text-slate-700 dark:text-gray-300 mb-3">
            The agent is requesting permission to:
          </p>

          {/* Tool call details */}
          <div className="bg-slate-100 dark:bg-[#0A0A10] rounded-lg p-3 mb-4 border border-slate-200 dark:border-border-dark">
            <div className="font-mono text-sm font-medium text-slate-800 dark:text-gray-200 mb-1">
              {toolName}
            </div>
            {toolInput && (
              <pre className="text-xs text-slate-600 dark:text-gray-400 overflow-x-auto">
                {typeof toolInput === "string"
                  ? toolInput
                  : JSON.stringify(toolInput, null, 2)}
              </pre>
            )}
          </div>

          {/* Options */}
          <div className="space-y-2 mb-4">
            {request.options.map((option) => {
              const isSelected = selectedOption === option.optionId;
              return (
                <button
                  key={option.optionId}
                  onClick={() => setSelectedOption(option.optionId)}
                  className={`w-full text-left px-4 py-3 rounded-lg border-2 transition-all flex items-center gap-3 ${
                    isSelected
                      ? option.optionId === "reject"
                        ? "border-rose-500 bg-rose-500/10"
                        : "border-primary bg-primary/10"
                      : "border-slate-200 dark:border-border-dark hover:border-slate-300 dark:hover:border-slate-700"
                  }`}
                >
                  <div className={`size-4 rounded-full border-2 flex items-center justify-center ${
                    isSelected
                      ? option.optionId === "reject"
                        ? "border-rose-500 bg-rose-500"
                        : "border-primary bg-primary"
                      : "border-slate-300 dark:border-slate-600"
                  }`}>
                    {isSelected && (
                      <CheckCircle className="size-3 text-white" />
                    )}
                  </div>
                  <span className="text-sm font-medium text-slate-900 dark:text-white">
                    {option.name}
                  </span>
                </button>
              );
            })}
          </div>

          {/* User message input */}
          <div className="mb-2">
            <label className="text-xs font-medium text-slate-600 dark:text-gray-400 mb-1.5 block">
              Add a note (optional)
            </label>
            <textarea
              value={userMessage}
              onChange={(e) => setUserMessage(e.target.value)}
              placeholder="Provide additional context or instructions..."
              rows={2}
              className="w-full px-3 py-2 text-sm rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-darkest text-slate-900 dark:text-white placeholder:text-slate-400 focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary resize-none"
            />
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-slate-200 dark:border-border-dark">
          <button
            onClick={onClose}
            disabled={isSubmitting}
            className="px-4 py-2 text-sm font-medium text-slate-700 dark:text-gray-300 hover:text-slate-900 dark:hover:text-white transition-colors disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSubmitting || !selectedOption}
            className={`px-4 py-2 text-sm font-medium text-white rounded-lg transition-all flex items-center gap-2 ${
              isSubmitting || !selectedOption
                ? "bg-slate-300 dark:bg-slate-700 cursor-not-allowed"
                : selectedOption === "reject"
                ? "bg-rose-500 hover:bg-rose-600"
                : "bg-primary hover:bg-primary/90"
            }`}
          >
            {isSubmitting ? (
              <>
                <span className="inline-block size-3 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                Sending...
              </>
            ) : (
              <>
                {selectedOption === "reject" ? (
                  <XCircle className="size-4" />
                ) : (
                  <CheckCircle className="size-4" />
                )}
                {request.options.find((o) => o.optionId === selectedOption)?.name || "Submit"}
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
