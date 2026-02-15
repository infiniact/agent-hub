"use client";

import { useState } from "react";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { useAgentStore } from "@/stores/agentStore";
import { Modal } from "@/components/ui/Modal";
import { Codicon } from "@/components/ui/Codicon";
import { IconPicker } from "@/components/ui/IconPicker";
import { cn } from "@/lib/cn";

interface WorkspaceCreateDialogProps {
  open: boolean;
  onClose: () => void;
}

export function WorkspaceCreateDialog({
  open,
  onClose,
}: WorkspaceCreateDialogProps) {
  const [name, setName] = useState("");
  const [icon, setIcon] = useState("folder");
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [creating, setCreating] = useState(false);

  const agents = useAgentStore((s) => s.agents);
  const fetchAgents = useAgentStore((s) => s.fetchAgents);
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const setActiveWorkspace = useWorkspaceStore((s) => s.setActiveWorkspace);
  const selectWorkspaceDirectory = useWorkspaceStore(
    (s) => s.selectWorkspaceDirectory
  );

  const handleCreate = async () => {
    if (!name.trim()) return;
    setCreating(true);
    try {
      const ws = await createWorkspace({
        name: name.trim(),
        icon,
        agent_ids: selectedAgents,
      });
      await setActiveWorkspace(ws.id);

      // Refetch agents to show the cloned agents for the new workspace
      await fetchAgents();

      // Prompt user to select a working directory for the new workspace
      await selectWorkspaceDirectory(ws.id);

      resetAndClose();
    } catch (error) {
      console.error("[WorkspaceCreateDialog] Failed to create:", error);
    } finally {
      setCreating(false);
    }
  };

  const resetAndClose = () => {
    setName("");
    setIcon("folder");
    setSelectedAgents([]);
    onClose();
  };

  const toggleAgent = (agentId: string) => {
    setSelectedAgents((prev) =>
      prev.includes(agentId)
        ? prev.filter((id) => id !== agentId)
        : [...prev, agentId]
    );
  };

  return (
    <Modal open={open} onClose={resetAndClose} title="New Workspace">
      <div className="space-y-4">
        {/* Name */}
        <div>
          <label className="block text-xs font-medium text-slate-500 dark:text-gray-400 mb-1">
            Name
          </label>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCreate();
            }}
            placeholder="My Project"
            className="w-full px-3 py-2 rounded-lg bg-slate-50 dark:bg-white/5 border border-slate-200 dark:border-border-dark text-sm text-slate-900 dark:text-white placeholder-slate-400 dark:placeholder-gray-600 focus:outline-none focus:border-primary"
            autoFocus
          />
        </div>

        {/* Icon picker */}
        <div>
          <label className="block text-xs font-medium text-slate-500 dark:text-gray-400 mb-1">
            Icon
          </label>
          <IconPicker value={icon} onChange={setIcon} />
        </div>

        {/* Agent template multi-select */}
        {agents.length > 0 && (
          <div>
            <label className="block text-xs font-medium text-slate-500 dark:text-gray-400 mb-1">
              Agent Templates
            </label>
            <p className="text-[10px] text-slate-400 dark:text-gray-600 mb-1.5">
              Selected agents will be cloned as independent instances in the new
              workspace.
            </p>
            <div className="max-h-32 overflow-y-auto space-y-1">
              {agents.map((agent) => (
                <button
                  key={agent.id}
                  onClick={() => toggleAgent(agent.id)}
                  className={cn(
                    "w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left transition-colors text-xs",
                    selectedAgents.includes(agent.id)
                      ? "bg-primary/10 text-primary"
                      : "text-slate-500 dark:text-gray-400 hover:bg-slate-100 dark:hover:bg-white/5"
                  )}
                >
                  <div
                    className={cn(
                      "size-4 rounded border flex items-center justify-center shrink-0",
                      selectedAgents.includes(agent.id)
                        ? "bg-primary border-primary"
                        : "border-slate-300 dark:border-gray-600"
                    )}
                  >
                    {selectedAgents.includes(agent.id) && (
                      <Codicon
                        name="check"
                        className="text-[10px] text-background-dark"
                      />
                    )}
                  </div>
                  {agent.name}
                </button>
              ))}
            </div>
          </div>
        )}

        <p className="text-[10px] text-slate-400 dark:text-gray-500">
          A folder picker will open after creation to set the working directory.
        </p>

        {/* Actions */}
        <div className="flex justify-end gap-2 pt-2">
          <button
            onClick={resetAndClose}
            className="px-4 py-2 text-xs rounded-lg text-slate-500 dark:text-gray-400 hover:bg-slate-100 dark:hover:bg-white/5 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={!name.trim() || creating}
            className="px-4 py-2 text-xs rounded-lg bg-primary text-background-dark font-medium hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {creating ? "Creating..." : "Create"}
          </button>
        </div>
      </div>
    </Modal>
  );
}
