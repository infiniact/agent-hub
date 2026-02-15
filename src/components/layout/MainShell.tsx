"use client";

import { useState, useRef, useEffect } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { ConfigSection } from "@/components/config/ConfigSection";
import { ChatSection } from "@/components/chat/ChatSection";
import { NavBar } from "@/components/layout/NavBar";
import { KanbanBoard } from "@/components/kanban/KanbanBoard";
import { ChatToolPanel } from "@/components/chat-tools/ChatToolPanel";
import { useAgentStore } from "@/stores/agentStore";

export function MainShell() {
  const [configCollapsed, setConfigCollapsed] = useState(true);
  const [kanbanCollapsed, setKanbanCollapsed] = useState(false);
  const [chatToolsCollapsed, setChatToolsCollapsed] = useState(false);
  const configScrollRef = useRef<HTMLDivElement>(null);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const showKanban = useAgentStore((s) => s.showKanban);
  const showChatTools = useAgentStore((s) => s.showChatTools);

  // Reset config scroll position when switching agents
  useEffect(() => {
    if (configScrollRef.current) {
      configScrollRef.current.scrollTop = 0;
    }
  }, [selectedAgentId]);

  // When switching to kanban, expand it
  useEffect(() => {
    if (showKanban) {
      setKanbanCollapsed(false);
    }
  }, [showKanban]);

  // When switching to chat tools, expand it
  useEffect(() => {
    if (showChatTools) {
      setChatToolsCollapsed(false);
    }
  }, [showChatTools]);

  // The top panel is collapsed when:
  // - chat tools mode: chatToolsCollapsed
  // - kanban mode: kanbanCollapsed
  // - agent mode: configCollapsed
  const topCollapsed = showChatTools ? chatToolsCollapsed : showKanban ? kanbanCollapsed : configCollapsed;

  const toggleTopPanel = () => {
    if (showChatTools) {
      setChatToolsCollapsed(!chatToolsCollapsed);
    } else if (showKanban) {
      setKanbanCollapsed(!kanbanCollapsed);
    } else {
      setConfigCollapsed(!configCollapsed);
    }
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <NavBar />

      {/* Content area — always starts exactly below NavBar */}
      <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
        {/* Top panel: Chat Tools or Kanban or Config */}
        {!topCollapsed && (
          showChatTools ? (
            <div className="flex-1 min-h-0 overflow-hidden">
              <ChatToolPanel />
            </div>
          ) : showKanban ? (
            <div className="flex-1 min-h-0 overflow-hidden">
              <KanbanBoard />
            </div>
          ) : (
            <div
              ref={configScrollRef}
              className="flex-1 min-h-0 overflow-y-auto bg-white dark:bg-background-dark"
            >
              <ConfigSection />
            </div>
          )
        )}
        {/* Divider with toggle */}
        <div className="relative h-px bg-slate-200 dark:bg-border-dark flex items-center justify-center shrink-0">
          <button
            onClick={toggleTopPanel}
            className="absolute z-30 size-8 rounded-full bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all shadow-md"
          >
            {topCollapsed ? (
              <Codicon name="fold-down" className="text-[20px]" />
            ) : (
              <Codicon name="fold-up" className="text-[20px]" />
            )}
          </button>
        </div>
        <div
          className={`${
            topCollapsed ? "flex-1" : "shrink-0 basis-[35%]"
          } min-h-[200px] flex flex-col bg-slate-50 dark:bg-[#07070C] relative shadow-[0_-10px_20px_-10px_rgba(0,0,0,0.15)]`}
        >
          <ChatSection />
        </div>
      </div>
    </div>
  );
}
