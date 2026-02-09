"use client";

import { useChatStore } from "@/stores/chatStore";
import { InlinePermission } from "./InlinePermission";
import { useAgentStore } from "@/stores/agentStore";
import { useRef } from "react";
import { tauriInvoke } from "@/lib/tauri";

export function PermissionDialogWrapper() {
  const pendingPermission = useChatStore((s) => s.pendingPermission);
  const selectedAgentId = useAgentStore((s) => s.selectedAgentId);
  const clearPendingPermission = useChatStore((s) => s.clearPendingPermission);

  // Keep a ref to the current permission so the handler always has the latest data,
  // even if the store clears it before the async handler runs.
  const permissionRef = useRef(pendingPermission);
  permissionRef.current = pendingPermission;

  if (!pendingPermission || !selectedAgentId) {
    return null;
  }

  const handleResponse = async (optionId: string, userMessage?: string) => {
    const perm = permissionRef.current;
    if (!perm) return;

    const requestId = perm.id;
    const agentId = selectedAgentId;

    // Clear immediately so the dialog hides
    clearPendingPermission();

    try {
      await tauriInvoke('respond_permission', {
        agentId,
        requestId,
        optionId,
        userMessage,
      });
      console.log('[PermissionDialog] Permission response sent');
    } catch (error) {
      console.error('[PermissionDialog] Failed to send permission response:', error);
    }
  };

  const handleClose = () => {
    clearPendingPermission();
  };

  return (
    <div className="fixed bottom-4 left-1/2 -translate-x-1/2 z-50 w-full max-w-md px-4">
      <InlinePermission
        request={pendingPermission}
        onResponse={handleResponse}
        onDismiss={handleClose}
      />
    </div>
  );
}
