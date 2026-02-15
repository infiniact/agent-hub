"use client";

import { useAgentStore } from "@/stores/agentStore";
import { useAcpStore } from "@/stores/acpStore";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { Codicon } from "@/components/ui/Codicon";
import { McpIcon } from "@/components/icons/McpIcon";
import { cn } from "@/lib/cn";
import { useEffect, useState, useMemo } from "react";
import { isTauri } from "@/lib/tauri";

const iconMap: Record<string, string> = {
  code: "code",
  manage_search: "search",
  terminal: "terminal",
  psychology: "lightbulb",
  shield: "shield",
  rocket_launch: "rocket",
  database: "database",
  auto_awesome: "sparkle",
  architecture: "library",
};

export function NavBar() {
  const agents = useAgentStore((s) => s.agents);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const selectAgent = useAgentStore((s) => s.selectAgent);
  const fetchAgents = useAgentStore((s) => s.fetchAgents);
  const createAgent = useAgentStore((s) => s.createAgent);
  const updateAgent = useAgentStore((s) => s.updateAgent);
  const setControlHub = useAgentStore((s) => s.setControlHub);
  const deleteAgent = useAgentStore((s) => s.deleteAgent);
  const controlHubAgentId = useAgentStore((s) => s.controlHubAgentId);
  const showKanban = useAgentStore((s) => s.showKanban);
  const setShowKanban = useAgentStore((s) => s.setShowKanban);
  const discoveredAgents = useAcpStore((s) => s.discoveredAgents);
  const scanForAgents = useAcpStore((s) => s.scanForAgents);

  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const selectWorkspaceDirectory = useWorkspaceStore((s) => s.selectWorkspaceDirectory);
  const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId);
  const workingDirectory = activeWorkspace?.working_directory || '';
  const folderName = workingDirectory ? workingDirectory.split('/').pop() : null;

  // Get task runs to show status dots for agents with scheduled/running tasks
  const taskRuns = useOrchestrationStore((s) => s.taskRuns);
  const fetchTaskRuns = useOrchestrationStore((s) => s.fetchTaskRuns);
  const clearViewingTaskRun = useOrchestrationStore((s) => s.clearViewingTaskRun);

  const [initialized, setInitialized] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ agentId: string; x: number; y: number } | null>(null);

  // Compute agent task status dots: only show currently running tasks
  const agentTaskDots = useMemo(() => {
    const dotsMap: Record<string, Array<'running' | 'scheduled'>> = {};

    for (const task of taskRuns) {
      const agentId = task.control_hub_agent_id;
      if (!dotsMap[agentId]) {
        dotsMap[agentId] = [];
      }
      if (dotsMap[agentId].length >= 5) continue;

      // Scheduled tasks pending execution
      if (task.schedule_type !== "none" && task.next_run_at && !task.is_paused) {
        const nextRun = new Date(task.next_run_at);
        if (nextRun > new Date()) {
          dotsMap[agentId].push('scheduled');
          continue;
        }
      }

      // Only running/in-progress tasks
      if (task.status === "running" || task.status === "analyzing" || task.status === "awaiting_confirmation" || task.status === "pending") {
        dotsMap[agentId].push('running');
      }
    }

    return dotsMap;
  }, [taskRuns]);

  // Fetch task runs on mount and when workspace changes
  useEffect(() => {
    if (isTauri() && activeWorkspaceId) {
      fetchTaskRuns();
    }
  }, [fetchTaskRuns, activeWorkspaceId]);

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
          const wsId = useWorkspaceStore.getState().activeWorkspaceId;

          createAgent({
            name: discovered.name,
            icon: 'code',
            description: `Auto-discovered agent from ${discovered.source_path}`,
            execution_mode: 'RunNow',
            model: discovered.models?.[0] ?? '',
            temperature: 0.7,
            max_tokens: 4096,
            system_prompt: 'You are a helpful AI assistant.',
            capabilities_json: '[]',
            acp_command: discovered.command,
            acp_args_json: discovered.args_json,
            workspace_id: wsId ?? undefined,
          }).then((defaultAgent) => {
            console.log('[NavBar] Agent created from discovered:', defaultAgent.id);
            selectAgent(defaultAgent.id);
          }).catch((e) => {
            console.error('[NavBar] Failed to create agent:', e);
          });
        } else if (currentAgents.length === 0) {
          // No agents at all, create a default one (will need manual configuration)
          console.log('[NavBar] No agents found, creating default agent...');
          const wsId = useWorkspaceStore.getState().activeWorkspaceId;
          createAgent({
            name: 'Default Agent',
            icon: 'code',
            description: 'A general-purpose AI agent. Configure the ACP command to enable auto-start.',
            execution_mode: 'RunNow',
            model: '',
            temperature: 0.7,
            max_tokens: 4096,
            system_prompt: 'You are a helpful coding assistant.',
            capabilities_json: '[]',
            acp_command: undefined,
            acp_args_json: undefined,
            workspace_id: wsId ?? undefined,
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
    <div className="h-12 shrink-0 bg-white dark:bg-[#07070C] border-b border-slate-200 dark:border-border-dark flex items-center px-4 justify-between">
      {/* Left: Session management + workspace directory */}
      <div className="flex items-center gap-2">
        <button
          className="size-8 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-primary transition-colors"
          title="Sessions"
        >
          <Codicon name="comment-discussion" />
        </button>
        {/* Workspace directory */}
        {activeWorkspace && (
          <>
            <div className="w-px h-5 bg-slate-200 dark:bg-border-dark/60" />
            <button
              className="flex items-center gap-1.5 px-2 py-1 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 text-slate-400 dark:text-gray-500 hover:text-primary transition-colors text-xs"
              onClick={() => activeWorkspaceId && selectWorkspaceDirectory(activeWorkspaceId)}
              title={workingDirectory ? `Workspace: ${workingDirectory}` : "Select workspace folder"}
            >
              <Codicon name="folder-opened" className="text-[14px] leading-none" />
              {folderName && (
                <span className="max-w-[120px] truncate">{folderName}</span>
              )}
              {!workingDirectory && (
                <div className="size-2 rounded-full bg-amber-500 shadow-[0_0_4px_rgba(245,158,11,0.4)]" />
              )}
            </button>
          </>
        )}
      </div>

      {/* Center: Agent tabs */}
      <div className="flex items-center gap-3">
        {/* Kanban button with divider */}
        <button
          onClick={() => {
            setShowKanban(true);
            // Clear selected agent when showing kanban
            selectAgent(null as any);
          }}
          className={cn(
            "size-8 rounded-lg flex items-center justify-center transition-all",
            showKanban
              ? "bg-primary text-background-dark shadow-[0_0_10px_rgba(0,229,255,0.3)]"
              : "bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 text-slate-400 dark:text-gray-400 hover:text-primary"
          )}
          title="Session Kanban"
        >
          <Codicon name="kanban" className="text-[20px]" />
        </button>
        <div className="w-px h-5 bg-slate-200 dark:border-border-dark/60 mx-1" />
        {agents.map((agent) => {
          const dots = agentTaskDots[agent.id] || [];

          return (
            <div
              key={agent.id}
              className="relative group cursor-pointer"
              onClick={() => {
                selectAgent(agent.id);
                setShowKanban(false);
                clearViewingTaskRun();
              }}
              onContextMenu={(e) => handleContextMenu(e, agent.id)}
              title={agent.name}
            >
              <div
                className={cn(
                  "size-8 rounded-lg flex items-center justify-center transition-all transform hover:scale-105",
                  !agent.is_enabled && "opacity-30 grayscale",
                  !showKanban && selectedAgentId === agent.id
                    ? "bg-primary text-background-dark shadow-[0_0_10px_rgba(0,229,255,0.3)]"
                    : "bg-white dark:bg-surface-dark border border-slate-200 dark:border-white/5 hover:border-primary/50 text-slate-400 dark:text-gray-400 hover:text-primary"
                )}
              >
                <Codicon name={iconMap[agent.icon] ?? "code"} className="text-[20px]" />
              </div>
              {/* Delete button on hover */}
              {agents.length > 1 && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    const id = agent.id;
                    deleteAgent(id).then(() => {
                      const remaining = useAgentStore.getState().agents;
                      if (useAgentStore.getState().selectedAgentId === null && remaining.length > 0) {
                        selectAgent(remaining[0].id);
                      }
                    }).catch((err) => console.error('[NavBar] delete failed:', err));
                  }}
                  className="absolute -top-1.5 -right-1.5 size-4 rounded-full bg-slate-300 dark:bg-gray-600 text-white flex items-center justify-center opacity-0 group-hover:opacity-100 hover:bg-rose-500 transition-all"
                >
                  <Codicon name="close" className="text-[10px]" />
                </button>
              )}
              {/* Task status dots */}
              {dots.length > 0 && (
                <div className="absolute -top-1 left-1/2 -translate-x-1/2 flex gap-[2px]">
                  {dots.map((status, i) => (
                    <div
                      key={i}
                      className={cn(
                        "size-[5px] rounded-full",
                        status === "running" && "bg-blue-500 animate-pulse",
                        status === "scheduled" && "bg-amber-400"
                      )}
                    />
                  ))}
                </div>
              )}
              {/* Crown icon for Control Hub */}
              {agent.is_control_hub && (
                <div className="absolute -top-1.5 -left-1.5">
                  <Codicon name="star-full" className="text-[12px] text-amber-400" />
                </div>
              )}
              {/* Disabled indicator */}
              {!agent.is_enabled && (
                <div className="absolute -bottom-1 -right-1">
                  <Codicon name="debug-pause" className="text-[10px] text-rose-400" />
                </div>
              )}
            </div>
          );
        })}
        {agents.length === 0 && (
          <div className="relative group cursor-pointer">
            <div className="size-8 rounded-lg bg-primary text-background-dark flex items-center justify-center shadow-[0_0_10px_rgba(0,229,255,0.3)]">
              <Codicon name="code" className="text-[20px]" />
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
                model: best?.models?.[0] ?? '',
                temperature: 0.7,
                max_tokens: 4096,
                system_prompt: 'You are a helpful AI assistant.',
                capabilities_json: '[]',
                acp_command: best?.command,
                acp_args_json: best?.args_json,
                workspace_id: activeWorkspaceId ?? undefined,
              });

              await selectAgent(newAgent.id);
            } catch (e) {
              console.error('[NavBar] Failed to add agent:', e);
            }
          }}
          className="size-8 rounded-lg border border-slate-300 dark:border-border-dark bg-white dark:bg-surface-dark hover:border-primary hover:text-primary text-slate-400 dark:text-gray-500 flex items-center justify-center transition-colors"
        >
          <Codicon name="add" />
        </button>
      </div>

      {/* Right: MCP + Skill management */}
      <div className="flex items-center gap-1">
        <button
          className="size-8 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-primary transition-colors"
          title="MCP Servers"
        >
          <McpIcon className="size-4" />
        </button>
        <button
          className="size-8 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 flex items-center justify-center text-slate-400 dark:text-gray-500 hover:text-primary transition-colors"
          title="Skills"
        >
          <Codicon name="zap" />
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
            <Codicon name="star-full" className="text-[12px] text-amber-400" />
            {controlHubAgentId === contextMenu.agentId ? "Remove Control Hub" : "Set as Control Hub"}
          </button>
          {agents.length > 1 && (
            <button
              onClick={async () => {
                const id = contextMenu.agentId;
                setContextMenu(null);
                try {
                  await deleteAgent(id);
                  // If the deleted agent was selected, auto-select another
                  const remaining = useAgentStore.getState().agents;
                  if (useAgentStore.getState().selectedAgentId === null && remaining.length > 0) {
                    selectAgent(remaining[0].id);
                  }
                } catch (e) {
                  console.error('[NavBar] Failed to delete agent:', e);
                }
              }}
              className="w-full text-left px-3 py-1.5 text-xs hover:bg-rose-50 dark:hover:bg-rose-500/10 text-rose-500 flex items-center gap-2"
            >
              <Codicon name="trash" className="text-[12px]" />
              Delete Agent
            </button>
          )}
        </div>
      )}
    </div>
  );
}
