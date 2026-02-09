"use client";

import { cn } from "@/lib/cn";

interface BadgeProps {
  children: React.ReactNode;
  variant?: "primary" | "mcp" | "skill";
  className?: string;
}

export function Badge({ children, variant = "primary", className }: BadgeProps) {
  return (
    <span
      className={cn(
        "px-2 py-0.5 rounded text-[10px] font-bold uppercase tracking-wider",
        variant === "primary" && "bg-primary/20 text-primary border border-primary/30",
        variant === "mcp" && "bg-cyan-500/10 text-cyan-400 border border-cyan-500/30",
        variant === "skill" && "bg-purple-500/10 text-purple-400 border border-purple-500/30",
        className
      )}
    >
      {children}
    </span>
  );
}
