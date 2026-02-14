"use client";

import { Codicon } from "@/components/ui/Codicon";
import { GeneratedFileBlock } from "@/components/chat/GeneratedFileBlock";
import { useState } from "react";

interface ToolCallProps {
  toolCall: {
    id: string;
    name: string;
    title?: string;
    status: string;
    rawInput?: any;
    rawOutput?: any;
    result?: string;
  };
}

/** Extract file path and content from a fs/write_text_file tool call's rawInput. */
function extractFileWriteInfo(rawInput: any): { path: string; content: string } | null {
  if (!rawInput) return null;
  const input = typeof rawInput === "string" ? (() => { try { return JSON.parse(rawInput); } catch { return null; } })() : rawInput;
  if (!input) return null;
  const path = input.path || input.filePath || input.file_path;
  const content = input.content || input.text || "";
  if (typeof path === "string" && path) return { path, content: String(content) };
  return null;
}

export function ToolCallDisplay({ toolCall }: ToolCallProps) {
  const [expanded, setExpanded] = useState(false);

  const isPending = toolCall.status === "pending";
  const isCompleted = toolCall.status === "completed" || toolCall.status === "complete";
  const isFailed = toolCall.status === "failed" || toolCall.status === "error";
  const isRunning = toolCall.status === "running" || isPending;

  // Check if this is a file write tool call
  const isFileWrite = toolCall.name === "fs/write_text_file" || toolCall.name === "write_text_file";
  const fileInfo = isFileWrite ? extractFileWriteInfo(toolCall.rawInput) : null;

  // If it's a completed file write with valid info, render as GeneratedFileBlock
  if (isFileWrite && fileInfo && (isCompleted || isRunning)) {
    return (
      <div className="space-y-1">
        <div className="flex items-center gap-2 px-1">
          {isRunning && !isCompleted && (
            <Codicon name="loading" className="text-[14px] text-primary codicon-modifier-spin flex-none" />
          )}
          {isCompleted && (
            <Codicon name="pass-filled" className="text-[14px] text-emerald-400 flex-none" />
          )}
          <span className="text-[10px] text-slate-400 dark:text-gray-500">Write file</span>
        </div>
        <GeneratedFileBlock path={fileInfo.path} content={fileInfo.content} />
      </div>
    );
  }

  // Build a short description from title or rawInput
  const description = toolCall.title || toolCall.name;

  // Format output for display
  const outputText = (() => {
    if (toolCall.result) return toolCall.result;
    if (!toolCall.rawOutput) return null;
    if (typeof toolCall.rawOutput === "string") return toolCall.rawOutput;
    try {
      // rawOutput can be an array of content blocks or a string
      if (Array.isArray(toolCall.rawOutput)) {
        return toolCall.rawOutput
          .map((block: any) => {
            if (typeof block === "string") return block;
            if (block.type === "text") return block.text;
            return JSON.stringify(block);
          })
          .join("\n");
      }
      return JSON.stringify(toolCall.rawOutput, null, 2);
    } catch {
      return String(toolCall.rawOutput);
    }
  })();

  const hasDetails = toolCall.rawInput || outputText;

  return (
    <div className="bg-slate-100 dark:bg-[#0A0A10] border border-slate-200 dark:border-border-dark rounded-lg px-3 py-2 text-xs">
      <div
        className={`flex items-center gap-2 ${hasDetails ? "cursor-pointer" : ""}`}
        onClick={() => hasDetails && setExpanded(!expanded)}
      >
        {isRunning && !isCompleted && (
          <Codicon name="loading" className="text-[14px] text-primary codicon-modifier-spin flex-none" />
        )}
        {isCompleted && (
          <Codicon name="pass-filled" className="text-[14px] text-emerald-400 flex-none" />
        )}
        {isFailed && (
          <Codicon name="error" className="text-[14px] text-rose-400 flex-none" />
        )}
        <span className="font-mono font-medium text-slate-700 dark:text-gray-300 truncate">
          {description}
        </span>
        {hasDetails && (
          expanded
            ? <Codicon name="chevron-down" className="text-[12px] text-slate-400 flex-none ml-auto" />
            : <Codicon name="chevron-right" className="text-[12px] text-slate-400 flex-none ml-auto" />
        )}
      </div>
      {expanded && (
        <div className="mt-2 space-y-1.5">
          {toolCall.rawInput && (
            <pre className="text-slate-500 dark:text-gray-500 font-mono overflow-x-auto whitespace-pre-wrap max-h-32 overflow-y-auto text-[10px] leading-tight">
              {typeof toolCall.rawInput === "string"
                ? toolCall.rawInput
                : JSON.stringify(toolCall.rawInput, null, 2)}
            </pre>
          )}
          {outputText && (
            <pre className="text-slate-600 dark:text-gray-400 font-mono overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto text-[10px] leading-tight border-t border-slate-200 dark:border-border-dark pt-1.5">
              {outputText.length > 2000 ? outputText.slice(0, 2000) + "\n... (truncated)" : outputText}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
