"use client";

import { useAgentStore } from "@/stores/agentStore";
import { useAcpStore } from "@/stores/acpStore";
import {
  Code,
  Search,
  Terminal,
  Brain,
  Shield,
  Plus,
  Rocket,
  Database,
  Sparkles,
  Landmark,
  Crown,
} from "lucide-react";
import { cn } from "@/lib/cn";
import { useEffect, useState } from "react";
import { isTauri } from "@/lib/tauri";

const iconMap: Record<string, React.ReactNode> = {
  code: <Code className="size-5" />,
  manage_search: <Search className="size-5" />,
  terminal: <Terminal className="size-5" />,
  psychology: <Brain className="size-5" />,
  shield: <Shield className="size-5" />,
  rocket_launch: <Rocket className="size-5" />,
  database: <Database className="size-5" />,
  auto_awesome: <Sparkles className="size-5" />,
  architecture: <Landmark className="size-5" />,
};

export function NavBar() {
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const selectAgent = useAgentStore((s) => s.selectAgent);
  const fetchAgents = useAgentStore((s) => s.fetchAgents);
  const createAgent = useAgentStore((s) => s.createAgent);
  const updateAgent = useAgentStore((s) => s.updateAgent);
  const setControlHub = useAgentStore((s) => s.setControlHub);
  const controlHubAgentId = useAgentStore((s) => s.controlHubAgentId);
  const discoveredAgents = useAcpStore((s) => s.discoveredAgents);
  const scanForAgents = useAcpStore((s) => s.scanForAgents);
  const [initialized, setInitialized] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ agentId: string; x: number; y: number } | null>(null);

  useEffect(() => {
    if (!isTauri() || initialized) return;
    console.log('[NavBar] Initializing...');

    const init = async () => {
      // Scan for available agents first
      await scanForAgents();
      await fetchAgents();
      setInitialized(true);

      // Use a timeout to ensure stores are updated
      setTimeout(async () => {
        const currentAgents = useAgentStore.getState().agents;
        const currentDiscovered = useAcpStore.getState().discoveredAgents;
        const currentSelectedId = useAgentStore.getState().selectedAgentId;

        console.log('[NavBar] After fetch, agents:', currentAgents.length, 'discovered:', currentDiscovered.length);

        // Find the best discovered agent (prefer Built-in ACP, only available ones)
        const availableAgents = currentDiscovered.filter((d) => d.available);
        const builtinAdapter = availableAgents.find((d) => d.name.includes('Claude Code'));
        const bestDiscovered = builtinAdapter || availableAgents[0];

        // Update existing agents that use outdated commands (npx/pnpx) to use built-in adapter
        if (bestDiscovered && currentAgents.length > 0) {
          for (const agent of currentAgents) {
            const usesOldCommand = agent.acp_command && (
              agent.acp_command.includes('npx') ||
              agent.acp_command.includes('pnpx') ||
              (agent.acp_command.includes('claude') && !agent.acp_command.includes('node'))
            );
            if (usesOldCommand) {
              console.log('[NavBar] Updating agent to use built-in adapter:', agent.id, agent.acp_command, '->', bestDiscovered.command);
              try {
                await updateAgent(agent.id, {
                  acp_command: bestDiscovered.command,
                  acp_args_json: bestDiscovered.args_json,
                });
              } catch (e) {
                console.error('[NavBar] Failed to update agent:', e);
              }
            }
          }
        }

        if (currentAgents.length === 0 && availableAgents.length > 0) {
          const discovered = bestDiscovered!;
          console.log('[NavBar] Creating agent from discovered:', discovered.name);

          createAgent({
            name: discovered.name,
            icon: 'code',
            description: `Auto-discovered agent from ${discovered.source_path}`,
            execution_mode: 'RunNow',
            model: 'gpt-4',
            temperature: 0.7,
            max_tokens: 4096,
            system_prompt: 'You are a helpful AI assistant.',
            capabilities_json: '[]',
            acp_command: discovered.command,
            acp_args_json: discovered.args_json,
          }).then((defaultAgent) => {
            console.log('[NavBar] Agent created from discovered:', defaultAgent.id);
            selectAgent(defaultAgent.id);
          }).catch((e) => {
            console.error('[NavBar] Failed to create agent:', e);
          });
        } else if (currentAgents.length === 0) {
          // No agents at all, create a default one (will need manual configuration)
          console.log('[NavBar] No agents found, creating default agent...');
          createAgent({
            name: 'Default Agent',
            icon: 'code',
            description: 'A general-purpose AI agent. Configure the ACP command to enable auto-start.',
            execution_mode: 'RunNow',
            model: 'gpt-4',
            temperature: 0.7,
            max_tokens: 4096,
            system_prompt: 'You are a helpful coding assistant.',
            capabilities_json: '[]',
            acp_command: undefined,
            acp_args_json: undefined,
          }).then((defaultAgent) => {
            console.log('[NavBar] Default agent created:', defaultAgent.id);
            selectAgent(defaultAgent.id);
          }).catch((e) => {
            console.error('[NavBar] Failed to create default agent:', e);
          });
        } else if (!currentSelectedId) {
          // Auto-select first agent
          console.log('[NavBar] Auto-selecting first agent:', currentAgents[0]?.id);
          if (currentAgents[0]) {
            selectAgent(currentAgents[0].id);
          }
        }
      }, 100);
    };

    init();
  }, [initialized]); // Only run on mount

  // Close context menu on click outside
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [contextMenu]);

  const handleContextMenu = (e: React.MouseEvent, agentId: string) => {
    e.preventDefault();
    setContextMenu({ agentId, x: e.clientX, y: e.clientY });
  };

  return (
    <div className="flex-none h-12 bg-white dark:bg-[#07070C] border-b border-slate-200 dark:border-border-dark flex items-center px-6 justify-center z-40 overflow-visible">
      <div className="flex items-center gap-3">
        {agents.map((agent) => (
          <div
            key={agent.id}
            className="relative group cursor-pointer"
            onClick={() => selectAgent(agent.id)}
            onContextMenu={(e) => handleContextMenu(e, agent.id)}
          >
            <div
              className={cn(
                "size-8 rounded-lg flex items-center justify-center transition-all transform hover:scale-105",
                selectedAgentId === agent.id
                  ? "bg-primary text-background-dark shadow-[0_0_10px_rgba(0,229,255,0.3)]"
                  : "bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 text-slate-400 dark:text-gray-400 hover:text-primary"
              )}
            >
              {iconMap[agent.icon] ?? <Code className="size-5" />}
            </div>
            {agent.status === "Running" && (
              <div className="status-dot absolute -top-0.5 -right-0.5 size-2 rounded-full border border-background-dark bg-primary" />
            )}
            {/* Crown icon for Control Hub */}
            {agent.is_control_hub && (
              <div className="absolute -top-1.5 -left-1.5">
                <Crown className="size-3 text-amber-400 fill-amber-400" />
              </div>
            )}
          </div>
        ))}
        {agents.length === 0 && (
          <div className="relative group cursor-pointer">
            <div className="size-8 rounded-lg bg-primary text-background-dark flex items-center justify-center shadow-[0_0_10px_rgba(0,229,255,0.3)]">
              <Code className="size-5" />
            </div>
            <div className="status-dot absolute -top-0.5 -right-0.5 size-2 rounded-full border border-background-dark bg-primary" />
          </div>
        )}
        <div className="w-px h-5 bg-slate-200 dark:bg-border-dark/60 mx-1" />
        <button
          onClick={async () => {
            try {
              const discovered = useAcpStore.getState().discoveredAgents;
              const available = discovered.filter((d) => d.available);
              const best = available.find((d) => d.name.includes('Claude Code')) || available[0];

              const newAgent = await createAgent({
                name: best ? best.name : `Agent ${agents.length + 1}`,
                icon: 'code',
                description: best
                  ? `Agent using ${best.name}`
                  : 'New AI agent. Configure the ACP command to enable.',
                execution_mode: 'RunNow',
                model: 'gpt-4',
                temperature: 0.7,
                max_tokens: 4096,
                system_prompt: 'You are a helpful AI assistant.',
                capabilities_json: '[]',
                acp_command: best?.command,
                acp_args_json: best?.args_json,
              });
              await selectAgent(newAgent.id);
            } catch (e) {
              console.error('[NavBar] Failed to add agent:', e);
            }
          }}
          className="size-8 rounded-lg border border-slate-300 dark:border-border-dark bg-white dark:bg-surface-dark hover:border-primary hover:text-primary text-slate-400 dark:text-gray-500 flex items-center justify-center transition-colors"
        >
          <Plus className="size-4" />
        </button>
      </div>

      {/* Context menu */}
      {contextMenu && (
        <div
          className="fixed z-50 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg shadow-2xl py-1 min-w-[160px]"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            onClick={() => {
              setControlHub(contextMenu.agentId);
              setContextMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 text-xs hover:bg-slate-100 dark:hover:bg-white/5 flex items-center gap-2"
          >
            <Crown className="size-3 text-amber-400" />
            {controlHubAgentId === contextMenu.agentId ? "Remove Control Hub" : "Set as Control Hub"}
          </button>
        </div>
      )}
    </div>
  );
}
