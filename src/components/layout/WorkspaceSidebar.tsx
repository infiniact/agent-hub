"use client";

import { useState, useRef, useEffect } from "react";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { useAgentStore } from "@/stores/agentStore";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { useChatStore } from "@/stores/chatStore";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";
import { WorkspaceCreateDialog } from "./WorkspaceCreateDialog";

// Legacy icon values stored in DB that don't match codicon names
const LEGACY_ICON_MAP: Record<string, string> = { star: "star-full" };

function resolveIcon(icon: string): string {
  return LEGACY_ICON_MAP[icon] ?? icon;
}

function isImageIcon(icon: string): boolean {
  return icon.startsWith("data:");
}

// ---------------------------------------------------------------------------
// Per-workspace state snapshot cache
// Saves / restores agent, chat and orchestration store slices so that
// switching workspaces feels instant and does NOT disturb running tasks.
// ---------------------------------------------------------------------------

interface WorkspaceSnapshot {
  agent: Record<string, unknown>;
  chat: Record<string, unknown>;
  orchestration: Record<string, unknown>;
}

const snapshotCache = new Map<string, WorkspaceSnapshot>();

function saveSnapshot(workspaceId: string) {
  const a = useAgentStore.getState();
  const c = useChatStore.getState();
  const o = useOrchestrationStore.getState();

  snapshotCache.set(workspaceId, {
    agent: {
      agents: a.agents,
      selectedAgentId: a.selectedAgentId,
      controlHubAgentId: a.controlHubAgentId,
      readyAgentIds: a.readyAgentIds,
      showKanban: a.showKanban,
    },
    chat: {
      sessions: c.sessions,
      currentSessionId: c.currentSessionId,
      messages: c.messages,
      // Streaming state is ephemeral — we save it so the user sees what they
      // left, but on restore we clear it and rely on DB refresh + live events.
      isStreaming: c.isStreaming,
      streamedContent: c.streamedContent,
      toolCalls: c.toolCalls,
      pendingPermission: c.pendingPermission,
    },
    orchestration: {
      taskRunStates: o.taskRunStates,
      focusedTaskRunId: o.focusedTaskRunId,
      isOrchestrating: o.isOrchestrating,
      taskRuns: o.taskRuns,
      pendingOrchPermissions: o.pendingOrchPermissions,
      viewingTaskRun: o.viewingTaskRun,
      viewingAssignments: o.viewingAssignments,
      viewingAgentTracking: o.viewingAgentTracking,
      viewingTaskPlan: o.viewingTaskPlan,
      discoveredSkills: o.discoveredSkills,
      restoredTaskRunIds: o.restoredTaskRunIds,
    },
  });
}

/** Restore a previously saved snapshot. Returns false if none exists. */
function restoreSnapshot(workspaceId: string): boolean {
  const snap = snapshotCache.get(workspaceId);
  if (!snap) return false;

  useAgentStore.setState(snap.agent);

  // Restore chat state but always reset streaming — when the user comes back
  // we rely on a DB refresh to catch any messages that completed while away.
  // If the stream is still running, the live event listeners will re-attach
  // because currentSessionId matches again.
  useChatStore.setState({
    ...snap.chat,
    isStreaming: false,
    streamedContent: "",
    toolCalls: [],
  });

  useOrchestrationStore.setState(snap.orchestration);

  return true;
}

// ---------------------------------------------------------------------------

function getInitialChatState() {
  return {
    sessions: [],
    currentSessionId: null,
    messages: [],
    isStreaming: false,
    streamedContent: "",
    toolCalls: [],
    pendingPermission: null,
  };
}

function getInitialOrchState() {
  return {
    taskRunStates: {},
    focusedTaskRunId: null,
    isOrchestrating: false,
    taskRuns: [],
    pendingOrchPermissions: [],
    viewingTaskRun: null,
    viewingAssignments: [],
    viewingAgentTracking: {},
    viewingTaskPlan: null,
    discoveredSkills: null,
    restoredTaskRunIds: [],
  };
}

// ---------------------------------------------------------------------------

