"use client";

import { useEffect, useState } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { Button } from "@/components/ui/Button";
import { cn } from "@/lib/cn";
import { useChatToolStore } from "@/stores/chatToolStore";
import { useAgentStore } from "@/stores/agentStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { ChatToolCreateDialog } from "./ChatToolCreateDialog";
import { ChatToolConfigPanel } from "./ChatToolConfigPanel";
import type { ChatTool } from "@/types/chatTool";

function StatusBadge({ status }: { status: string }) {
  const config: Record<string, { color: string; label: string }> = {
    running: { color: "bg-emerald-500", label: "Running" },
    starting: { color: "bg-amber-400", label: "Starting" },
    login_required: { color: "bg-amber-400", label: "Login Required" },
    waiting_for_login: { color: "bg-amber-400", label: "Waiting" },
    error: { color: "bg-rose-500", label: "Error" },
    stopped: { color: "bg-slate-400", label: "Stopped" },
  };
  const c = config[status] || { color: "bg-slate-400", label: status };

  return (
    <span className="inline-flex items-center gap-1.5 text-[10px] font-medium text-slate-500 dark:text-gray-400">
      <span className={cn("size-2 rounded-full", c.color)} />
      {c.label}
    </span>
  );
}

export function ChatToolPanel() {
  const chatTools = useChatToolStore((s) => s.chatTools);
  const selectedChatToolIdByWorkspace = useChatToolStore((s) => s.selectedChatToolIdByWorkspace);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const selectedChatToolId = selectedChatToolIdByWorkspace[activeWorkspaceId ?? '__global__'] ?? null;
  const selectChatTool = useChatToolStore((s) => s.selectChatTool);
  const fetchChatTools = useChatToolStore((s) => s.fetchChatTools);
  const deleteChatTool = useChatToolStore((s) => s.deleteChatTool);
  const loading = useChatToolStore((s) => s.loading);

  const [showCreate, setShowCreate] = useState(false);

  useEffect(() => {
    fetchChatTools();
  }, [fetchChatTools]);

  const selectedTool = chatTools.find((t) => t.id === selectedChatToolId);

  return (
    <div className="flex h-full bg-white dark:bg-background-dark">
      {/* Left sidebar - tool list */}
      <div className="w-64 shrink-0 border-r border-slate-200 dark:border-border-dark flex flex-col">
        <div className="p-3 border-b border-slate-200 dark:border-border-dark flex items-center justify-between">
          <h2 className="text-sm font-bold text-slate-800 dark:text-white flex items-center gap-2">
            <Codicon name="comment-discussion" className="text-primary" />
            Chat
          </h2>
          <button
            onClick={() => setShowCreate(true)}
            className="size-7 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark hover:border-primary hover:text-primary text-slate-400 dark:text-gray-500 flex items-center justify-center transition-colors"
          >
            <Codicon name="add" className="text-[14px]" />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {loading && chatTools.length === 0 && (
            <div className="text-xs text-slate-400 dark:text-gray-500 text-center py-8">
              Loading...
            </div>
          )}
          {!loading && chatTools.length === 0 && (
            <div className="text-xs text-slate-400 dark:text-gray-500 text-center py-8">
              <Codicon name="comment-discussion" className="text-2xl mb-2 block" />
              No chat tools configured.
              <br />
              <button
                onClick={() => setShowCreate(true)}
                className="text-primary hover:underline mt-2 inline-block"
              >
                Add one
              </button>
            </div>
          )}
          {chatTools.map((tool) => (
            <ChatToolListItem
              key={tool.id}
              tool={tool}
              selected={selectedChatToolId === tool.id}
              onSelect={() => selectChatTool(tool.id)}
              onDelete={() => deleteChatTool(tool.id)}
            />
          ))}
        </div>
      </div>

      {/* Right content - config/detail */}
      <div className="flex-1 min-w-0 overflow-y-auto">
        {selectedTool ? (
          <ChatToolConfigPanel tool={selectedTool} />
        ) : (
          <div className="flex items-center justify-center h-full text-slate-400 dark:text-gray-500">
            <div className="text-center">
              <Codicon name="comment-discussion" className="text-4xl mb-3 block" />
              <p className="text-sm">
                {chatTools.length > 0
                  ? "Select a chat tool to configure"
                  : "Add a chat tool to get started"}
              </p>
            </div>
          </div>
        )}
      </div>

      {/* Create dialog */}
      <ChatToolCreateDialog
        open={showCreate}
        onClose={() => setShowCreate(false)}
      />
    </div>
  );
}

function ChatToolListItem({
  tool,
  selected,
  onSelect,
  onDelete,
}: {
  tool: ChatTool;
  selected: boolean;
  onSelect: () => void;
  onDelete: () => void;
}) {
  return (
    <div
      onClick={onSelect}
      className={cn(
        "group relative flex items-center gap-3 px-3 py-2.5 rounded-lg cursor-pointer transition-all",
        selected
          ? "bg-primary/10 border border-primary/30"
          : "hover:bg-slate-50 dark:hover:bg-white/5 border border-transparent"
      )}
    >
      <div
        className={cn(
          "size-8 rounded-lg flex items-center justify-center shrink-0",
          selected
            ? "bg-primary text-background-dark"
            : "bg-slate-100 dark:bg-surface-dark text-slate-400 dark:text-gray-500"
        )}
      >
        <Codicon name="comment-discussion" className="text-[16px]" />
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-xs font-semibold text-slate-700 dark:text-gray-200 truncate">
          {tool.name}
        </div>
        <StatusBadge status={tool.status} />
      </div>

      {/* Delete button on hover */}
      <button
        onClick={(e) => {
          e.stopPropagation();
          onDelete();
        }}
        className="absolute top-1 right-1 size-5 rounded-full bg-slate-200 dark:bg-gray-600 text-white flex items-center justify-center opacity-0 group-hover:opacity-100 hover:bg-rose-500 transition-all"
      >
        <Codicon name="close" className="text-[10px]" />
      </button>

      {/* Message stats */}
      {(tool.messages_received > 0 || tool.messages_sent > 0) && (
        <div className="text-[10px] text-slate-400 dark:text-gray-500 shrink-0">
          {tool.messages_received}/{tool.messages_sent}
        </div>
      )}
    </div>
  );
}
