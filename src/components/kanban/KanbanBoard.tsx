"use client";

import { useState, useMemo, useEffect } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";
import { useOrchestrationStore } from "@/stores/orchestrationStore";
import { useAgentStore } from "@/stores/agentStore";
import { tauriInvoke } from "@/lib/tauri";
import type { TaskRun, RecurrencePattern } from "@/types/orchestration";

type TaskStatus = "todo" | "inprogress" | "done" | "cancelled";
type TimeFilter = "day" | "week" | "month" | "all";

interface KanbanColumn {
  id: TaskStatus;
  label: string;
  color: string;
  bgColor: string;
  borderColor: string;
}

const columns: KanbanColumn[] = [
  {
    id: "todo",
    label: "To Do",
    color: "text-slate-500 dark:text-slate-400",
    bgColor: "bg-slate-500",
    borderColor: "border-slate-400 dark:border-slate-500",
  },
  {
    id: "inprogress",
    label: "In Progress",
    color: "text-blue-500 dark:text-blue-400",
    bgColor: "bg-blue-500",
    borderColor: "border-blue-400 dark:border-blue-500",
  },
  {
    id: "done",
    label: "Done",
    color: "text-emerald-500 dark:text-emerald-400",
    bgColor: "bg-emerald-500",
    borderColor: "border-emerald-400 dark:border-emerald-500",
  },
  {
    id: "cancelled",
    label: "Cancelled",
    color: "text-rose-500 dark:text-rose-400",
    bgColor: "bg-rose-500",
    borderColor: "border-rose-400 dark:border-rose-500",
  },
];

// Map task run status to kanban status
function mapTaskStatus(task: TaskRun): TaskStatus {
  // Scheduled tasks that haven't run yet go to todo
  if (task.schedule_type !== "none" && task.next_run_at) {
    const nextRun = new Date(task.next_run_at);
    if (nextRun > new Date()) {
      return "todo";
    }
  }
  // Regular status mapping
  switch (task.status) {
    case "pending":
    case "analyzing":
      return "todo";
    case "running":
    case "awaiting_confirmation":
      return "inprogress";
    case "completed":
      return "done";
    case "failed":
    case "cancelled":
      return "cancelled";
    default:
      return "todo";
  }
}

// Get status badge for task run
function getStatusBadge(status: TaskRun["status"]) {
  switch (status) {
    case "pending":
      return { text: "Pending", className: "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300" };
    case "analyzing":
      return { text: "Analyzing", className: "bg-amber-100 text-amber-600 dark:bg-amber-900/30 dark:text-amber-400" };
    case "running":
      return { text: "Running", className: "bg-blue-100 text-blue-600 dark:bg-blue-900/30 dark:text-blue-400" };
    case "awaiting_confirmation":
      return { text: "Review", className: "bg-purple-100 text-purple-600 dark:bg-purple-900/30 dark:text-purple-400" };
    case "completed":
      return { text: "Done", className: "bg-emerald-100 text-emerald-600 dark:bg-emerald-900/30 dark:text-emerald-400" };
    case "failed":
      return { text: "Failed", className: "bg-rose-100 text-rose-600 dark:bg-rose-900/30 dark:text-rose-400" };
    case "cancelled":
      return { text: "Cancelled", className: "bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400" };
    default:
      return { text: status, className: "bg-slate-100 text-slate-600 dark:bg-slate-800 dark:text-slate-300" };
  }
}

interface TaskCardProps {
  task: TaskRun;
  agents: ReturnType<typeof useAgentStore.getState>["agents"];
  onClick?: () => void;
  onCancel?: () => void;
  isSelected?: boolean;
}

// Parse recurrence pattern JSON
function parseRecurrencePattern(json: string | null): RecurrencePattern | null {
  if (!json) return null;
  try {
    return JSON.parse(json);
  } catch {
    return null;
  }
}

