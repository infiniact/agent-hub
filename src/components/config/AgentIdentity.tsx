"use client";

import { useAgentStore } from "@/stores/agentStore";
import {
  Code,
  Terminal,
  Brain,
  Shield,
  Rocket,
  Database,
  Sparkles,
  Landmark,
  History,
  Play,
  Pencil,
  Crown,
} from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { useState, useRef, useEffect } from "react";

const iconOptions = [
  { name: "code", Icon: Code },
  { name: "terminal", Icon: Terminal },
  { name: "psychology", Icon: Brain },
  { name: "rocket_launch", Icon: Rocket },
  { name: "database", Icon: Database },
  { name: "shield", Icon: Shield },
  { name: "auto_awesome", Icon: Sparkles },
  { name: "architecture", Icon: Landmark },
];

function getAgentIcon(iconName: string) {
  const found = iconOptions.find((o) => o.name === iconName);
  const Icon = found?.Icon ?? Code;
  return <Icon className="size-6" />;
}

export function AgentIdentity() {
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const updateAgent = useAgentStore((s) => s.updateAgent);
  const setControlHub = useAgentStore((s) => s.setControlHub);
  const controlHubAgentId = useAgentStore((s) => s.controlHubAgentId);
  const [iconDropdownOpen, setIconDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const agent = agents.find((a) => a.id === selectedAgentId);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setIconDropdownOpen(false);
      }
    };
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, []);

  const name = agent?.name ?? "Coder Bot v1.2";
  const description =
    agent?.description ?? "Specialized in TypeScript refactoring and Next.js architecture.";
  const status = agent?.status ?? "Idle";
  const icon = agent?.icon ?? "code";
  const isHub = agent?.id === controlHubAgentId;

  const handleToggleHub = async () => {
    if (!agent) return;
    if (isHub) {
      // Remove hub by updating is_control_hub to false
      await updateAgent(agent.id, { is_control_hub: false });
      useAgentStore.setState({ controlHubAgentId: null });
    } else {
      await setControlHub(agent.id);
    }
  };

  return (
    <div className="flex items-center justify-between px-8 py-5 flex-none">
      <div className="flex items-center gap-4">
        <div className="relative" ref={dropdownRef}>
          <button
            onClick={() => setIconDropdownOpen(!iconDropdownOpen)}
            className="size-12 rounded-lg bg-slate-100 dark:bg-surface-dark border-2 border-slate-200 dark:border-border-dark flex items-center justify-center text-primary hover:border-primary transition-all group overflow-hidden"
          >
            {getAgentIcon(icon)}
            <div className="absolute inset-0 bg-primary/10 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity">
              <Pencil className="size-3 text-primary" />
            </div>
          </button>
          {iconDropdownOpen && (
            <div className="absolute top-full left-0 mt-2 p-3 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl z-50 w-48">
              <p className="text-[10px] font-bold text-slate-400 dark:text-gray-500 uppercase tracking-wider mb-3 px-1">
                Choose Identity
              </p>
              <div className="grid grid-cols-4 gap-2">
                {iconOptions.map(({ name: iconName, Icon }) => (
                  <button
                    key={iconName}
                    className="size-8 rounded flex items-center justify-center hover:bg-primary/20 text-slate-500 dark:text-gray-400 hover:text-primary transition-colors"
                    onClick={() => {
                      if (agent) {
                        updateAgent(agent.id, { icon: iconName });
                      }
                      setIconDropdownOpen(false);
                    }}
                  >
                    <Icon className="size-5" />
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
        <div>
          <div className="flex items-center gap-3 mb-1">
            <h2 className="text-2xl font-bold text-slate-900 dark:text-white tracking-tight">
              {name}
            </h2>
            <Badge variant="primary">
              {status === "Running" ? "ACTIVE" : status.toUpperCase()}
            </Badge>
            {isHub && (
              <Badge variant="primary">
                <Crown className="size-3 mr-1" />
                CONTROL HUB
              </Badge>
            )}
          </div>
          <p className="text-sm text-slate-500 dark:text-gray-400">{description}</p>
        </div>
      </div>
      <div className="flex gap-3">
        <Button
          variant={isHub ? "primary" : "secondary"}
          onClick={handleToggleHub}
        >
          <Crown className="size-4" /> {isHub ? "Hub Active" : "Set as Hub"}
        </Button>
        <Button variant="secondary">
          <History className="size-4" /> Logs
        </Button>
        <Button variant="primary">
          <Play className="size-4" /> Run Diagnostics
        </Button>
      </div>
    </div>
  );
}
