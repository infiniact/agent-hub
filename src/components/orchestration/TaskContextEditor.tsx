"use client";

import { useState } from "react";
import { Modal } from "@/components/ui/Modal";
import { Codicon } from "@/components/ui/Codicon";

interface TaskContextEditorProps {
  open: boolean;
  onClose: () => void;
  taskRunId: string;
  initialContext: string;
  taskTitle: string;
  onResume: (taskRunId: string, editedContext: string) => void;
  onDismiss: (taskRunId: string) => void;
}

export function TaskContextEditor({
  open,
  onClose,
  taskRunId,
  initialContext,
  taskTitle,
  onResume,
  onDismiss,
}: TaskContextEditorProps) {
  const [context, setContext] = useState(initialContext);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleResume = async () => {
    if (!context.trim() || isSubmitting) return;
    setIsSubmitting(true);
    try {
      onResume(taskRunId, context);
      onClose();
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDismiss = () => {
    onDismiss(taskRunId);
    onClose();
  };

  return (
    <Modal open={open} onClose={onClose} title="Edit Context & Resume" className="max-w-2xl">
      {/* Task title */}
      <div className="flex items-center gap-2 mb-3 px-3 py-2 rounded-lg bg-slate-100 dark:bg-white/5 border border-slate-200 dark:border-border-dark/50">
        <Codicon name="symbol-event" className="text-[14px] text-slate-400 shrink-0" />
        <span className="text-xs text-slate-500 dark:text-gray-400 truncate">
          {taskTitle}
        </span>
        <span className="text-[10px] font-mono text-slate-400 dark:text-gray-600 shrink-0 ml-auto">
          {taskRunId.slice(-6)}
        </span>
      </div>

      {/* Context textarea */}
      <div className="relative">
        <textarea
          value={context}
          onChange={(e) => setContext(e.target.value)}
          className="w-full px-3 py-2 rounded-lg bg-slate-50 dark:bg-black/20 border border-slate-200 dark:border-border-dark/50 text-sm text-slate-700 dark:text-gray-300 placeholder:text-slate-400 dark:placeholder:text-gray-500 resize-y focus:outline-none focus:ring-1 focus:ring-primary/50 min-h-[200px] max-h-[400px] font-mono"
          placeholder="Edit the context for the resumed task..."
        />
        <div className="absolute bottom-2 right-3 text-[10px] text-slate-400 dark:text-gray-600 font-mono">
          {context.length.toLocaleString()} chars
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 mt-4">
        <button
          onClick={handleDismiss}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-red-400 hover:text-red-300 hover:bg-red-500/10 transition-colors"
        >
          <Codicon name="trash" className="text-[14px]" />
          Dismiss Task
        </button>
        <div className="flex-1" />
        <button
          onClick={onClose}
          className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium bg-slate-200 dark:bg-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-300 dark:hover:bg-slate-600 transition-colors"
        >
          Cancel
        </button>
        <button
          disabled={!context.trim() || isSubmitting}
          onClick={handleResume}
          className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium bg-primary text-white hover:bg-primary/90 disabled:opacity-50 transition-colors"
        >
          <Codicon name="debug-restart" className="text-[14px]" />
          {isSubmitting ? "Resuming..." : "Resume with Changes"}
        </button>
      </div>
    </Modal>
  );
}
