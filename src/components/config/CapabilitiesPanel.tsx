"use client";

import { useState } from "react";
import { Search, X } from "lucide-react";
import { Badge } from "@/components/ui/Badge";

interface Capability {
  id: string;
  name: string;
  type: "mcp" | "skill";
}

const defaultCapabilities: Capability[] = [
  { id: "1", name: "Enterprise Retrieve", type: "mcp" },
  { id: "2", name: "Web Search", type: "skill" },
  { id: "3", name: "Code Refactor", type: "skill" },
  { id: "4", name: "SQL Analytics", type: "mcp" },
];

export function CapabilitiesPanel() {
  const [capabilities, setCapabilities] = useState<Capability[]>(defaultCapabilities);
  const [searchQuery, setSearchQuery] = useState("");

  const removeCapability = (id: string) => {
    setCapabilities((prev) => prev.filter((c) => c.id !== id));
  };

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">
          Capabilities
        </h3>
        <div className="flex gap-2">
          <Badge variant="mcp">MCP</Badge>
          <Badge variant="skill">SKILL</Badge>
        </div>
      </div>
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 dark:text-gray-500 size-5" />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full h-11 pl-10 pr-4 bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg text-sm text-slate-900 dark:text-white focus:ring-1 focus:ring-primary focus:border-primary outline-none transition-all placeholder:text-slate-400 dark:placeholder:text-gray-600"
          placeholder="Search and add capabilities..."
        />
      </div>
      <div className="flex flex-wrap gap-2 min-h-[100px] p-4 rounded-xl border border-dashed border-slate-200 dark:border-border-dark bg-slate-50/50 dark:bg-surface-dark/50">
        {capabilities.map((cap) => (
          <span
            key={cap.id}
            className={`px-2 py-1 rounded text-[10px] font-bold uppercase tracking-wider transition-all cursor-default flex items-center gap-1.5 ${
              cap.type === "mcp"
                ? "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30"
                : "bg-purple-500/10 text-purple-400 border border-purple-500/30"
            }`}
          >
            {cap.name}
            <button onClick={() => removeCapability(cap.id)} className="hover:opacity-70">
              <X className="size-3" />
            </button>
          </span>
        ))}
      </div>
    </div>
  );
}
