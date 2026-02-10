"use client";

import { useAgentStore } from "@/stores/agentStore";
import { Codicon } from "@/components/ui/Codicon";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { useState, useRef, useEffect } from "react";

const iconOptions = [
  { name: "code", icon: "code" },
  { name: "terminal", icon: "terminal" },
  { name: "psychology", icon: "lightbulb" },
  { name: "rocket_launch", icon: "rocket" },
  { name: "database", icon: "database" },
  { name: "shield", icon: "shield" },
  { name: "auto_awesome", icon: "sparkle" },
  { name: "architecture", icon: "library" },
];

function getAgentIcon(iconName: string) {
  const found = iconOptions.find((o) => o.name === iconName);
  const codiconName = found?.icon ?? "code";
  return <Codicon name={codiconName} className="text-[24px]" />;
}

export function AgentIdentity() {
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const updateAgent = useAgentStore((s) => s.updateAgent);
  const setControlHub = useAgentStore((s) => s.setControlHub);
  const controlHubAgentId = useAgentStore((s) => s.controlHubAgentId);
  const [iconDropdownOpen, setIconDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Inline editing states
  const [editingName, setEditingName] = useState(false);
  const [editingDesc, setEditingDesc] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [draftDesc, setDraftDesc] = useState("");
  const nameInputRef = useRef<HTMLInputElement>(null);
  const descInputRef = useRef<HTMLInputElement>(null);

  const agent = agents.find((a) => a.id === selectedAgentId);

  // Reset editing state when switching agents
  useEffect(() => {
    setEditingName(false);
    setEditingDesc(false);
  }, [selectedAgentId]);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setIconDropdownOpen(false);
      }
    };
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, []);

  // Auto-focus inputs when editing starts
  useEffect(() => {
    if (editingName) nameInputRef.current?.focus();
  }, [editingName]);
  useEffect(() => {
    if (editingDesc) descInputRef.current?.focus();
  }, [editingDesc]);

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

  const startEditName = () => {
    setDraftName(name);
    setEditingName(true);
  };

  const saveName = () => {
    const trimmed = draftName.trim();
    if (agent && trimmed && trimmed !== name) {
      updateAgent(agent.id, { name: trimmed });
    }
    setEditingName(false);
  };

  const startEditDesc = () => {
    setDraftDesc(description);
    setEditingDesc(true);
  };

  const saveDesc = () => {
    const trimmed = draftDesc.trim();
    if (agent && trimmed !== description) {
      updateAgent(agent.id, { description: trimmed });
    }
    setEditingDesc(false);
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
              <Codicon name="edit" className="text-[12px] text-primary" />
            </div>
          </button>
          {iconDropdownOpen && (
            <div className="absolute top-full left-0 mt-2 p-3 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl shadow-2xl z-50 w-48">
              <p className="text-[10px] font-bold text-slate-400 dark:text-gray-500 uppercase tracking-wider mb-3 px-1">
                Choose Identity
              </p>
              <div className="grid grid-cols-4 gap-2">
                {iconOptions.map(({ name: iconName, icon: codiconName }) => (
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
                    <Codicon name={codiconName} className="text-[20px]" />
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-3 mb-1">
            {editingName ? (
              <div className="flex items-center gap-1.5">
                <input
                  ref={nameInputRef}
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                  onBlur={saveName}
                  onKeyDown={(e) => {
                    if (e.nativeEvent.isComposing) return;
                    if (e.key === "Enter") saveName();
                    if (e.key === "Escape") setEditingName(false);
                  }}
                  className="text-2xl font-bold text-slate-900 dark:text-white tracking-tight bg-transparent border-b-2 border-primary outline-none px-0"
                />
                <button onClick={saveName} className="text-primary hover:text-primary/80">
                  <Codicon name="check" />
                </button>
              </div>
            ) : (
              <h2
                onClick={startEditName}
                className="text-2xl font-bold text-slate-900 dark:text-white tracking-tight cursor-pointer hover:text-primary/80 transition-colors"
                title="Click to edit"
              >
                {name}
              </h2>
            )}
            <Badge variant="primary">
              {status === "Running" ? "ACTIVE" : status.toUpperCase()}
            </Badge>
            {isHub && (
              <Badge variant="primary">
                <Codicon name="star-full" className="text-[12px] mr-1" />
                CONTROL HUB
              </Badge>
            )}
          </div>
          {editingDesc ? (
            <div className="flex items-center gap-1.5">
              <input
                ref={descInputRef}
                value={draftDesc}
                onChange={(e) => setDraftDesc(e.target.value)}
                onBlur={saveDesc}
                onKeyDown={(e) => {
                  if (e.nativeEvent.isComposing) return;
                  if (e.key === "Enter") saveDesc();
                  if (e.key === "Escape") setEditingDesc(false);
                }}
                className="text-sm text-slate-500 dark:text-gray-400 bg-transparent border-b border-primary outline-none w-full px-0"
              />
              <button onClick={saveDesc} className="text-primary hover:text-primary/80 shrink-0">
                <Codicon name="check" className="text-[14px]" />
              </button>
            </div>
          ) : (
            <p
              onClick={startEditDesc}
              className="text-sm text-slate-500 dark:text-gray-400 cursor-pointer hover:text-slate-700 dark:hover:text-gray-300 transition-colors"
              title="Click to edit"
            >
              {description}
            </p>
          )}
        </div>
      </div>
      <div className="flex gap-3">
        <Button
          variant={isHub ? "primary" : "secondary"}
          onClick={handleToggleHub}
        >
          <Codicon name="star-full" /> {isHub ? "Hub Active" : "Set as Hub"}
        </Button>
        <Button variant="secondary">
          <Codicon name="history" /> Logs
        </Button>
        <Button variant="primary">
          <Codicon name="play" /> Run Diagnostics
        </Button>
      </div>
    </div>
  );
}
