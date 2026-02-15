"use client";

import { useState } from "react";
import { Modal } from "@/components/ui/Modal";
import { Button } from "@/components/ui/Button";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";
import { useChatToolStore } from "@/stores/chatToolStore";
import { useAgentStore } from "@/stores/agentStore";
import { useWorkspaceStore } from "@/stores/workspaceStore";
import { CHAT_TOOL_PLUGINS } from "@/types/chatTool";
import type { ChatToolPluginInfo } from "@/types/chatTool";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ChatToolCreateDialog({ open, onClose }: Props) {
  const createChatTool = useChatToolStore((s) => s.createChatTool);
  const selectChatTool = useChatToolStore((s) => s.selectChatTool);
  const agents = useAgentStore((s) => s.agents);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);

  const [step, setStep] = useState<"select" | "configure">("select");
  const [selectedPlugin, setSelectedPlugin] = useState<ChatToolPluginInfo | null>(null);
  const [name, setName] = useState("");
  const [linkedAgentId, setLinkedAgentId] = useState("");
  const [autoReplyMode, setAutoReplyMode] = useState("all");
  const [configValues, setConfigValues] = useState<Record<string, string>>({});
  const [creating, setCreating] = useState(false);

  const resetForm = () => {
    setStep("select");
    setSelectedPlugin(null);
    setName("");
    setLinkedAgentId("");
    setAutoReplyMode("all");
    setConfigValues({});
    setCreating(false);
  };

  const handleClose = () => {
    resetForm();
    onClose();
  };

  const handleSelectPlugin = (plugin: ChatToolPluginInfo) => {
    setSelectedPlugin(plugin);
    setName(plugin.name);
    setStep("configure");
  };

  const handleCreate = async () => {
    if (!selectedPlugin || !name.trim()) return;
    setCreating(true);
    try {
      const configJson = JSON.stringify(configValues);
      const chatTool = await createChatTool({
        name: name.trim(),
        plugin_type: selectedPlugin.type,
        config_json: configJson,
        linked_agent_id: linkedAgentId || undefined,
        auto_reply_mode: autoReplyMode,
        workspace_id: activeWorkspaceId ?? undefined,
      });
      selectChatTool(chatTool.id);
      handleClose();
    } catch (error) {
      console.error("[ChatToolCreate] Failed:", error);
      setCreating(false);
    }
  };

  return (
    <Modal open={open} onClose={handleClose} title="Add Chat Tool">
      {step === "select" && (
        <div className="space-y-2">
          <p className="text-xs text-slate-500 dark:text-gray-400 mb-3">
            Select a chat platform to integrate:
          </p>
          {CHAT_TOOL_PLUGINS.map((plugin) => (
            <button
              key={plugin.type}
              onClick={() => handleSelectPlugin(plugin)}
              className="w-full flex items-center gap-3 p-3 rounded-lg border border-slate-200 dark:border-border-dark hover:border-primary/50 hover:bg-primary/5 transition-all text-left"
            >
              <div className="size-10 rounded-lg bg-primary/10 flex items-center justify-center shrink-0">
                <Codicon name={plugin.icon} className="text-primary text-[20px]" />
              </div>
              <div>
                <div className="text-sm font-semibold text-slate-800 dark:text-white">
                  {plugin.name}
                </div>
                <div className="text-xs text-slate-400 dark:text-gray-500">
                  {plugin.description}
                </div>
              </div>
            </button>
          ))}
        </div>
      )}

      {step === "configure" && selectedPlugin && (
        <div className="space-y-4">
          <button
            onClick={() => setStep("select")}
            className="text-xs text-slate-400 hover:text-primary flex items-center gap-1 transition-colors"
          >
            <Codicon name="chevron-left" className="text-[12px]" />
            Back
          </button>

          {/* Name */}
          <div>
            <label className="block text-xs font-semibold text-slate-600 dark:text-gray-300 mb-1">
              Name
            </label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="w-full h-9 px-3 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-800 dark:text-white focus:outline-none focus:border-primary"
              placeholder="My WeChat"
            />
          </div>

          {/* Plugin-specific config fields */}
          {selectedPlugin.configFields.map((field) => (
            <div key={field.key}>
              <label className="block text-xs font-semibold text-slate-600 dark:text-gray-300 mb-1">
                {field.label}
              </label>
              <input
                type={field.type === "password" ? "password" : "text"}
                value={configValues[field.key] || ""}
                onChange={(e) =>
                  setConfigValues((prev) => ({ ...prev, [field.key]: e.target.value }))
                }
                className="w-full h-9 px-3 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-800 dark:text-white focus:outline-none focus:border-primary"
                placeholder={field.placeholder}
              />
            </div>
          ))}

          {/* Link Agent */}
          <div>
            <label className="block text-xs font-semibold text-slate-600 dark:text-gray-300 mb-1">
              Link to Agent
            </label>
            <select
              value={linkedAgentId}
              onChange={(e) => setLinkedAgentId(e.target.value)}
              className="w-full h-9 px-3 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-800 dark:text-white focus:outline-none focus:border-primary"
            >
              <option value="">None (manual only)</option>
              {agents.map((agent) => (
                <option key={agent.id} value={agent.id}>
                  {agent.name}
                </option>
              ))}
            </select>
            <p className="text-[10px] text-slate-400 dark:text-gray-500 mt-1">
              Messages will be forwarded to this agent for auto-reply.
            </p>
          </div>

          {/* Auto Reply Mode */}
          <div>
            <label className="block text-xs font-semibold text-slate-600 dark:text-gray-300 mb-1">
              Auto Reply
            </label>
            <select
              value={autoReplyMode}
              onChange={(e) => setAutoReplyMode(e.target.value)}
              className="w-full h-9 px-3 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-800 dark:text-white focus:outline-none focus:border-primary"
            >
              <option value="all">All messages</option>
              <option value="contacts_only">Contacts only</option>
              <option value="none">Disabled</option>
            </select>
          </div>

          {/* Create button */}
          <div className="flex justify-end gap-2 pt-2">
            <Button variant="ghost" size="sm" onClick={handleClose}>
              Cancel
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={handleCreate}
              disabled={!name.trim() || creating}
            >
              {creating ? "Creating..." : "Create"}
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
