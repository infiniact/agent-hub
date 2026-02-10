"use client";

import { cn } from "@/lib/cn";
import { useState, useRef, useEffect } from "react";
import { Codicon } from "@/components/ui/Codicon";

interface SelectOption {
  label: string;
  value: string;
  disabled?: boolean;
}

interface SelectProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  className?: string;
}

export function Select({ value, options, onChange, className }: SelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, []);

  const selected = options.find((o) => o.value === value);

  return (
    <div ref={ref} className={cn("relative", className)}>
      <button
        onClick={() => setOpen(!open)}
        className="w-full h-9 flex items-center justify-between px-3 bg-white dark:bg-background-dark border border-slate-200 dark:border-border-dark rounded-lg text-sm text-slate-900 dark:text-white"
      >
        <span>{selected?.label ?? value}</span>
        <Codicon name="chevron-down" className="text-slate-400" />
      </button>
      {open && (
        <div className="absolute top-[calc(100%+4px)] left-0 w-full bg-white dark:bg-surface-dark border border-slate-200 dark:border-border-dark rounded-lg shadow-xl z-50 py-1 max-h-64 overflow-y-auto">
          {options.map((opt) => (
            <button
              key={opt.value}
              disabled={opt.disabled}
              className={cn(
                "w-full text-left px-4 py-2 text-sm transition-colors",
                opt.disabled
                  ? "text-slate-300 dark:text-gray-600 cursor-not-allowed"
                  : "text-slate-600 dark:text-gray-300 hover:bg-primary/10 hover:text-primary cursor-pointer"
              )}
              onClick={() => {
                if (opt.disabled) return;
                onChange(opt.value);
                setOpen(false);
              }}
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
