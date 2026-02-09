"use client";

import { cn } from "@/lib/cn";

interface SliderProps {
  value: number;
  min: number;
  max: number;
  step: number;
  label: string;
  onChange: (value: number) => void;
  formatValue?: (value: number) => string;
  className?: string;
}

export function Slider({
  value,
  min,
  max,
  step,
  label,
  onChange,
  formatValue,
  className,
}: SliderProps) {
  return (
    <div className={cn("flex-1 flex flex-col gap-1 max-w-md", className)}>
      <div className="flex justify-between items-center">
        <span className="text-xs font-medium text-slate-400">{label}</span>
        <span className="text-xs font-mono text-primary">
          {formatValue ? formatValue(value) : value}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full h-1 bg-slate-200 dark:bg-gray-700 rounded-lg appearance-none cursor-pointer accent-primary"
      />
    </div>
  );
}