// Format next run time for display
function formatNextRun(nextRunAt: string | null): string {
  if (!nextRunAt) return "";
  const date = new Date(nextRunAt);
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();

  if (diffMs < 0) return "Overdue";

  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMins < 1) return "Now";
  if (diffMins < 60) return `In ${diffMins}m`;
  if (diffHours < 24) return `In ${diffHours}h`;
  if (diffDays < 7) return `In ${diffDays}d`;

  return date.toLocaleDateString("zh-CN", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function TaskCard({ task, agents, onClick, onCancel, isSelected }: TaskCardProps) {
  const [expanded, setExpanded] = useState(false);
  const statusBadge = getStatusBadge(task.status);
  const controlHubAgent = agents.find((a) => a.id === task.control_hub_agent_id);
  const recurrencePattern = parseRecurrencePattern(task.recurrence_pattern_json);

  // Check if task is scheduled
  const isScheduled = task.schedule_type !== "none" && task.next_run_at;
  const nextRunDisplay = formatNextRun(task.next_run_at);

  // Parse task plan to get assignment info
  let assignmentCount = 0;
  try {
    if (task.task_plan_json) {
      const plan = JSON.parse(task.task_plan_json);
      assignmentCount = plan.assignments?.length || 0;
    }
  } catch {
    // ignore parse errors
  }

  const formatDuration = (ms: number) => {
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
    return `${(ms / 60000).toFixed(1)}m`;
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString("zh-CN", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  // Get frequency icon and label
  const getFrequencyInfo = () => {
    if (!recurrencePattern) return null;
    switch (recurrencePattern.frequency) {
      case "daily":
        return { icon: "calendar", label: "Daily" };
      case "weekly":
        return { icon: "calendar", label: "Weekly" };
      case "monthly":
        return { icon: "calendar", label: "Monthly" };
      case "yearly":
        return { icon: "calendar", label: "Yearly" };
      default:
        return { icon: "calendar", label: "Scheduled" };
    }
  };

  const frequencyInfo = getFrequencyInfo();

  const isRunning = task.status === "running" || task.status === "analyzing" || task.status === "pending" || task.status === "awaiting_confirmation";

  return (
    <div
      onClick={onClick}
      className={cn(
        "group bg-white dark:bg-surface-dark border rounded-lg p-3 cursor-pointer transition-all",
        "hover:shadow-md hover:border-primary/30",
        isSelected && "ring-2 ring-primary border-primary/50",
        !isSelected && "border-slate-200 dark:border-border-dark",
        isScheduled && task.is_paused && "opacity-60"
      )}
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-2 mb-2">
        <h4 className="text-sm font-medium text-slate-800 dark:text-white line-clamp-2 flex-1">
          {task.title || task.user_prompt.slice(0, 80)}
        </h4>
        <div className="flex items-center gap-1.5 shrink-0">
          {/* Cancel button for running tasks */}
          {isRunning && onCancel && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                onCancel();
              }}
              className="size-5 rounded flex items-center justify-center text-slate-400 hover:text-rose-500 hover:bg-rose-50 dark:hover:bg-rose-900/20 transition-colors"
              title="Cancel task"
            >
              <Codicon name="debug-stop" className="text-[12px]" />
            </button>
          )}
          <span className={cn("text-[10px] font-medium px-1.5 py-0.5 rounded", statusBadge.className)}>
            {statusBadge.text}
          </span>
        </div>
      </div>

      {/* Description */}
      <p className="text-xs text-slate-500 dark:text-gray-400 line-clamp-2 mb-3">
        {task.user_prompt}
      </p>

      {/* Schedule info for scheduled tasks */}
      {isScheduled && (
        <div className={cn(
          "flex items-center gap-2 mb-2 text-[10px] px-2 py-1 rounded",
          task.is_paused
            ? "bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400"
            : "bg-blue-50 text-blue-600 dark:bg-blue-900/20 dark:text-blue-400"
        )}>
          <Codicon name="clock" className="text-[12px]" />
          {task.is_paused ? (
            <span>Paused</span>
          ) : (
            <>
              {frequencyInfo && (
                <span className="font-medium">{frequencyInfo.label}</span>
              )}
              <span>· {nextRunDisplay}</span>
            </>
          )}
        </div>
      )}

      {/* Meta info */}
      <div className="flex items-center gap-3 text-[10px] text-slate-400 dark:text-gray-500">
        {controlHubAgent && (
          <div className="flex items-center gap-1">
            <Codicon name="hubot" className="text-[12px]" />
            <span className="truncate max-w-[80px]">{controlHubAgent.name}</span>
          </div>
        )}
        {assignmentCount > 0 && (
          <div className="flex items-center gap-1">
            <Codicon name="person" className="text-[12px]" />
            <span>{assignmentCount} agents</span>
          </div>
        )}
        <div className="flex items-center gap-1">
          <Codicon name="clock" className="text-[12px]" />
          <span>{formatDate(task.created_at)}</span>
        </div>
      </div>

      {/* Expanded content */}
      {expanded && task.result_summary && (
        <div className="mt-3 pt-3 border-t border-slate-100 dark:border-border-dark">
          <p className="text-xs text-slate-600 dark:text-gray-300">{task.result_summary}</p>
        </div>
      )}

      {/* Token stats for completed tasks */}
      {task.status === "completed" && (task.total_tokens_in > 0 || task.total_tokens_out > 0) && (
        <div className="mt-2 pt-2 border-t border-slate-100 dark:border-border-dark/50 flex items-center gap-3 text-[10px] text-slate-400 dark:text-gray-500">
          <span>{task.total_tokens_in.toLocaleString()} in</span>
          <span>{task.total_tokens_out.toLocaleString()} out</span>
          {task.total_duration_ms > 0 && <span>{formatDuration(task.total_duration_ms)}</span>}
        </div>
      )}

      {/* Rating display */}
      {task.rating !== null && task.rating > 0 && (
        <div className="mt-2 flex items-center gap-0.5">
          {Array.from({ length: 5 }).map((_, i) => (
            <Codicon
              key={i}
              name="star-full"
              className={cn(
                "text-[12px]",
                i < task.rating! ? "text-amber-400" : "text-slate-300 dark:text-slate-600"
              )}
            />
          ))}
        </div>
      )}
    </div>
  );
}