export function WorkspaceSidebar() {
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const setActiveWorkspace = useWorkspaceStore((s) => s.setActiveWorkspace);
  const deleteWorkspace = useWorkspaceStore((s) => s.deleteWorkspace);
  const updateWorkspace = useWorkspaceStore((s) => s.updateWorkspace);
  const sidebarExpanded = useWorkspaceStore((s) => s.sidebarExpanded);
  const toggleSidebar = useWorkspaceStore((s) => s.toggleSidebar);
  const selectWorkspaceDirectory = useWorkspaceStore((s) => s.selectWorkspaceDirectory);
  const fetchTaskRuns = useOrchestrationStore((s) => s.fetchTaskRuns);
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);

  const [showCreateDialog, setShowCreateDialog] = useState(false);
  const [contextMenu, setContextMenu] = useState<{
    workspaceId: string;
    x: number;
    y: number;
  } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const renameInputRef = useRef<HTMLInputElement>(null);

  // Close context menu on click outside
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [contextMenu]);

  // Focus rename input
  useEffect(() => {
    if (renaming && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [renaming]);

  const handleSwitchWorkspace = async (id: string) => {
    if (id === activeWorkspaceId) return;

    // 1. Save the current workspace's full state (does NOT cancel anything)
    if (activeWorkspaceId) {
      saveSnapshot(activeWorkspaceId);
    }

    // 2. Persist the active workspace choice
    await setActiveWorkspace(id);

    // 3. Try to restore a previously cached snapshot for the target workspace
    const restored = restoreSnapshot(id);

    if (restored) {
      // Snapshot restored — data is already correct, no refresh needed.
      // If a session had an active stream that completed while away,
      // the user can refresh manually or the next sendPrompt will sync.
    } else {
      // First visit to this workspace — clean initialisation
      useChatStore.setState(getInitialChatState());
      useOrchestrationStore.setState(getInitialOrchState());
      useAgentStore.setState({ readyAgentIds: [], showKanban: false });

      const agentStore = useAgentStore.getState();
      await agentStore.fetchAgents();
      const newAgents = useAgentStore.getState().agents;
      if (newAgents.length > 0) {
        await agentStore.selectAgent(newAgents[0].id);
      }
      fetchTaskRuns();
    }
  };

  const handleContextMenu = (e: React.MouseEvent, workspaceId: string) => {
    e.preventDefault();
    setContextMenu({ workspaceId, x: e.clientX, y: e.clientY });
  };

  const handleRenameSubmit = async () => {
    if (renaming && renameValue.trim()) {
      await updateWorkspace(renaming, { name: renameValue.trim() });
    }
    setRenaming(null);
  };

  return (
    <>
      <div
        className={cn(
          "flex-none flex flex-col bg-white dark:bg-[#07070C] border-r border-slate-200 dark:border-border-dark transition-all duration-200 overflow-hidden",
          sidebarExpanded ? "w-48" : "w-12"
        )}
      >
        {/* Workspace list */}
        <div className="flex-1 flex flex-col gap-1 py-2 px-1.5 overflow-y-auto">
          {workspaces.map((ws) => {
            const isActive = ws.id === activeWorkspaceId;
            const resolved = resolveIcon(ws.icon);
            const isImage = isImageIcon(resolved);

            // Determine if this workspace has active orchestration tasks
            let isBusy = false;
            if (isActive) {
              isBusy = isOrchestrating;
            } else {
              const snap = snapshotCache.get(ws.id);
              if (snap) {
                isBusy = !!(snap.orchestration as { isOrchestrating?: boolean }).isOrchestrating;
              }
            }

            return (
              <div
                key={ws.id}
                className={cn(
                  "group relative flex items-center gap-2 rounded-lg cursor-pointer transition-all",
                  sidebarExpanded ? "px-2 py-1.5" : "px-0 py-0 justify-center",
                  isActive
                    ? "bg-primary/10 text-primary"
                    : "text-slate-400 dark:text-gray-500 hover:bg-slate-100 dark:hover:bg-white/5 hover:text-slate-600 dark:hover:text-gray-300"
                )}
                onClick={() => handleSwitchWorkspace(ws.id)}
                onContextMenu={(e) => handleContextMenu(e, ws.id)}
                title={sidebarExpanded ? undefined : ws.name}
              >
                <div
                  className={cn(
                    "size-8 rounded-lg flex items-center justify-center shrink-0 transition-all",
                    isActive
                      ? "bg-primary text-background-dark shadow-[0_0_8px_rgba(0,229,255,0.2)]"
                      : "bg-slate-100 dark:bg-white/5",
                    isBusy && "animate-[workspace-breathing_2s_ease-in-out_infinite]"
                  )}
                >
                  {isImage ? (
                    <img src={resolved} alt="" className="size-4 rounded object-cover" />
                  ) : (
                    <Codicon name={resolved} className="text-[16px]" />
                  )}
                </div>
                {sidebarExpanded && (
                  <div className="flex-1 min-w-0">
                    {renaming === ws.id ? (
                      <input
                        ref={renameInputRef}
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        onBlur={handleRenameSubmit}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") handleRenameSubmit();
                          if (e.key === "Escape") setRenaming(null);
                        }}
                        className="w-full bg-transparent text-xs font-medium outline-none border-b border-primary"
                      />
                    ) : (
                      <>
                        <div className="text-xs font-medium truncate">
                          {ws.name}
                        </div>
                        {ws.working_directory && (
                          <div className="text-[10px] text-slate-400 dark:text-gray-600 truncate">
                            {ws.working_directory.split("/").pop()}
                          </div>
                        )}
                      </>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        {/* Bottom actions */}
        <div className="flex-none border-t border-slate-200 dark:border-border-dark p-1.5 flex flex-col gap-1">
          {/* Add workspace */}
          <button
            onClick={() => setShowCreateDialog(true)}
            className={cn(
              "flex items-center gap-2 rounded-lg text-slate-400 dark:text-gray-500 hover:text-primary hover:bg-slate-100 dark:hover:bg-white/5 transition-colors",
              sidebarExpanded ? "px-2 py-1.5" : "px-0 py-1.5 justify-center"
            )}
            title="New workspace"
          >
            <div className="size-8 rounded-lg flex items-center justify-center shrink-0">
              <Codicon name="add" className="text-[16px]" />
            </div>
            {sidebarExpanded && (
              <span className="text-xs">New workspace</span>
            )}
          </button>
          {/* Toggle expand */}
          <button
            onClick={toggleSidebar}
            className={cn(
              "flex items-center gap-2 rounded-lg text-slate-400 dark:text-gray-500 hover:text-primary hover:bg-slate-100 dark:hover:bg-white/5 transition-colors",
              sidebarExpanded ? "px-2 py-1.5" : "px-0 py-1.5 justify-center"
            )}
            title={sidebarExpanded ? "Collapse sidebar" : "Expand sidebar"}
          >
            <div className="size-8 rounded-lg flex items-center justify-center shrink-0">
              <Codicon
                name={sidebarExpanded ? "chevron-left" : "chevron-right"}
                className="text-[16px]"
              />
            </div>
            {sidebarExpanded && (
              <span className="text-xs">Collapse</span>
            )}
          </button>
        </div>
      </div>

      {/* Context menu */}
      {contextMenu && (
        <div
          className="fixed z-50 bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg shadow-2xl py-1 min-w-[160px]"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          <button
            onClick={() => {
              const ws = workspaces.find((w) => w.id === contextMenu.workspaceId);
              if (ws) {
                setRenameValue(ws.name);
                setRenaming(ws.id);
              }
              setContextMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 text-xs hover:bg-slate-100 dark:hover:bg-white/5 flex items-center gap-2"
          >
            <Codicon name="edit" className="text-[12px]" />
            Rename
          </button>
          <button
            onClick={() => {
              selectWorkspaceDirectory(contextMenu.workspaceId);
              setContextMenu(null);
            }}
            className="w-full text-left px-3 py-1.5 text-xs hover:bg-slate-100 dark:hover:bg-white/5 flex items-center gap-2"
          >
            <Codicon name="folder-opened" className="text-[12px]" />
            Set directory
          </button>
          {workspaces.length > 1 && (
            <button
              onClick={() => {
                deleteWorkspace(contextMenu.workspaceId);
                setContextMenu(null);
              }}
              className="w-full text-left px-3 py-1.5 text-xs hover:bg-rose-50 dark:hover:bg-rose-500/10 text-rose-500 flex items-center gap-2"
            >
              <Codicon name="trash" className="text-[12px]" />
              Delete
            </button>
          )}
        </div>
      )}

      <WorkspaceCreateDialog
        open={showCreateDialog}
        onClose={() => setShowCreateDialog(false)}
      />
    </>
  );
}
