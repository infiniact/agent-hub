"use client";

import { useState } from "react";
import { User, Cpu, Thermometer, Hash } from "lucide-react";
import { Select } from "@/components/ui/Select";
import { Slider } from "@/components/ui/Slider";
import { cn } from "@/lib/cn";
import { useAgentStore } from "@/stores/agentStore";
import { useAcpStore } from "@/stores/acpStore";

const steps = [
  { id: 1, label: "IDENTITY", Icon: User },
  { id: 2, label: "MODEL", Icon: Cpu },
  { id: 3, label: "TEMP", Icon: Thermometer },
  { id: 4, label: "LIMIT", Icon: Hash },
];

const modelOptions = [
  { label: "GPT-4 Turbo", value: "gpt-4-turbo" },
  { label: "Claude 3.5 Sonnet", value: "claude-3.5-sonnet" },
  { label: "Llama 3 70B", value: "llama-3-70b" },
];

export function ParameterWizard() {
  const [activeStep, setActiveStep] = useState(1);
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const updateAgent = useAgentStore((s) => s.updateAgent);

  const discoveredAgents = useAcpStore((s) => s.discoveredAgents);
  const scanning = useAcpStore((s) => s.scanning);

  const agent = agents.find((a) => a.id === selectedAgentId);
  const model = agent?.model ?? "gpt-4-turbo";
  const temperature = agent?.temperature ?? 0.7;
  const maxTokens = agent?.max_tokens ?? 4096;

  const agentOptions = discoveredAgents.map((da) => ({
    label: da.available ? da.name : `${da.name} (not installed)`,
    value: da.command,
    disabled: !da.available,
  }));

  const handleIdentityChange = (command: string) => {
    if (!agent) return;
    const matched = discoveredAgents.find((da) => da.command === command);
    if (!matched || !matched.available) return;
    updateAgent(agent.id, {
      name: matched.name,
      acp_command: matched.command,
      acp_args_json: matched.args_json,
    });
  };

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">
        Parameter Wizard
      </h3>
      <div className="bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl p-4">
        <div className="flex flex-col lg:flex-row gap-6 items-center">
          {/* Step tabs */}
          <div className="flex flex-none gap-1 bg-white dark:bg-background-dark p-1 rounded-lg border border-slate-200 dark:border-border-dark">
            {steps.map(({ id, label, Icon }) => (
              <button
                key={id}
                onClick={() => setActiveStep(id)}
                className={cn(
                  "wizard-step flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-bold transition-all border border-transparent",
                  activeStep === id ? "active" : "text-slate-500"
                )}
              >
                <Icon className="size-4" />
                {label}
              </button>
            ))}
          </div>
          {/* Step content */}
          <div className="flex-1 w-full">
            {activeStep === 1 && (
              <div className="flex items-center gap-4">
                <span className="text-xs font-medium text-slate-400 whitespace-nowrap">
                  Choose Identity:
                </span>
                {scanning ? (
                  <span className="text-xs text-slate-400 animate-pulse">
                    Scanning for agents...
                  </span>
                ) : agentOptions.length === 0 ? (
                  <span className="text-xs text-slate-400">
                    No agents detected — install an ACP-compatible agent
                  </span>
                ) : (
                  <Select
                    value={agent?.acp_command ?? agentOptions[0]?.value ?? ""}
                    options={agentOptions}
                    onChange={handleIdentityChange}
                    className="w-full max-w-sm"
                  />
                )}
              </div>
            )}
            {activeStep === 2 && (
              <div className="flex items-center gap-4">
                <span className="text-xs font-medium text-slate-400 whitespace-nowrap">
                  LLM Model:
                </span>
                <Select
                  value={model}
                  options={modelOptions}
                  onChange={(v) => agent && updateAgent(agent.id, { model: v })}
                  className="w-full max-w-sm"
                />
              </div>
            )}
            {activeStep === 3 && (
              <div className="flex items-center gap-6">
                <Slider
                  value={temperature}
                  min={0}
                  max={1}
                  step={0.1}
                  label="Temperature"
                  onChange={(v) => agent && updateAgent(agent.id, { temperature: v })}
                />
              </div>
            )}
            {activeStep === 4 && (
              <div className="flex items-center gap-6">
                <Slider
                  value={maxTokens}
                  min={256}
                  max={8192}
                  step={256}
                  label="Max Tokens"
                  onChange={(v) => agent && updateAgent(agent.id, { max_tokens: v })}
                />
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
