"use client";

import { useState } from "react";
import { ChevronsUp, ChevronsDown } from "lucide-react";
import { ConfigSection } from "@/components/config/ConfigSection";
import { ChatSection } from "@/components/chat/ChatSection";
import { NavBar } from "@/components/layout/NavBar";

export function MainShell() {
  const [configCollapsed, setConfigCollapsed] = useState(true);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* NavBar: outside overflow-hidden content area, always fully visible */}
      <NavBar />

      {/* Content area below NavBar */}
      <div className="flex-1 flex flex-col min-h-0 overflow-hidden">
        {!configCollapsed && (
          <div className="flex-1 flex flex-col min-h-0 overflow-y-auto">
            <ConfigSection />
          </div>
        )}
        {/* Divider with toggle */}
        <div className="relative h-px bg-slate-200 dark:bg-border-dark flex items-center justify-center flex-none">
          <button
            onClick={() => setConfigCollapsed(!configCollapsed)}
            className="absolute z-30 size-8 rounded-full bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark flex items-center justify-center text-slate-400 dark:text-gray-400 hover:text-primary transition-all shadow-md"
          >
            {configCollapsed ? (
              <ChevronsDown className="size-5" />
            ) : (
              <ChevronsUp className="size-5" />
            )}
          </button>
        </div>
        <div
          className={`${
            configCollapsed ? "flex-1" : "h-[35%]"
          } min-h-[200px] flex flex-col bg-slate-50 dark:bg-[#07070C] relative shadow-[0_-10px_20px_-10px_rgba(0,0,0,0.15)]`}
        >
          <ChatSection />
        </div>
      </div>
    </div>
  );
}