interface KanbanBoardProps {
  selectedTaskId?: string | null;
}

export function KanbanBoard({ selectedTaskId }: KanbanBoardProps) {
  const taskRuns = useOrchestrationStore((s) => s.taskRuns);
  const fetchTaskRuns = useOrchestrationStore((s) => s.fetchTaskRuns);
  const viewTaskRun = useOrchestrationStore((s) => s.viewTaskRun);
  const agents = useAgentStore((s) => s.agents);
  const setShowKanban = useAgentStore((s) => s.setShowKanban);
  const [activeFilter, setActiveFilter] = useState<TaskStatus | "all">("all");
  const [timeFilter, setTimeFilter] = useState<TimeFilter>("day");

  // Handle task cancellation
  const handleCancelTask = async (taskId: string) => {
    try {
      await tauriInvoke('cancel_orchestration', { taskRunId: taskId });
      // Refresh list to reflect new status
      fetchTaskRuns();
    } catch (error) {
      console.error('[Kanban] Failed to cancel task:', error);
    }
  };

  // Handle task selection
  const handleTaskSelect = async (task: TaskRun) => {
    await viewTaskRun(task);
    // Close kanban panel to show the chat area with task details
    setShowKanban(false);
  };

  // Fetch task runs on mount
  useEffect(() => {
    fetchTaskRuns();
  }, [fetchTaskRuns]);

  // Filter tasks by time range
  const filteredByTime = useMemo(() => {
    if (timeFilter === "all") return taskRuns;

    const now = new Date();
    const startOfDay = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const startOfWeek = new Date(startOfDay);
    startOfWeek.setDate(startOfWeek.getDate() - startOfWeek.getDay()); // Sunday
    const startOfMonth = new Date(now.getFullYear(), now.getMonth(), 1);

    return taskRuns.filter((task) => {
      const taskDate = new Date(task.created_at);
      switch (timeFilter) {
        case "day":
          return taskDate >= startOfDay;
        case "week":
          return taskDate >= startOfWeek;
        case "month":
          return taskDate >= startOfMonth;
        default:
          return true;
      }
    });
  }, [taskRuns, timeFilter]);

  // Group tasks by status
  const tasksByStatus = useMemo(() => {
    const grouped: Record<TaskStatus, TaskRun[]> = {
      todo: [],
      inprogress: [],
      done: [],
      cancelled: [],
    };

    for (const task of filteredByTime) {
      const status = mapTaskStatus(task);
      grouped[status].push(task);
    }

    // Sort each group by created_at (newest first), but scheduled tasks by next_run_at
    for (const status of Object.keys(grouped) as TaskStatus[]) {
      grouped[status].sort((a, b) => {
        // Scheduled tasks go to top of todo column, sorted by next_run_at
        if (status === "todo") {
          const aScheduled = a.schedule_type !== "none" && a.next_run_at;
          const bScheduled = b.schedule_type !== "none" && b.next_run_at;
          if (aScheduled && bScheduled) {
            return new Date(a.next_run_at!).getTime() - new Date(b.next_run_at!).getTime();
          }
          if (aScheduled) return -1;
          if (bScheduled) return 1;
        }
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      });
    }

    return grouped;
  }, [filteredByTime]);

  // Filter columns if needed
  const visibleColumns = activeFilter === "all"
    ? columns
    : columns.filter((c) => c.id === activeFilter);

  // Calculate summary stats
  const stats = useMemo(() => ({
    total: filteredByTime.length,
    todo: tasksByStatus.todo.length,
    inprogress: tasksByStatus.inprogress.length,
    done: tasksByStatus.done.length,
    cancelled: tasksByStatus.cancelled.length,
  }), [filteredByTime, tasksByStatus]);

  return (
    <div className="h-full flex flex-col bg-slate-50 dark:bg-[#07070C]">
      {/* Header */}
      <div className="shrink-0 px-6 py-4 bg-white dark:bg-surface-dark border-b border-slate-200 dark:border-border-dark">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-3">
            <Codicon name="kanban" className="text-[24px] text-primary" />
            <h2 className="text-lg font-bold text-slate-900 dark:text-white">Session Kanban</h2>
          </div>

          {/* Status filter buttons */}
          <div className="flex items-center gap-1 bg-slate-100 dark:bg-white/5 rounded-lg p-1">
            <button
              onClick={() => setActiveFilter("all")}
              className={cn(
                "px-3 py-1.5 text-xs font-medium rounded-md transition-colors",
                activeFilter === "all"
                  ? "bg-white dark:bg-surface-dark text-slate-900 dark:text-white shadow-sm"
                  : "text-slate-500 dark:text-gray-400 hover:text-slate-700 dark:hover:text-gray-200"
              )}
            >
              All ({stats.total})
            </button>
            {columns.map((col) => (
              <button
                key={col.id}
                onClick={() => setActiveFilter(col.id)}
                className={cn(
                  "px-3 py-1.5 text-xs font-medium rounded-md transition-colors",
                  activeFilter === col.id
                    ? "bg-white dark:bg-surface-dark text-slate-900 dark:text-white shadow-sm"
                    : "text-slate-500 dark:text-gray-400 hover:text-slate-700 dark:hover:text-gray-200"
                )}
              >
                {col.label} ({stats[col.id]})
              </button>
            ))}
          </div>
        </div>

        {/* Time filter row */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-slate-400 dark:text-gray-500">Time:</span>
          <div className="flex items-center gap-1">
            {[
              { value: "all" as TimeFilter, label: "All" },
              { value: "day" as TimeFilter, label: "Today" },
              { value: "week" as TimeFilter, label: "This Week" },
              { value: "month" as TimeFilter, label: "This Month" },
            ].map((opt) => (
              <button
                key={opt.value}
                onClick={() => setTimeFilter(opt.value)}
                className={cn(
                  "px-2 py-1 text-[11px] font-medium rounded transition-colors",
                  timeFilter === opt.value
                    ? "bg-primary/10 text-primary dark:bg-primary/20"
                    : "text-slate-500 dark:text-gray-400 hover:bg-slate-100 dark:hover:bg-white/5"
                )}
              >
                {opt.label}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Board */}
      <div className="flex-1 overflow-x-auto overflow-y-hidden">
        <div className="inline-flex h-full min-w-full">
          {visibleColumns.map((column) => (
            <div
              key={column.id}
              className="flex flex-col w-[320px] min-w-[280px] max-w-[400px] border-r border-slate-200 dark:border-border-dark"
            >
              {/* Column header */}
              <div
                className={cn(
                  "shrink-0 px-4 py-3 border-b-2 flex items-center justify-between",
                  column.borderColor
                )}
              >
                <div className="flex items-center gap-2">
                  <div className={cn("w-2 h-2 rounded-full", column.bgColor)} />
                  <span className={cn("text-sm font-semibold", column.color)}>
                    {column.label}
                  </span>
                  <span className="text-xs text-slate-400 dark:text-gray-500 bg-slate-100 dark:bg-white/5 px-1.5 py-0.5 rounded">
                    {tasksByStatus[column.id].length}
                  </span>
                </div>
              </div>

              {/* Column content */}
              <div className="flex-1 overflow-y-auto p-3 space-y-3">
                {tasksByStatus[column.id].length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-12 text-slate-400 dark:text-gray-500">
                    <Codicon name="inbox" className="text-[32px] mb-2 opacity-50" />
                    <p className="text-xs">No tasks</p>
                  </div>
                ) : (
                  tasksByStatus[column.id].map((task) => (
                    <TaskCard
                      key={task.id}
                      task={task}
                      agents={agents}
                      isSelected={selectedTaskId === task.id}
                      onClick={() => handleTaskSelect(task)}
                      onCancel={() => handleCancelTask(task.id)}
                    />
                  ))
                )}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
