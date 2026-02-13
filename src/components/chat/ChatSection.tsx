"use client";

import { MessageList } from "./MessageList";
import { ChatInput } from "./ChatInput";
import { OrchestrationPanel } from "@/components/orchestration/OrchestrationPanel";
import { TaskHistoryPanel } from "@/components/orchestration/TaskHistoryPanel";
import { useOrchestrationStore } from "@/stores/orchestrationStore";

export function ChatSection() {
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const activeTaskRun = useOrchestrationStore((s) => s.activeTaskRun);
  const viewingTaskRun = useOrchestrationStore((s) => s.viewingTaskRun);

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
      {/* Hide chat input when viewing task history */}
      {!showTaskHistory && <ChatInput />}
    </>
  );
}
