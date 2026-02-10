"use client";

import { useState, useRef, useEffect } from "react";
import { useAgentStore } from "@/stores/agentStore";

const DEFAULT_PROMPT = `You are an expert TypeScript developer with a focus on clean architecture and performance optimization.
When analyzing code:
1. Prioritize type safety.
2. Suggest immutable patterns where possible.
3. Keep component render cycles efficient.
Always explain your reasoning before providing the refactored code block.`;

export function SystemPromptEditor() {
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const updateAgent = useAgentStore((s) => s.updateAgent);

  const agent = agents.find((a) => a.id === selectedAgentId);
  const storeValue = agent?.system_prompt ?? DEFAULT_PROMPT;

  const [draft, setDraft] = useState(storeValue);
  const composingRef = useRef(false);

  // Sync draft when the selected agent changes
  useEffect(() => {
    setDraft(storeValue);
  }, [selectedAgentId]);

  const flush = (value: string) => {
    if (agent && value !== storeValue) {
      updateAgent(agent.id, { system_prompt: value });
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <span className="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest flex justify-between">
        System Prompt
        <span className="text-[10px] font-normal lowercase tracking-normal text-slate-500">
          Controls the agent&apos;s persona and rules
        </span>
      </span>
      <textarea
        className="w-full h-80 bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl p-4 text-slate-700 dark:text-gray-300 text-sm font-mono leading-relaxed focus:ring-1 focus:ring-primary focus:border-primary outline-none resize-none shadow-sm"
        spellCheck={false}
        value={draft}
        onCompositionStart={() => { composingRef.current = true; }}
        onCompositionEnd={(e) => {
          composingRef.current = false;
          const value = (e.target as HTMLTextAreaElement).value;
          setDraft(value);
          flush(value);
        }}
        onChange={(e) => {
          setDraft(e.target.value);
          if (!composingRef.current) {
            flush(e.target.value);
          }
        }}
        onBlur={(e) => {
          flush(e.target.value);
        }}
      />
    </div>
  );
}
