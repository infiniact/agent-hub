"use client";

import { useState, useRef, useEffect, useCallback } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { useChatStore } from "@/stores/chatStore";
import { useAgentStore } from "@/stores/agentStore";
import { useOrchestrationStore } from "@/stores/orchestrationStore";

export function ChatInput() {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sendPrompt = useChatStore((s) => s.sendPrompt);
  const ensureSession = useChatStore((s) => s.ensureSession);
  const currentSessionId = useChatStore((s) => s.currentSessionId);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const agents = useAgentStore((s) => s.agents);
  const controlHubAgentId = useAgentStore((s) => s.controlHubAgentId);
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const startOrchestration = useOrchestrationStore((s) => s.startOrchestration);
  const continueOrchestration = useOrchestrationStore((s) => s.continueOrchestration);
  const activeTaskRun = useOrchestrationStore((s) => s.activeTaskRun);

  const isOrchestrationMode = !!controlHubAgentId;
  const isTaskCompleted = activeTaskRun &&
    ['completed', 'failed', 'cancelled'].includes(activeTaskRun.status);
  const selectedAgent = agents.find((a) => a.id === selectedAgentId);
  const hubAgent = agents.find((a) => a.id === controlHubAgentId);

  const handleSend = useCallback(async () => {
    const trimmedText = text.trim();
    console.log('[ChatInput] handleSend called:', {
      hasText: !!trimmedText,
      isStreaming,
      isOrchestrating,
      isOrchestrationMode,
      currentSessionId,
      selectedAgentId
    });

    if (!trimmedText || isStreaming || isOrchestrating) {
      return;
    }

    // Continue mode: task completed/failed/cancelled, send supplementary instructions
    if (isTaskCompleted) {
      console.log('[ChatInput] Continuing orchestration with additional instructions...');
      try {
        await continueOrchestration(trimmedText);
        setText("");
      } catch (e) {
        console.error('[ChatInput] Failed to continue orchestration:', e);
      }
      return;
    }

    if (isOrchestrationMode) {
      // Orchestration mode: send to Control Hub orchestrator
      console.log('[ChatInput] Starting orchestration...');
      try {
        await startOrchestration(trimmedText);
        setText("");
      } catch (e) {
        console.error('[ChatInput] Failed to start orchestration:', e);
      }
      return;
    }

    // Direct mode: send to selected agent
    let sessionId = currentSessionId;
    if (!sessionId) {
      if (!selectedAgentId) {
        console.log('[ChatInput] No agent selected');
        return;
      }
      console.log('[ChatInput] No session, creating one...');
      try {
        sessionId = await ensureSession(selectedAgentId);
      } catch (e) {
        console.error('[ChatInput] Failed to ensure session:', e);
        return;
      }
    }

    try {
      await sendPrompt(sessionId, trimmedText);
      setText("");
    } catch (e) {
      console.error('[ChatInput] Failed to send prompt:', e);
    }
  }, [text, isStreaming, isOrchestrating, isTaskCompleted, isOrchestrationMode,
      currentSessionId, selectedAgentId, continueOrchestration, startOrchestration,
      ensureSession, sendPrompt]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.nativeEvent.isComposing) return;
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      e.preventDefault();
      handleSend();
    }
  };

  useEffect(() => {
    textareaRef.current?.focus();
  }, [currentSessionId]);

  return (
    <div className="pb-8 pt-2 px-8 bg-slate-50 dark:bg-[#07070C]">
      <div className="max-w-6xl mx-auto">
        {/* Mode indicator pill */}
        <div className="flex items-center gap-2 mb-2 px-1">
          {isOrchestrationMode ? (
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-amber-500/10 border border-amber-500/20">
              <Codicon name="star-full" className="text-[12px] text-amber-400" />
              <span className="text-[10px] font-bold text-amber-400 uppercase tracking-wider">
                Control Hub: {hubAgent?.name ?? "Unknown"}
              </span>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-primary/10 border border-primary/20">
              <Codicon name="account" className="text-[12px] text-primary" />
              <span className="text-[10px] font-bold text-primary uppercase tracking-wider">
                Direct: {selectedAgent?.name ?? "No agent"}
              </span>
            </div>
          )}
        </div>

        <div className="relative bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl focus-within:ring-1 focus-within:ring-primary/50 transition-all">
          <textarea
            ref={textareaRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            className="w-full bg-transparent border-none text-slate-900 dark:text-white px-4 py-4 min-h-[100px] text-sm focus:ring-0 placeholder:text-slate-400 font-body resize-none outline-none"
            placeholder={
              isTaskCompleted
                ? "Add context or instructions to restart..."
                : isOrchestrationMode
                ? "Describe a task for the Control Hub to orchestrate..."
                : "Type your message..."
            }
          />
          <div className="flex items-center justify-between px-4 py-3 bg-slate-50/50 dark:bg-white/[0.02] rounded-b-xl border-t border-slate-100 dark:border-border-dark/30">
            <div className="flex items-center gap-2">
              <button
                className="size-9 rounded-lg hover:bg-slate-200/50 dark:hover:bg-white/10 flex items-center justify-center text-slate-400 transition-colors"
                title="Attach image"
              >
                <Codicon name="file-media" className="text-[20px]" />
              </button>
              <button
                className="size-9 rounded-lg hover:bg-slate-200/50 dark:hover:bg-white/10 flex items-center justify-center text-slate-400 transition-colors"
                title="Upload file"
              >
                <Codicon name="file" className="text-[20px]" />
              </button>
              <button
                className="size-9 rounded-lg hover:bg-slate-200/50 dark:hover:bg-white/10 flex items-center justify-center text-slate-400 transition-colors"
                title="Voice input"
              >
                <Codicon name="unmute" className="text-[20px]" />
              </button>
            </div>
            <div className="flex items-center gap-3">
              <span className="text-[10px] text-slate-400 dark:text-gray-500 font-medium">
                ⌘ + Enter to send
              </span>
              <button
                onClick={handleSend}
                disabled={!text.trim() || isStreaming || isOrchestrating}
                className="h-9 px-5 bg-primary hover:bg-cyan-400 disabled:opacity-50 disabled:cursor-not-allowed text-background-dark rounded-lg flex items-center gap-2 font-bold text-xs transition-all shadow-lg shadow-primary/20"
              >
                {isTaskCompleted ? (
                  <>Continue <Codicon name="debug-restart" /></>
                ) : isOrchestrationMode ? (
                  <>Orchestrate <Codicon name="arrow-right" /></>
                ) : (
                  <>Send <Codicon name="arrow-right" /></>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
