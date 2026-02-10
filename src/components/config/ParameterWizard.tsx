"use client";

import { useState, useEffect } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { Select } from "@/components/ui/Select";
import { Slider } from "@/components/ui/Slider";
import { useAgentStore } from "@/stores/agentStore";
import { useAcpStore } from "@/stores/acpStore";

/** Extract the first URL from a string, if any. */
function extractUrl(text: string): string | null {
  const match = text.match(/https?:\/\/[^\s)'"]+/);
  return match ? match[0] : null;
}

/** Check if an error message indicates authentication is required. */
function isAuthError(text: string): boolean {
  const lower = text.toLowerCase();
  return (
    lower.includes("authentication required") ||
    lower.includes("login") ||
    lower.includes("api_key") ||
    lower.includes("api key") ||
    lower.includes("authenticate")
  );
}

export function ParameterWizard() {
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const updateAgent = useAgentStore((s) => s.updateAgent);
  const ensureAgentReady = useAgentStore((s) => s.ensureAgentReady);
  const refreshModels = useAgentStore((s) => s.refreshModels);
  const agentInitializing = useAgentStore((s) => s.agentInitializing);
  const agentError = useAgentStore((s) => s.agentError);

  const discoveredAgents = useAcpStore((s) => s.discoveredAgents);
  const scanning = useAcpStore((s) => s.scanning);

  const agent = agents.find((a) => a.id === selectedAgentId);
  const model = agent?.model ?? "";
  const maxConcurrency = agent?.max_concurrency ?? 1;

  // Manual model input state
  const [manualModel, setManualModel] = useState("");

  // Reset manual model when switching agents
  useEffect(() => {
    setManualModel("");
  }, [selectedAgentId]);

  // Build model options from the discovered agent (live data only, no fallback)
  const currentDiscovered = discoveredAgents.find(
    (da) => da.command === agent?.acp_command
  );
  const modelList = currentDiscovered?.models ?? [];
  const modelOptions = modelList.map((m) => ({ label: m, value: m }));

  // Agent is connected but returned no model list
  const agentConnectedNoModels =
    !agentInitializing && !agentError && !!agent?.acp_command && modelOptions.length === 0;

  const handleManualModelSave = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed || !agent) return;
    // Save model to agent config and cache
    const models = [trimmed];
    updateAgent(agent.id, {
      model: trimmed,
      available_models_json: JSON.stringify(models),
    });
    // Also update discovered agents store so the dropdown appears on next render
    if (agent.acp_command) {
      useAcpStore.getState().updateDiscoveredAgentModels(agent.acp_command, models);
    }
    setManualModel("");
  };

  const agentOptions = discoveredAgents.map((da) => ({
    label: da.available ? da.name : `${da.name} (not installed)`,
    value: da.command,
    disabled: !da.available,
  }));

  const handleIdentityChange = (command: string) => {
    if (!agent) return;
    const matched = discoveredAgents.find((da) => da.command === command);
    if (!matched || !matched.available) return;

    // Immediately clear models for both old and new agent commands
    const acpStore = useAcpStore.getState();
    if (agent.acp_command) {
      acpStore.updateDiscoveredAgentModels(agent.acp_command, []);
    }
    acpStore.updateDiscoveredAgentModels(command, []);

    // Optimistically update local agent state so the UI reflects
    // the new identity immediately (don't wait for backend round-trip)
    useAgentStore.setState((s) => ({
      agents: s.agents.map((a) =>
        a.id === agent.id
          ? { ...a, name: matched.name, acp_command: matched.command, model: '', available_models_json: '[]' }
          : a
      ),
    }));

    updateAgent(agent.id, {
      name: matched.name,
      acp_command: matched.command,
      acp_args_json: matched.args_json,
      model: '',                        // Clear stale model
      available_models_json: '[]',      // Clear cached models
    }).then(() => {
      // Clear ready state so ensureAgentReady runs fresh for the new identity
      useAgentStore.setState((s) => ({
        readyAgentIds: s.readyAgentIds.filter((id) => id !== agent.id),
      }));
      // Spawn + initialize + fetch models; auto-selects first model when done
      ensureAgentReady(agent.id);
    });
  };

  return (
    <div className="space-y-4">
      <h3 className="text-sm font-bold text-slate-400 dark:text-gray-500 uppercase tracking-widest">
        Parameter Wizard
      </h3>
      <div className="bg-slate-50 dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-xl p-5">
        <div className="flex flex-col sm:flex-row gap-6">
          {/* Choose Agent */}
          <div className="flex-1 space-y-2">
            <div className="flex items-center justify-between h-7">
              <label className="flex items-center gap-2 text-xs font-bold text-slate-500 dark:text-gray-400 uppercase tracking-wide">
                <Codicon name="hubot" />
                Choose Agent
              </label>
            </div>
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
                className="w-full"
              />
            )}
          </div>

          {/* LLM Model */}
          <div className="flex-1 space-y-2">
            <div className="flex items-center justify-between h-7">
              <label className="flex items-center gap-2 text-xs font-bold text-slate-500 dark:text-gray-400 uppercase tracking-wide">
                <Codicon name="server-process" />
                LLM Model
              </label>
              {agent?.acp_command && !agentInitializing && (
                <button
                  type="button"
                  onClick={() => agent && refreshModels(agent.id)}
                  className="p-1 rounded hover:bg-slate-200 dark:hover:bg-gray-700 text-slate-400 dark:text-gray-500 hover:text-slate-600 dark:hover:text-gray-300 transition-colors"
                  title="Refresh models"
                >
                  <Codicon name="refresh" className="text-[14px]" />
                </button>
              )}
            </div>
            {agentInitializing ? (
              <span className="flex items-center gap-2 text-xs text-slate-400">
                <Codicon name="loading" className="text-[12px] codicon-modifier-spin" />
                Loading models from agent...
              </span>
            ) : agentConnectedNoModels ? (
              <div className="space-y-1">
                <input
                  type="text"
                  value={manualModel || model}
                  onChange={(e) => setManualModel(e.target.value)}
                  onBlur={(e) => handleManualModelSave(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.nativeEvent.isComposing) return;
                    if (e.key === "Enter") handleManualModelSave((e.target as HTMLInputElement).value);
                  }}
                  placeholder="Enter model ID (e.g. gemini-2.5-pro)"
                  className="w-full h-9 px-3 text-sm rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-slate-900 dark:text-gray-100 placeholder:text-slate-400 dark:placeholder:text-gray-600 focus:outline-none focus:ring-1 focus:ring-primary"
                />
                <span className="text-[10px] text-slate-400 dark:text-gray-500">
                  Agent does not report models. Enter model ID manually.
                </span>
              </div>
            ) : modelOptions.length === 0 ? (
              <span className="text-xs text-slate-400">
                Select an agent to load available models
              </span>
            ) : (
              <Select
                value={model}
                options={modelOptions}
                onChange={(v) => agent && updateAgent(agent.id, { model: v })}
                className="w-full"
              />
            )}
            {agentError && !agentInitializing && (
              <div className="flex items-start gap-1.5 text-xs text-red-500 dark:text-red-400 mt-1">
                <Codicon name="warning" className="text-[14px] mt-0.5 shrink-0" />
                <div className="min-w-0">
                  <span className="break-words">{agentError}</span>
                  {isAuthError(agentError) && (() => {
                    const url = extractUrl(agentError);
                    return url ? (
                      <a
                        href={url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="flex items-center gap-1 mt-1 text-blue-500 hover:text-blue-400 underline"
                      >
                        <Codicon name="link-external" className="text-[12px]" />
                        Open login page
                      </a>
                    ) : null;
                  })()}
                </div>
              </div>
            )}
          </div>

          {/* Max Concurrency */}
          <div className="flex-1 space-y-2">
            <div className="flex items-center justify-between h-7">
              <label className="flex items-center gap-2 text-xs font-bold text-slate-500 dark:text-gray-400 uppercase tracking-wide">
                <Codicon name="organization" />
                Max Concurrency
              </label>
              <span className="text-xs font-mono text-primary">{maxConcurrency}</span>
            </div>
            <div className="h-9 flex items-center">
              <Slider
                value={maxConcurrency}
                min={1}
                max={10}
                step={1}
                onChange={(v) =>
                  agent && updateAgent(agent.id, { max_concurrency: v })
                }
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
