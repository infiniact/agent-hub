"use client";

import { MessageList } from "./MessageList";
import { ChatInput } from "./ChatInput";
import { OrchestrationPanel } from "@/components/orchestration/OrchestrationPanel";
import { TaskHistoryPanel } from "@/components/orchestration/TaskHistoryPanel";
import { InlinePermission } from "@/components/chat/InlinePermission";
import { useOrchestrationStore } from "@/stores/orchestrationStore";

export function ChatSection() {
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const activeTaskRun = useOrchestrationStore((s) => s.activeTaskRun);
  const viewingTaskRun = useOrchestrationStore((s) => s.viewingTaskRun);
  const pendingOrchPermission = useOrchestrationStore((s) => s.pendingOrchPermission);
  const respondToOrchPermission = useOrchestrationStore((s) => s.respondToOrchPermission);

  const showOrchestration = isOrchestrating || activeTaskRun !== null;
  const showTaskHistory = !showOrchestration && viewingTaskRun !== null;

  return (
    <>
      <div className="flex-1 overflow-y-auto px-8 py-6 flex flex-col gap-6">
        {showOrchestration ? (
          <OrchestrationPanel />
        ) : showTaskHistory ? (
          <TaskHistoryPanel />
        ) : (
          <MessageList />
        )}
      </div>

      {/* Permission dialog — pinned between scroll area and input */}
      {pendingOrchPermission && (
        <div className="px-8 py-2 border-t border-slate-200 dark:border-border-dark/50 bg-slate-50 dark:bg-[#07070C]">
          <div className="max-w-6xl mx-auto">
          <InlinePermission
            request={{
              id: pendingOrchPermission.requestId,
              sessionId: pendingOrchPermission.sessionId,
              toolCall: pendingOrchPermission.toolCall,
              options: pendingOrchPermission.options,
            }}
            onResponse={(optionId) => {
              respondToOrchPermission(
                pendingOrchPermission.taskRunId,
                pendingOrchPermission.agentId,
                String(pendingOrchPermission.requestId),
                optionId
              );
            }}
            onDismiss={() => {
              respondToOrchPermission(
                pendingOrchPermission.taskRunId,
                pendingOrchPermission.agentId,
                String(pendingOrchPermission.requestId),
                "allow"
              );
            }}
          />
          </div>
        </div>
      )}

      {/* Hide chat input when viewing task history */}
      {!showTaskHistory && <ChatInput />}
    </>
  );
}
