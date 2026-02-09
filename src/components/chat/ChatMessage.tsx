"use client";

import { User, Bot } from "lucide-react";
import type { ChatMessage as ChatMessageType } from "@/types/chat";
import { ToolCallDisplay } from "./ToolCallDisplay";
import { MarkdownContent } from "./MarkdownContent";
import { CollapsibleContent } from "./CollapsibleContent";

interface ChatMessageProps {
  message: ChatMessageType;
  isStreaming?: boolean;
}

export function ChatMessage({ message, isStreaming }: ChatMessageProps) {
  const isUser = message.role === "User";

  let textContent = "";
  try {
    const blocks = JSON.parse(message.content_json);
    if (Array.isArray(blocks)) {
      textContent = blocks
        .filter((b: { type: string }) => b.type === "text")
        .map((b: { text?: string }) => b.text ?? "")
        .join("\n");
    }
  } catch {
    textContent = message.content_json;
  }

  let toolCalls = null;
  if (message.tool_calls_json) {
    try {
      toolCalls = JSON.parse(message.tool_calls_json);
    } catch {
      // ignore
    }
  }

  if (isUser) {
    return (
      <div className="flex justify-end gap-4 pl-12">
        <div className="flex flex-col items-end gap-1 max-w-[80%]">
          <div className="bg-slate-200 dark:bg-[#1E1E2E] text-slate-900 dark:text-white px-5 py-3 rounded-2xl rounded-tr-sm border border-slate-300 dark:border-white/5 shadow-sm">
            <CollapsibleContent>
              <p className="text-sm leading-relaxed font-body whitespace-pre-wrap">{textContent}</p>
            </CollapsibleContent>
          </div>
        </div>
        <div className="size-8 rounded-full border border-slate-300 dark:border-border-dark flex-none bg-slate-200 dark:bg-surface-dark flex items-center justify-center">
          <User className="size-4 text-slate-500" />
        </div>
      </div>
    );
  }

  return (
    <div className="flex gap-4 pr-12">
      <div className="size-8 rounded-full flex-none bg-primary/20 flex items-center justify-center">
        <Bot className="size-4 text-primary" />
      </div>
      <div className="flex flex-col gap-2 max-w-[80%]">
        <div className="bg-white dark:bg-surface-dark text-slate-900 dark:text-gray-200 px-5 py-3 rounded-2xl rounded-tl-sm border border-slate-200 dark:border-border-dark shadow-sm">
          <CollapsibleContent>
            <MarkdownContent content={textContent} className="text-sm leading-relaxed font-body" />
          </CollapsibleContent>
          {isStreaming && (
            <span className="inline-block w-2 h-4 bg-primary animate-pulse ml-1" />
          )}
        </div>
        {toolCalls && Array.isArray(toolCalls) && (
          <div className="space-y-2">
            {toolCalls.map((tc: { id: string; name: string; status: string; result?: string }) => (
              <ToolCallDisplay key={tc.id} toolCall={tc} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
