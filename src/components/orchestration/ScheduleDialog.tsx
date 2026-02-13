"use client";

import { useState, useMemo } from "react";
import { Codicon } from "@/components/ui/Codicon";
import { cn } from "@/lib/cn";
import type { TaskRun, RecurrencePattern, ScheduleTaskRequest } from "@/types/orchestration";

interface ScheduleDialogProps {
  taskRun: TaskRun;
  onSchedule: (request: ScheduleTaskRequest) => Promise<void>;
  onClear: () => Promise<void>;
  onClose: () => void;
}

type ScheduleType = "none" | "once" | "recurring";
type Frequency = "daily" | "weekly" | "monthly" | "yearly";

const DAYS_OF_WEEK = [
  { value: 0, label: "Sun" },
  { value: 1, label: "Mon" },
  { value: 2, label: "Tue" },
  { value: 3, label: "Wed" },
  { value: 4, label: "Thu" },
  { value: 5, label: "Fri" },
  { value: 6, label: "Sat" },
];

export function ScheduleDialog({ taskRun, onSchedule, onClear, onClose }: ScheduleDialogProps) {
  // Parse existing schedule
  const existingPattern: RecurrencePattern | null = taskRun.recurrence_pattern_json
    ? JSON.parse(taskRun.recurrence_pattern_json)
    : null;

  const [scheduleType, setScheduleType] = useState<ScheduleType>(
    taskRun.schedule_type === "none" ? "once" : taskRun.schedule_type
  );
  const [frequency, setFrequency] = useState<Frequency>(
    existingPattern?.frequency || "daily"
  );
  const [scheduledDate, setScheduledDate] = useState<string>(() => {
    if (taskRun.scheduled_time) {
      return taskRun.scheduled_time.slice(0, 16); // datetime-local format
    }
    // Default to tomorrow
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    tomorrow.setHours(9, 0, 0, 0);
    return tomorrow.toISOString().slice(0, 16);
  });
  const [scheduledTime, setScheduledTime] = useState<string>(
    existingPattern?.time || "09:00"
  );
  const [interval, setInterval] = useState<number>(existingPattern?.interval || 1);
  const [daysOfWeek, setDaysOfWeek] = useState<number[]>(
    existingPattern?.days_of_week || [1, 2, 3, 4, 5] // Mon-Fri by default
  );
  const [dayOfMonth, setDayOfMonth] = useState<number>(
    existingPattern?.day_of_month || 1
  );
  const [month, setMonth] = useState<number>(existingPattern?.month || 1);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Calculate next run preview
  const nextRunPreview = useMemo(() => {
    if (scheduleType === "none") return null;

    if (scheduleType === "once") {
      const date = new Date(scheduledDate);
      return date.toLocaleString("zh-CN", {
        year: "numeric",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      });
    }

    // For recurring, calculate next occurrence
    const now = new Date();
    const [hours, minutes] = scheduledTime.split(":").map(Number);

    switch (frequency) {
      case "daily": {
        const next = new Date();
        next.setHours(hours, minutes, 0, 0);
        if (next <= now) {
          next.setDate(next.getDate() + interval);
        }
        return `${frequency} at ${scheduledTime} (next: ${next.toLocaleDateString("zh-CN", { month: "short", day: "numeric" })})`;
      }
      case "weekly": {
        const currentDay = now.getDay();
        const nextDay = daysOfWeek.find((d) => d > currentDay) ?? daysOfWeek[0];
        const diff = nextDay > currentDay ? nextDay - currentDay : 7 - currentDay + nextDay;
        const next = new Date(now);
        next.setDate(next.getDate() + diff);
        next.setHours(hours, minutes, 0, 0);
        return `weekly on ${daysOfWeek.map((d) => DAYS_OF_WEEK[d].label).join(", ")} at ${scheduledTime}`;
      }
      case "monthly":
        return `monthly on day ${dayOfMonth} at ${scheduledTime}`;
      case "yearly":
        return `yearly on ${new Date(2000, month - 1, dayOfMonth).toLocaleDateString("zh-CN", { month: "long", day: "numeric" })} at ${scheduledTime}`;
      default:
        return null;
    }
  }, [scheduleType, scheduledDate, scheduledTime, frequency, interval, daysOfWeek, dayOfMonth, month]);

  const handleToggleDayOfWeek = (day: number) => {
    setDaysOfWeek((prev) =>
      prev.includes(day) ? prev.filter((d) => d !== day) : [...prev, day].sort()
    );
  };

  const handleSubmit = async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (scheduleType === "none") {
        await onClear();
      } else {
        const request: ScheduleTaskRequest = {
          task_run_id: taskRun.id,
          schedule_type: scheduleType,
          scheduled_time: scheduleType === "once" ? new Date(scheduledDate).toISOString() : undefined,
          recurrence_pattern: scheduleType === "recurring" ? {
            frequency,
            time: scheduledTime,
            interval,
            days_of_week: frequency === "weekly" ? daysOfWeek : undefined,
            day_of_month: frequency === "monthly" || frequency === "yearly" ? dayOfMonth : undefined,
            month: frequency === "yearly" ? month : undefined,
          } : undefined,
        };
        await onSchedule(request);
      }
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save schedule");
    } finally {
      setIsLoading(false);
    }
  };

  const handleClear = async () => {
    setIsLoading(true);
    setError(null);
    try {
      await onClear();
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to clear schedule");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-white dark:bg-surface-dark rounded-xl shadow-2xl w-[480px] max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-slate-200 dark:border-border-dark">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
              <Codicon name="clock" className="text-[18px] text-blue-600 dark:text-blue-400" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-slate-900 dark:text-white">
                Schedule Task
              </h2>
              <p className="text-xs text-slate-500 dark:text-gray-400 truncate max-w-[300px]">
                {taskRun.title || taskRun.user_prompt.slice(0, 50)}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-white/5 text-slate-400"
          >
            <Codicon name="close" className="text-[18px]" />
          </button>
        </div>

        {/* Content */}
        <div className="p-5 space-y-5">
          {/* Schedule Type Selection */}
          <div>
            <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
              Schedule Type
            </label>
            <div className="flex gap-2">
              {[
                { value: "once" as ScheduleType, label: "One Time", icon: "calendar" },
                { value: "recurring" as ScheduleType, label: "Recurring", icon: "refresh" },
              ].map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => setScheduleType(opt.value)}
                  className={cn(
                    "flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg border text-sm font-medium transition-colors",
                    scheduleType === opt.value
                      ? "border-primary bg-primary/10 text-primary"
                      : "border-slate-200 dark:border-border-dark text-slate-600 dark:text-gray-400 hover:border-slate-300 dark:hover:border-slate-600"
                  )}
                >
                  <Codicon name={opt.icon} className="text-[16px]" />
                  {opt.label}
                </button>
              ))}
            </div>
          </div>

          {/* One-time schedule */}
          {scheduleType === "once" && (
            <div>
              <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                Run At
              </label>
              <input
                type="datetime-local"
                value={scheduledDate}
                onChange={(e) => setScheduledDate(e.target.value)}
                className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
              />
            </div>
          )}

          {/* Recurring schedule */}
          {scheduleType === "recurring" && (
            <>
              {/* Frequency */}
              <div>
                <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                  Frequency
                </label>
                <div className="flex gap-2">
                  {(["daily", "weekly", "monthly", "yearly"] as Frequency[]).map((f) => (
                    <button
                      key={f}
                      onClick={() => setFrequency(f)}
                      className={cn(
                        "flex-1 px-3 py-2 rounded-lg border text-xs font-medium capitalize transition-colors",
                        frequency === f
                          ? "border-primary bg-primary/10 text-primary"
                          : "border-slate-200 dark:border-border-dark text-slate-600 dark:text-gray-400 hover:border-slate-300"
                      )}
                    >
                      {f}
                    </button>
                  ))}
                </div>
              </div>

              {/* Time */}
              <div>
                <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                  Time
                </label>
                <input
                  type="time"
                  value={scheduledTime}
                  onChange={(e) => setScheduledTime(e.target.value)}
                  className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                />
              </div>

              {/* Interval */}
              <div>
                <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                  Repeat every
                </label>
                <div className="flex items-center gap-2">
                  <input
                    type="number"
                    min={1}
                    max={99}
                    value={interval}
                    onChange={(e) => setInterval(Math.max(1, parseInt(e.target.value) || 1))}
                    className="w-20 px-3 py-2 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                  />
                  <span className="text-sm text-slate-600 dark:text-gray-400">
                    {frequency === "daily" ? "day(s)" : frequency === "weekly" ? "week(s)" : frequency === "monthly" ? "month(s)" : "year(s)"}
                  </span>
                </div>
              </div>

              {/* Days of week (for weekly) */}
              {frequency === "weekly" && (
                <div>
                  <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                    Days of Week
                  </label>
                  <div className="flex gap-1">
                    {DAYS_OF_WEEK.map((day) => (
                      <button
                        key={day.value}
                        onClick={() => handleToggleDayOfWeek(day.value)}
                        className={cn(
                          "flex-1 px-2 py-2 rounded-lg border text-xs font-medium transition-colors",
                          daysOfWeek.includes(day.value)
                            ? "border-primary bg-primary/10 text-primary"
                            : "border-slate-200 dark:border-border-dark text-slate-600 dark:text-gray-400 hover:border-slate-300"
                        )}
                      >
                        {day.label}
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {/* Day of month (for monthly/yearly) */}
              {(frequency === "monthly" || frequency === "yearly") && (
                <div>
                  <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                    Day of Month
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={31}
                    value={dayOfMonth}
                    onChange={(e) => setDayOfMonth(Math.min(31, Math.max(1, parseInt(e.target.value) || 1)))}
                    className="w-20 px-3 py-2 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                  />
                </div>
              )}

              {/* Month (for yearly) */}
              {frequency === "yearly" && (
                <div>
                  <label className="block text-xs font-medium text-slate-600 dark:text-gray-300 mb-2">
                    Month
                  </label>
                  <select
                    value={month}
                    onChange={(e) => setMonth(parseInt(e.target.value))}
                    className="w-full px-3 py-2 rounded-lg border border-slate-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-slate-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary/50"
                  >
                    {[
                      "January", "February", "March", "April", "May", "June",
                      "July", "August", "September", "October", "November", "December"
                    ].map((m, i) => (
                      <option key={i} value={i + 1}>{m}</option>
                    ))}
                  </select>
                </div>
              )}
            </>
          )}

          {/* Preview */}
          {nextRunPreview && (
            <div className="p-3 rounded-lg bg-slate-50 dark:bg-white/5 border border-slate-200 dark:border-border-dark">
              <div className="flex items-center gap-2 text-xs">
                <Codicon name="info" className="text-[14px] text-blue-500" />
                <span className="text-slate-600 dark:text-gray-300">Next run:</span>
                <span className="font-medium text-slate-900 dark:text-white">{nextRunPreview}</span>
              </div>
            </div>
          )}

          {/* Error */}
          {error && (
            <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
              <div className="flex items-center gap-2 text-xs text-red-600 dark:text-red-400">
                <Codicon name="error" className="text-[14px]" />
                {error}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-4 border-t border-slate-200 dark:border-border-dark">
          <button
            onClick={handleClear}
            disabled={isLoading || taskRun.schedule_type === "none"}
            className={cn(
              "px-3 py-2 text-xs font-medium rounded-lg transition-colors",
              taskRun.schedule_type === "none"
                ? "text-slate-300 dark:text-slate-600 cursor-not-allowed"
                : "text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
            )}
          >
            Clear Schedule
          </button>
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 text-xs font-medium rounded-lg border border-slate-200 dark:border-border-dark text-slate-600 dark:text-gray-400 hover:bg-slate-50 dark:hover:bg-white/5 transition-colors"
            >
              Cancel
            </button>
            <button
              onClick={handleSubmit}
              disabled={isLoading}
              className="px-4 py-2 text-xs font-medium rounded-lg bg-primary text-white hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              {isLoading ? "Saving..." : "Save Schedule"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
