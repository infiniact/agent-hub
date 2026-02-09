"use client";

import { MessageList } from "./MessageList";
import { ChatInput } from "./ChatInput";
import { OrchestrationPanel } from "@/components/orchestration/OrchestrationPanel";
import { useOrchestrationStore } from "@/stores/orchestrationStore";

export function ChatSection() {
  const isOrchestrating = useOrchestrationStore((s) => s.isOrchestrating);
  const activeTaskRun = useOrchestrationStore((s) => s.activeTaskRun);

  const showOrchestration = isOrchestrating || activeTaskRun !== null;

  return (
    <>
      <div className="flex-1 overflow-y-auto px-8 py-6 flex flex-col gap-6">
        {showOrchestration ? <OrchestrationPanel /> : <MessageList />}
      </div>
      <ChatInput />
    </>
  );
}
