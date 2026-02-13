"use client";

import { useEffect } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import { useAcpStore } from "@/stores/acpStore";
import { useAgentStore } from "@/stores/agentStore";
import { useChatStore } from "@/stores/chatStore";
import { initializeChatListeners } from "@/stores/chatStore";
import { initializeOrchestrationListeners } from "@/stores/orchestrationStore";
import { isTauri } from "@/lib/tauri";

export function Providers({ children }: { children: React.ReactNode }) {
  const loadSettings = useSettingsStore((s) => s.loadSettings);
  const loadWorkingDirectory = useSettingsStore((s) => s.loadWorkingDirectory);
  const scanForAgents = useAcpStore((s) => s.scanForAgents);
  const fetchAgents = useAgentStore((s) => s.fetchAgents);

  useEffect(() => {
    if (!isTauri()) return;
    console.log('[Providers] Initializing app');

    // Load settings and scan for agents
    loadSettings();
    loadWorkingDirectory();
    scanForAgents();

    // Fetch agents then merge DB-cached models into discoveredAgents
    fetchAgents().then(() => {
      const agents = useAgentStore.getState().agents;
      const acpStore = useAcpStore.getState();
      for (const agent of agents) {
        if (agent.acp_command && agent.available_models_json) {
          try {
            const models: string[] = JSON.parse(agent.available_models_json);
            if (models.length > 0) {
              acpStore.updateDiscoveredAgentModels(agent.acp_command, models);
            }
          } catch { /* ignore parse errors */ }
        }
      }
    });

    // Initialize Tauri event listeners for chat
    initializeChatListeners();

    // Initialize Tauri event listeners for orchestration
    initializeOrchestrationListeners();

    console.log('[Providers] App initialized');
  }, [loadSettings, loadWorkingDirectory, scanForAgents, fetchAgents]);

  return <>{children}</>;
}
