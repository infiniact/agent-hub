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
  const scanForAgents = useAcpStore((s) => s.scanForAgents);
  const fetchAgents = useAgentStore((s) => s.fetchAgents);

  useEffect(() => {
    if (!isTauri()) return;
    console.log('[Providers] Initializing app');

    // Load settings and scan for agents
    loadSettings();
    scanForAgents();
    fetchAgents();

    // Initialize Tauri event listeners for chat
    initializeChatListeners();

    // Initialize Tauri event listeners for orchestration
    initializeOrchestrationListeners();

    console.log('[Providers] App initialized');
  }, [loadSettings, scanForAgents, fetchAgents]);

  return <>{children}</>;
}
