"use client";

import { useState } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";

const modes = [
  {
    id: "RunNow",
    label: "Run Now",
    description: "Manual execution on demand.",
    icon: "play",
    color: "bg-primary/10 text-primary",
  },
  {
    id: "Schedule",
    label: "Schedule",
    description: "Execute at specific times.",
    icon: "calendar",
    color: "bg-emerald-500/10 text-emerald-400",
  },
  {
    id: "Automate",
    label: "Automate",
    description: "Loop based on triggers.",
    icon: "refresh",
    color: "bg-purple-500/10 text-purple-400",
  },
];

export function ExecutionMode() {
  const [active, setActive] = useState("RunNow");

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">
        Execution Mode
      </h3>
      <div className="grid grid-cols-3 gap-4">
        {modes.map(({ id, label, description, icon, color }) => (
          <div
            key={id}
            onClick={() => setActive(id)}
            className={cn(
              "execution-card group cursor-pointer p-4 rounded-xl border border-slate-200 dark:border-border-dark bg-slate-50 dark:bg-surface-dark hover:border-primary/50 transition-all flex flex-col gap-3",
              active === id && "active"
            )}
          >
            <div className={cn("size-10 rounded-lg flex items-center justify-center", color)}>
              <Codicon name={icon} className="text-[20px]" />
            </div>
            <div>
              <p className="font-bold text-sm text-slate-900 dark:text-white">{label}</p>
              <p className="text-[11px] text-slate-500 dark:text-gray-400 mt-1">{description}</p>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
