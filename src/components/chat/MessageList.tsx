"use client";

import { useChatStore } from "@/stores/chatStore";
import { ChatMessage } from "./ChatMessage";
import { ToolCallDisplay } from "./ToolCallDisplay";
import { MarkdownContent } from "./MarkdownContent";
import { useEffect, useRef } from "react";
import { Bot } from "lucide-react";

export function MessageList() {
  const messages = useChatStore((s) => s.messages);
  const streamedContent = useChatStore((s) => s.streamedContent);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const toolCalls = useChatStore((s) => s.toolCalls);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamedContent, toolCalls]);

  const hasStreamingContent = isStreaming && (streamedContent || toolCalls.length > 0);

  return (
    <>
      {messages.length === 0 && !isStreaming && (
        <div className="flex-1 flex items-center justify-center text-slate-400 dark:text-gray-600 text-sm">
          Start a conversation with your agent...
        </div>
      )}
      {messages.map((msg) => (
        <ChatMessage key={msg.id} message={msg} />
      ))}
      {hasStreamingContent && (
        <div className="flex gap-4 pr-12">
          <div className="size-8 rounded-full flex-none bg-primary/20 flex items-center justify-center">
            <Bot className="size-4 text-primary" />
          </div>
          <div className="flex flex-col gap-2 min-w-0 flex-1 max-w-[80%]">
            {/* Tool calls */}
            {toolCalls.length > 0 && (
              <div className="space-y-1.5">
                {toolCalls.map((tc: any) => (
                  <ToolCallDisplay key={tc.id} toolCall={tc} />
                ))}
              </div>
            )}
            {/* Streamed text */}
            {streamedContent && (
              <div className="bg-white dark:bg-surface-dark text-slate-900 dark:text-gray-200 px-5 py-3 rounded-2xl rounded-tl-sm border border-slate-200 dark:border-border-dark shadow-sm">
                <MarkdownContent
                  content={streamedContent}
                  className="text-sm leading-relaxed font-body"
                />
                <span className="inline-block w-2 h-4 bg-primary animate-pulse ml-1" />
              </div>
            )}
            {/* Show loading indicator if streaming but no content yet */}
            {isStreaming && !streamedContent && toolCalls.length === 0 && (
              <div className="flex items-center gap-2 text-slate-400 text-xs">
                <span className="inline-block w-2 h-4 bg-primary animate-pulse" />
                <span>Agent is thinking...</span>
              </div>
            )}
          </div>
        </div>
      )}
      {/* Show thinking indicator when streaming starts but nothing has come yet */}
      {isStreaming && !streamedContent && toolCalls.length === 0 && (
        <div className="flex gap-4 pr-12">
          <div className="size-8 rounded-full flex-none bg-primary/20 flex items-center justify-center">
            <Bot className="size-4 text-primary" />
          </div>
          <div className="flex items-center gap-2 text-slate-400 dark:text-gray-500 text-xs py-2">
            <span className="inline-block w-2 h-4 bg-primary animate-pulse" />
            <span>Agent is thinking...</span>
          </div>
        </div>
      )}
      <div ref={endRef} />
    </>
  );
}
