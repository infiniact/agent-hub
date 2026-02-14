"use client";

import type { TaskPlan, AgentTrackingInfo, PlanValidation } from "@/types/orchestration";
import { Codicon } from "@/components/ui/Codicon";
import { useAgentStore } from "@/stores/agentStore";
import { MarkdownContent } from "@/components/chat/MarkdownContent";

interface TaskPlanViewProps {
  plan: TaskPlan;
  agentTracking: Record<string, AgentTrackingInfo>;
  planValidation?: PlanValidation | null;
}

export function TaskPlanView({ plan, agentTracking, planValidation }: TaskPlanViewProps) {
  const agents = useAgentStore((s) => s.agents);

  const getAgentName = (agentId: string) => {
    return agents.find((a) => a.id === agentId)?.name ?? "Unknown Agent";
  };

  const getStatusIcon = (agentId: string) => {
    const tracking = agentTracking[agentId];
    if (!tracking) return <Codicon name="circle-outline" className="text-[14px] text-slate-300 dark:text-gray-600" />;
    if (tracking.status === "completed")
      return <Codicon name="pass-filled" className="text-[14px] text-emerald-400" />;
    if (tracking.status === "running")
      return <Codicon name="loading" className="text-[14px] codicon-modifier-spin text-primary" />;
    return <Codicon name="circle-outline" className="text-[14px] text-slate-300 dark:text-gray-600" />;
  };

  const getAssignmentWarnings = (agentId: string): string[] => {
    if (!planValidation) return [];
    const av = planValidation.assignment_validations.find((v) => v.agent_id === agentId);
    return av?.warnings ?? [];
  };

  // Sort assignments by sequence_order
  const sorted = [...plan.assignments].sort(
    (a, b) => a.sequence_order - b.sequence_order
  );

  return (
    <div className="rounded-lg border border-slate-200 dark:border-border-dark/50 bg-white dark:bg-surface-dark p-3">
      <p className="text-xs font-bold uppercase tracking-wider text-slate-500 dark:text-gray-400 mb-2">
        Execution Plan
      </p>

      {/* Analysis */}
      <div className="text-xs text-slate-600 dark:text-gray-400 mb-3">
        <MarkdownContent content={plan.analysis} className="text-xs" />
      </div>

      {/* Steps */}
      <div className="flex flex-col gap-1">
        {sorted.map((assignment, idx) => {
          const warnings = getAssignmentWarnings(assignment.agent_id);
          return (
            <div key={`${assignment.agent_id}-${idx}`}>
              {idx > 0 && (
                <div className="flex justify-center py-0.5">
                  <Codicon name="arrow-down" className="text-[12px] text-slate-300 dark:text-gray-600" />
                </div>
              )}
              <div className="px-2 py-1.5 rounded bg-slate-50 dark:bg-white/[0.03]">
                <div className="flex items-start gap-2">
                  {getStatusIcon(assignment.agent_id)}
                  <span className="text-xs font-medium text-slate-700 dark:text-gray-300 shrink-0 pt-0.5">
                    {getAgentName(assignment.agent_id)}
                  </span>
                  <div className="text-[11px] text-slate-400 dark:text-gray-500 flex-1 min-w-0">
                    <MarkdownContent content={assignment.task_description} className="text-[11px]" />
                  </div>
                  <span className="text-[10px] text-slate-300 dark:text-gray-600 font-mono shrink-0 pt-0.5">
                    #{assignment.sequence_order}
                  </span>
                </div>

                {/* Matched skills tags */}
                {assignment.matched_skills && assignment.matched_skills.length > 0 && (
                  <div className="flex flex-wrap gap-1 mt-1 ml-5">
                    {assignment.matched_skills.map((skillId) => (
                      <span
                        key={skillId}
                        className="px-1.5 py-0.5 rounded text-[9px] font-medium bg-purple-500/10 text-purple-400 border border-purple-500/20"
                      >
                        {skillId}
                      </span>
                    ))}
                  </div>
                )}

                {/* Selection reason */}
                {assignment.selection_reason && (
                  <p className="text-[10px] italic text-slate-400 dark:text-gray-600 mt-1 ml-5">
                    {assignment.selection_reason}
                  </p>
                )}

                {/* Validation warnings */}
                {warnings.length > 0 && (
                  <div className="flex flex-wrap gap-1 mt-1 ml-5">
                    {warnings.map((warning, wi) => (
                      <span
                        key={wi}
                        className="px-1.5 py-0.5 rounded text-[9px] font-medium bg-amber-500/10 text-amber-500 border border-amber-500/20"
                      >
                        {warning}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
