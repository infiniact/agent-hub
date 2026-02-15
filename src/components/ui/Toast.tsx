"use client";

import { useEffect } from "react";
import { cn } from "@/lib/cn";
import { Codicon } from "@/components/ui/Codicon";
import { useToastStore, type Toast } from "@/stores/toastStore";

const ICON_MAP: Record<Toast["type"], string> = {
  error: "error",
  warning: "warning",
  info: "info",
  success: "pass",
};

const STYLE_MAP: Record<Toast["type"], string> = {
  error:
    "border-rose-500/40 bg-rose-950/80 text-rose-100 dark:border-rose-500/40 dark:bg-rose-950/80 dark:text-rose-100 border-rose-400/40 bg-rose-50 text-rose-900",
  warning:
    "border-amber-500/40 bg-amber-950/80 text-amber-100 dark:border-amber-500/40 dark:bg-amber-950/80 dark:text-amber-100 border-amber-400/40 bg-amber-50 text-amber-900",
  info:
    "border-primary/40 bg-cyan-950/80 text-cyan-100 dark:border-primary/40 dark:bg-cyan-950/80 dark:text-cyan-100 border-cyan-400/40 bg-cyan-50 text-cyan-900",
  success:
    "border-emerald-500/40 bg-emerald-950/80 text-emerald-100 dark:border-emerald-500/40 dark:bg-emerald-950/80 dark:text-emerald-100 border-emerald-400/40 bg-emerald-50 text-emerald-900",
};

const ICON_COLOR_MAP: Record<Toast["type"], string> = {
  error: "text-rose-400",
  warning: "text-amber-400",
  info: "text-primary",
  success: "text-emerald-400",
};

function ToastItem({ toast }: { toast: Toast }) {
  const removeToast = useToastStore((s) => s.removeToast);
  const duration = toast.duration ?? 5000;

  useEffect(() => {
    if (duration <= 0) return;
    const timer = setTimeout(() => removeToast(toast.id), duration);
    return () => clearTimeout(timer);
  }, [toast.id, duration, removeToast]);

  return (
    <div
      className={cn(
        "pointer-events-auto flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 shadow-lg backdrop-blur-sm",
        "min-w-[280px] max-w-[400px] animate-[slideIn_0.2s_ease-out]",
        STYLE_MAP[toast.type]
      )}
    >
      <Codicon
        name={ICON_MAP[toast.type]}
        className={cn("mt-0.5 shrink-0 text-[16px]", ICON_COLOR_MAP[toast.type])}
      />
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium leading-tight">{toast.title}</p>
        {toast.message && (
          <p className="mt-0.5 text-xs opacity-80 leading-snug line-clamp-3">
            {toast.message}
          </p>
        )}
      </div>
      <button
        onClick={() => removeToast(toast.id)}
        className="shrink-0 mt-0.5 rounded p-0.5 opacity-60 hover:opacity-100 transition-opacity"
      >
        <Codicon name="close" className="text-[14px]" />
      </button>
    </div>
  );
}

export function ToastContainer() {
  const toasts = useToastStore((s) => s.toasts);

  if (toasts.length === 0) return null;

  return (
    <div className="fixed top-4 right-4 z-[200] flex flex-col gap-2 pointer-events-none">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} />
      ))}
    </div>
  );
}
